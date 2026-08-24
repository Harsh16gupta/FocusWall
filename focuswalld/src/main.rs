//! FocusWall Daemon (`focuswalld`)
//!
//! Enforces DNS and firewall website blocking policies and serves the Unix domain socket IPC.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use chrono::{DateTime, Local, NaiveTime, TimeZone, Utc};
use clap::{Parser, Subcommand};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use focuswall_core::{
    evaluate_youtube_state, normalize_domain_input, resolve_domain_ips,
    write_ipc_response, DaemonConfig, Database, DnsManager, IpcRequest, IpcResponse,
    NftablesManager, PolicyKind,
};

#[derive(Parser, Debug)]
#[command(name = "focuswalld", about = "FocusWall System-Level Enforcement Daemon")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to static daemon configuration file (TOML)
    #[arg(short, long, default_value = "/etc/focuswall/config.toml")]
    config: PathBuf,

    /// Path to SQLite database file (overrides config file if specified)
    #[arg(long)]
    db_path: Option<PathBuf>,

    /// Path to dnsmasq configuration file (overrides config file if specified)
    #[arg(long)]
    dns_conf_path: Option<PathBuf>,

    /// Path to Unix domain socket for IPC (overrides config file if specified)
    #[arg(long)]
    socket_path: Option<PathBuf>,

    /// Directory to store firewall nftables cache files
    #[arg(long)]
    firewall_cache_dir: Option<PathBuf>,

    /// Override current time for deterministic testing/simulation (e.g. '20:30:00' or RFC3339)
    #[arg(long)]
    fake_now: Option<String>,

    /// Polling interval in seconds for schedule evaluation
    #[arg(long)]
    poll_interval_secs: Option<u64>,

    /// Run single reconciliation cycle and exit (for testing/diagnostics)
    #[arg(long)]
    run_once: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show current enforcement status and policies
    Status {
        /// Optional simulated time for status inspection
        #[arg(long)]
        fake_now: Option<String>,

        /// Path to SQLite database file
        #[arg(long)]
        db_path: Option<PathBuf>,

        /// Path to configuration file
        #[arg(short, long, default_value = "/etc/focuswall/config.toml")]
        config: PathBuf,
    },

    /// Propose and add a new website to block
    AddRule {
        /// Domain name or URL (e.g. 'reddit.com' or 'https://www.reddit.com/r/programming')
        input: String,

        /// Removal cooldown in hours (default: 24)
        #[arg(long, default_value_t = 24)]
        cooldown_hours: u32,

        /// Path to SQLite database file
        #[arg(long)]
        db_path: Option<PathBuf>,

        /// Path to configuration file
        #[arg(long, default_value = "/etc/focuswall/config.toml")]
        config: PathBuf,
    },

    /// Request removal for a custom rule (begins cooldown countdown)
    RequestRemoval {
        /// ID of the custom policy rule
        rule_id: i64,

        /// Optional reason for removal
        #[arg(long)]
        reason: Option<String>,

        /// Path to SQLite database file
        #[arg(long)]
        db_path: Option<PathBuf>,

        /// Path to configuration file
        #[arg(long, default_value = "/etc/focuswall/config.toml")]
        config: PathBuf,
    },

    /// Confirm and finalize removal of a custom rule after cooldown has elapsed
    ConfirmRemoval {
        /// ID of the custom policy rule
        rule_id: i64,

        /// Path to SQLite database file
        #[arg(long)]
        db_path: Option<PathBuf>,

        /// Path to configuration file
        #[arg(long, default_value = "/etc/focuswall/config.toml")]
        config: PathBuf,
    },

    /// Cancel a pending removal request
    CancelRemoval {
        /// ID of the custom policy rule
        rule_id: i64,

        /// Path to SQLite database file
        #[arg(long)]
        db_path: Option<PathBuf>,

        /// Path to configuration file
        #[arg(long, default_value = "/etc/focuswall/config.toml")]
        config: PathBuf,
    },

    /// View recent audit logs
    Logs {
        /// Maximum number of log entries to display
        #[arg(long, default_value_t = 20)]
        limit: u32,

        /// Path to SQLite database file
        #[arg(long)]
        db_path: Option<PathBuf>,

        /// Path to configuration file
        #[arg(long, default_value = "/etc/focuswall/config.toml")]
        config: PathBuf,
    },
}

fn parse_simulated_time(time_str: &str) -> Option<DateTime<Local>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(time_str) {
        return Some(dt.with_timezone(&Local));
    }

    let local_now = Local::now();
    let today = local_now.date_naive();

    if let Ok(time) = NaiveTime::parse_from_str(time_str, "%H:%M:%S") {
        return Local.from_local_datetime(&today.and_time(time)).single();
    }
    if let Ok(time) = NaiveTime::parse_from_str(time_str, "%H:%M") {
        return Local.from_local_datetime(&today.and_time(time)).single();
    }

    None
}

fn get_current_time(simulated: Option<&str>) -> DateTime<Local> {
    if let Some(s) = simulated {
        if let Some(dt) = parse_simulated_time(s) {
            return dt;
        }
        warn!("Failed to parse simulated time '{}', falling back to actual clock", s);
    }
    Local::now()
}

fn print_status(db_path: &PathBuf, fake_now: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open(db_path)?;
    let now = get_current_time(fake_now);
    let yt_state = evaluate_youtube_state(&now);
    let policies = db.get_active_policies()?;
    let blocked_domains = db.get_blocked_domains(&now)?;

    println!("==================================================");
    println!("FocusWall Status Overview");
    println!("==================================================");
    println!("Current Time (Local): {}", now.format("%Y-%m-%d %H:%M:%S %Z"));
    println!(
        "YouTube Window Status: [{:?}] (Allowed 20:00 - 21:00)",
        yt_state
    );
    println!("Active Policies Count: {}", policies.len());
    println!("Currently Blocked Domains Count: {}", blocked_domains.len());
    println!("--------------------------------------------------");
    println!("Policies:");
    for p in &policies {
        let state = p.evaluate(&now);
        let schedule_str = match &p.schedule {
            Some(w) => format!(" (Allowed {}-{})", w.start.format("%H:%M"), w.end.format("%H:%M")),
            None => " (24/7 Blocked)".to_string(),
        };
        let kind_str = match p.kind {
            PolicyKind::System => "SYSTEM",
            PolicyKind::Custom => "CUSTOM",
        };
        let removal_info = match &p.earliest_removal_at {
            Some(era) => format!(" [Removal Eligible At: {}]", era),
            None => "".to_string(),
        };
        println!(
            " • [ID: {:?}] [{}] {} - [{:?}]{} (Status: {:?}){}",
            p.id.unwrap_or(0), kind_str, p.name, state, schedule_str, p.status, removal_info
        );
    }
    println!("==================================================");

    Ok(())
}

async fn handle_ipc_client(
    mut stream: UnixStream,
    db: Arc<Mutex<Database>>,
    fake_now: Option<String>,
) {
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    while let Ok(n) = buf_reader.read_line(&mut line).await {
        if n == 0 {
            break;
        }

        let req_res: Result<IpcRequest, _> = serde_json::from_str(&line);
        let resp = match req_res {
            Ok(req) => match req {
                IpcRequest::GetStatus => {
                    let db_guard = db.lock().await;
                    let now = get_current_time(fake_now.as_deref());
                    let yt_state = evaluate_youtube_state(&now);
                    let policies = db_guard.get_active_policies().unwrap_or_default();
                    let blocked = db_guard.get_blocked_domains(&now).unwrap_or_default();
                    IpcResponse::Status {
                        current_time: now.to_rfc3339(),
                        youtube_state: yt_state,
                        policies,
                        blocked_domains: blocked,
                    }
                }
                IpcRequest::AddRule { input, cooldown_hours } => {
                    match normalize_domain_input(&input) {
                        Ok(normalized) => {
                            let db_guard = db.lock().await;
                            let cooldown = cooldown_hours.unwrap_or(24);
                            match db_guard.add_custom_rule(
                                &normalized.root_domain,
                                &normalized.domains,
                                cooldown,
                            ) {
                                Ok(policy) => IpcResponse::RuleAdded { policy },
                                Err(e) => IpcResponse::Error { message: e.to_string() },
                            }
                        }
                        Err(e) => IpcResponse::Error { message: e.to_string() },
                    }
                }
                IpcRequest::RequestRemoval { rule_id, reason, cooldown_hours_override } => {
                    let db_guard = db.lock().await;
                    match db_guard.request_removal(rule_id, reason.as_deref(), cooldown_hours_override) {
                        Ok(policy) => {
                            let era = policy.earliest_removal_at.clone().unwrap_or_default();
                            IpcResponse::RemovalRequested {
                                policy,
                                earliest_removal_at: era,
                            }
                        }
                        Err(e) => IpcResponse::Error { message: e.to_string() },
                    }
                }
                IpcRequest::ConfirmRemoval { rule_id } => {
                    let db_guard = db.lock().await;
                    let now = get_current_time(fake_now.as_deref());
                    match db_guard.confirm_removal(rule_id, &now) {
                        Ok(policy) => IpcResponse::RemovalConfirmed { policy },
                        Err(e) => IpcResponse::Error { message: e.to_string() },
                    }
                }
                IpcRequest::CancelRemovalRequest { rule_id } => {
                    let db_guard = db.lock().await;
                    match db_guard.cancel_removal_request(rule_id) {
                        Ok(policy) => IpcResponse::RemovalCancelled { policy },
                        Err(e) => IpcResponse::Error { message: e.to_string() },
                    }
                }
                IpcRequest::GetLogs { limit } => {
                    let db_guard = db.lock().await;
                    match db_guard.get_audit_logs(limit.unwrap_or(20)) {
                        Ok(entries) => IpcResponse::Logs { entries },
                        Err(e) => IpcResponse::Error { message: e.to_string() },
                    }
                }
            },
            Err(e) => IpcResponse::Error {
                message: format!("Invalid JSON request: {}", e),
            },
        };

        if let Err(e) = write_ipc_response(&mut writer, &resp).await {
            error!("Failed to write IPC response: {}", e);
            break;
        }

        line.clear();
    }
}

async fn run_cli_command(
    cmd: Commands,
    _default_cfg: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Commands::Status { fake_now, db_path, config: cfg_path } => {
            let config = DaemonConfig::load_from_file(&cfg_path).unwrap_or_default();
            let target_db = db_path.unwrap_or(config.db_path);
            print_status(&target_db, fake_now.as_deref())?;
        }
        Commands::AddRule { input, cooldown_hours, db_path, config: cfg_path } => {
            let config = DaemonConfig::load_from_file(&cfg_path).unwrap_or_default();
            let target_db = db_path.unwrap_or(config.db_path);
            let normalized = normalize_domain_input(&input)?;

            println!("Normalized root domain: {}", normalized.root_domain);
            println!("Blocked domains pattern: {:?}", normalized.domains);
            println!("Cooldown duration: {} hours", cooldown_hours);

            let db = Database::open(&target_db)?;
            let policy = db.add_custom_rule(&normalized.root_domain, &normalized.domains, cooldown_hours)?;
            println!("Successfully added rule for '{}' (ID: {:?})", policy.name, policy.id);
        }
        Commands::RequestRemoval { rule_id, reason, db_path, config: cfg_path } => {
            let config = DaemonConfig::load_from_file(&cfg_path).unwrap_or_default();
            let target_db = db_path.unwrap_or(config.db_path);
            let db = Database::open(&target_db)?;
            let policy = db.request_removal(rule_id, reason.as_deref(), None)?;
            println!(
                "Removal requested for rule '{}' (ID: {}). Earliest removal at: {}",
                policy.name, rule_id, policy.earliest_removal_at.unwrap_or_default()
            );
            println!("Note: Site remains BLOCKED during the entire cooldown period.");
        }
        Commands::ConfirmRemoval { rule_id, db_path, config: cfg_path } => {
            let config = DaemonConfig::load_from_file(&cfg_path).unwrap_or_default();
            let target_db = db_path.unwrap_or(config.db_path);
            let db = Database::open(&target_db)?;
            let now = Utc::now();
            let policy = db.confirm_removal(rule_id, &now)?;
            println!("Successfully confirmed removal of rule '{}' (ID: {}).", policy.name, rule_id);
        }
        Commands::CancelRemoval { rule_id, db_path, config: cfg_path } => {
            let config = DaemonConfig::load_from_file(&cfg_path).unwrap_or_default();
            let target_db = db_path.unwrap_or(config.db_path);
            let db = Database::open(&target_db)?;
            let policy = db.cancel_removal_request(rule_id)?;
            println!("Successfully cancelled removal request for rule '{}' (ID: {}). Status: Active.", policy.name, rule_id);
        }
        Commands::Logs { limit, db_path, config: cfg_path } => {
            let config = DaemonConfig::load_from_file(&cfg_path).unwrap_or_default();
            let target_db = db_path.unwrap_or(config.db_path);
            let db = Database::open(&target_db)?;
            let entries = db.get_audit_logs(limit)?;
            println!("==================================================");
            println!("FocusWall Audit Log (Most recent {})", entries.len());
            println!("==================================================");
            for entry in entries {
                println!("[{}] [{}] {}", entry.ts, entry.event_type, entry.detail);
            }
            println!("==================================================");
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let cli = Cli::parse();

    if let Some(cmd) = cli.command {
        return run_cli_command(cmd, cli.config).await;
    }

    // Load static config file or fall back to defaults
    let mut config = DaemonConfig::load_from_file(&cli.config).unwrap_or_default();

    // When running without root/sudo in development mode, fallback system default paths to /tmp
    if !focuswall_core::is_running_as_root() {
        if config.dns_conf_path == PathBuf::from("/etc/dnsmasq.d/focuswall.conf") {
            config.dns_conf_path = PathBuf::from("/tmp/focuswall_dns.conf");
            info!("Unprivileged mode: default DNS config redirected to /tmp/focuswall_dns.conf");
        }
        if config.socket_path == PathBuf::from("/run/focuswall/focuswall.sock") {
            config.socket_path = PathBuf::from("/tmp/focuswall.sock");
            info!("Unprivileged mode: default IPC socket redirected to /tmp/focuswall.sock");
        }
        if config.db_path == PathBuf::from("/var/lib/focuswall/focuswall.db") {
            config.db_path = PathBuf::from("/tmp/focuswall.db");
            info!("Unprivileged mode: default DB redirected to /tmp/focuswall.db");
        }
    }

    // Command-line flag overrides
    if let Some(db_p) = cli.db_path {
        config.db_path = db_p;
    }
    if let Some(dns_p) = cli.dns_conf_path {
        config.dns_conf_path = dns_p;
    }
    if let Some(sock_p) = cli.socket_path {
        config.socket_path = sock_p;
    }
    if let Some(poll_secs) = cli.poll_interval_secs {
        config.poll_interval_secs = poll_secs;
    }

    let firewall_cache_dir = cli.firewall_cache_dir.unwrap_or_else(|| {
        config
            .db_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("/tmp"))
            .to_path_buf()
    });

    info!("Starting focuswalld daemon...");
    info!("Configuration file: {}", cli.config.display());
    info!("Database path: {}", config.db_path.display());
    info!("DNS configuration path: {}", config.dns_conf_path.display());
    info!("IPC Socket path: {}", config.socket_path.display());
    info!("Firewall cache directory: {}", firewall_cache_dir.display());

    // Initialize Database, DNS Manager, and Firewall Manager
    let db = match Database::open(&config.db_path) {
        Ok(d) => Arc::new(Mutex::new(d)),
        Err(e) => {
            error!("Failed to initialize database: {}", e);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
        }
    };

    let dns_mgr = DnsManager::new(&config.dns_conf_path);
    let nft_mgr = NftablesManager::new(&firewall_cache_dir);

    // Setup IPC socket
    if let Some(parent) = config.socket_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::remove_file(&config.socket_path);

    let ipc_listener = match UnixListener::bind(&config.socket_path) {
        Ok(l) => {
            info!("IPC Unix socket listening on {}", config.socket_path.display());
            Some(l)
        }
        Err(e) => {
            warn!("Failed to bind IPC socket at {}: {}", config.socket_path.display(), e);
            None
        }
    };

    // Spawn IPC handler background task
    if let Some(listener) = ipc_listener {
        let db_clone = Arc::clone(&db);
        let fake_now_clone = cli.fake_now.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let client_db = Arc::clone(&db_clone);
                        let client_fake_now = fake_now_clone.clone();
                        tokio::spawn(async move {
                            handle_ipc_client(stream, client_db, client_fake_now).await;
                        });
                    }
                    Err(e) => {
                        warn!("IPC accept error: {}", e);
                        break;
                    }
                }
            }
        });
    }

    // Notify systemd that the daemon is initialized and ready
    let _ = sd_notify::notify(true, &[sd_notify::NotifyState::Ready]);
    {
        let db_guard = db.lock().await;
        let _ = db_guard.log_event("daemon_start", "focuswalld started successfully");
    }

    let mut last_applied_domains: Option<Vec<String>> = None;
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    info!("focuswalld initialization complete, entering main enforcement loop");

    loop {
        let now = get_current_time(cli.fake_now.as_deref());
        let current_blocked_domains = {
            let db_guard = db.lock().await;
            match db_guard.get_blocked_domains(&now) {
                Ok(domains) => domains,
                Err(e) => {
                    error!("Error evaluating blocked domains: {}", e);
                    Vec::new()
                }
            }
        };

        let needs_apply = match &last_applied_domains {
            Some(prev) => prev != &current_blocked_domains,
            None => true, // initial run
        };

        if needs_apply {
            let yt_state = evaluate_youtube_state(&now);
            info!(
                time = %now.format("%H:%M:%S"),
                youtube_state = ?yt_state,
                blocked_count = current_blocked_domains.len(),
                "State transition detected, applying DNS and firewall rules"
            );

            // Layer 1: DNS sinkhole configuration
            if let Err(e) = dns_mgr.apply_blocked_domains(&current_blocked_domains) {
                error!("Failed to write DNS blocking configuration: {}", e);
                let db_guard = db.lock().await;
                let _ = db_guard.log_event("enforcement_error", &format!("DNS write failed: {}", e));
            } else {
                let db_guard = db.lock().await;
                let _ = db_guard.log_event(
                    "policy_change",
                    &format!(
                        "DNS rules updated: YouTube={:?}, Total Blocked Domains={}",
                        yt_state,
                        current_blocked_domains.len()
                    ),
                );
            }

            // Layer 2: nftables IP backstop + DoH closure
            let resolved = resolve_domain_ips(&current_blocked_domains);
            if let Err(e) = nft_mgr.apply_rules(
                &resolved.ipv4,
                &resolved.ipv6,
                config.doh_blocking_enabled,
            ) {
                error!("Failed to apply nftables firewall rules: {}", e);
                let db_guard = db.lock().await;
                let _ = db_guard.log_event("enforcement_error", &format!("Firewall rules failed: {}", e));
            } else {
                let db_guard = db.lock().await;
                let _ = db_guard.log_event(
                    "policy_change",
                    &format!(
                        "Firewall rules updated: IPv4 count={}, IPv6 count={}, DoH blocked={}",
                        resolved.ipv4.len(),
                        resolved.ipv6.len(),
                        config.doh_blocking_enabled
                    ),
                );
            }

            last_applied_domains = Some(current_blocked_domains);
        }

        if cli.run_once {
            info!("Run once flag set, exiting after initial reconciliation");
            break;
        }

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(config.poll_interval_secs)) => {
                // Next tick
            }
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down focuswalld");
                break;
            }
            _ = sigint.recv() => {
                info!("Received SIGINT, shutting down focuswalld");
                break;
            }
        }
    }

    {
        let db_guard = db.lock().await;
        let _ = db_guard.log_event("daemon_stop", "focuswalld stopping cleanly");
    }
    let _ = fs::remove_file(&config.socket_path);
    let _ = sd_notify::notify(true, &[sd_notify::NotifyState::Stopping]);
    Ok(())
}
