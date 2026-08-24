//! FocusWall Daemon (`focuswalld`)
//!
//! Enforces DNS and firewall website blocking policies.

use std::path::PathBuf;
use std::time::Duration;
use chrono::{DateTime, Local, NaiveTime, TimeZone};
use clap::{Parser, Subcommand};
use tokio::signal::unix::{signal, SignalKind};
use tracing::{error, info, warn};

use focuswall_core::{
    evaluate_youtube_state, resolve_domain_ips, DaemonConfig, Database, DnsManager,
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
}

fn parse_simulated_time(time_str: &str) -> Option<DateTime<Local>> {
    // Try RFC3339 first
    if let Ok(dt) = DateTime::parse_from_rfc3339(time_str) {
        return Some(dt.with_timezone(&Local));
    }

    // Try HH:MM:SS or HH:MM on today's local date
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
        println!(
            " • [{}] {} - [{:?}]{} (Status: {:?})",
            kind_str, p.name, state, schedule_str, p.status
        );
    }
    println!("==================================================");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    // Load static config file or fall back to defaults
    let mut config = DaemonConfig::load_from_file(&cli.config).unwrap_or_default();

    // Command-line flag overrides
    if let Some(db_p) = cli.db_path {
        config.db_path = db_p;
    }
    if let Some(dns_p) = cli.dns_conf_path {
        config.dns_conf_path = dns_p;
    }
    if let Some(poll_secs) = cli.poll_interval_secs {
        config.poll_interval_secs = poll_secs;
    }

    if let Some(Commands::Status { fake_now, db_path, config: cfg_path }) = &cli.command {
        let status_config = DaemonConfig::load_from_file(cfg_path).unwrap_or_default();
        let target_db = db_path.clone().unwrap_or(status_config.db_path);
        return print_status(&target_db, fake_now.as_deref());
    }

    let firewall_cache_dir = cli.firewall_cache_dir.unwrap_or_else(|| {
        config
            .db_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("/var/lib/focuswall"))
            .to_path_buf()
    });

    info!("Starting focuswalld daemon...");
    info!("Configuration file: {}", cli.config.display());
    info!("Database path: {}", config.db_path.display());
    info!("DNS configuration path: {}", config.dns_conf_path.display());
    info!("Firewall cache directory: {}", firewall_cache_dir.display());

    // Initialize Database, DNS Manager, and Firewall Manager
    let db = match Database::open(&config.db_path) {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to initialize database: {}", e);
            return Err(e.into());
        }
    };

    let dns_mgr = DnsManager::new(&config.dns_conf_path);
    let nft_mgr = NftablesManager::new(&firewall_cache_dir);

    // Notify systemd that the daemon is initialized and ready
    let _ = sd_notify::notify(true, &[sd_notify::NotifyState::Ready]);
    let _ = db.log_event("daemon_start", "focuswalld started successfully");

    let mut last_applied_domains: Option<Vec<String>> = None;
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    info!("focuswalld initialization complete, entering main enforcement loop");

    loop {
        let now = get_current_time(cli.fake_now.as_deref());
        let current_blocked_domains = match db.get_blocked_domains(&now) {
            Ok(domains) => domains,
            Err(e) => {
                error!("Error evaluating blocked domains: {}", e);
                Vec::new()
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
                let _ = db.log_event("enforcement_error", &format!("DNS write failed: {}", e));
            } else {
                let _ = db.log_event(
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
                let _ = db.log_event("enforcement_error", &format!("Firewall rules failed: {}", e));
            } else {
                let _ = db.log_event(
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

    let _ = db.log_event("daemon_stop", "focuswalld stopping cleanly");
    let _ = sd_notify::notify(true, &[sd_notify::NotifyState::Stopping]);
    Ok(())
}
