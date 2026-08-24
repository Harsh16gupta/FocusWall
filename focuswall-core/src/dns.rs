//! DNS configuration generation and resolver management.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Error, Debug)]
pub enum DnsError {
    #[error("I/O error writing DNS config: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to execute DNS reload command: {0}")]
    Process(String),
}

/// Generates dnsmasq configuration content for a list of blocked domains.
/// Resolves both IPv4 to 0.0.0.0 and IPv6 to :: for complete DNS sinkholing.
pub fn generate_dnsmasq_config(blocked_domains: &[String]) -> String {
    let mut config = String::new();
    config.push_str("# FocusWall generated DNS blocking configuration\n");
    config.push_str("# DO NOT EDIT MANUALLY - Controlled by focuswalld\n\n");

    if blocked_domains.is_empty() {
        config.push_str("# No active domains blocked at this time.\n");
        return config;
    }

    for domain in blocked_domains {
        let trimmed = domain.trim();
        if !trimmed.is_empty() {
            config.push_str(&format!("address=/{}/0.0.0.0\n", trimmed));
            config.push_str(&format!("address=/{}/::\n", trimmed));
        }
    }

    config
}

pub struct DnsManager {
    config_path: PathBuf,
}

impl DnsManager {
    pub fn new<P: AsRef<Path>>(config_path: P) -> Self {
        Self {
            config_path: config_path.as_ref().to_path_buf(),
        }
    }

    /// Writes the generated DNS blocking configuration atomically.
    pub fn apply_blocked_domains(&self, blocked_domains: &[String]) -> Result<(), DnsError> {
        let content = generate_dnsmasq_config(blocked_domains);

        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write to temporary file in same directory for atomic rename
        let temp_path = self.config_path.with_extension("tmp");
        fs::write(&temp_path, content)?;
        fs::rename(&temp_path, &self.config_path)?;

        info!(
            target = %self.config_path.display(),
            domains_count = blocked_domains.len(),
            "Applied DNS blocking configuration"
        );

        self.reload_dnsmasq();
        Ok(())
    }

    /// Attempts to reload dnsmasq via standard signals.
    pub fn reload_dnsmasq(&self) {
        if !crate::is_running_as_root() || cfg!(test) {
            info!("Skipping dnsmasq service reload (running in test/unprivileged mode)");
            return;
        }

        // Attempt 1: systemctl reload-or-restart dnsmasq
        let sys_res = Command::new("systemctl")
            .args(["reload", "dnsmasq"])
            .output();

        match sys_res {
            Ok(out) if out.status.success() => {
                info!("Reloaded dnsmasq service successfully");
                return;
            }
            _ => {}
        }

        // Attempt 2: pkill -HUP dnsmasq
        let pkill_res = Command::new("pkill")
            .args(["-HUP", "dnsmasq"])
            .output();

        match pkill_res {
            Ok(out) if out.status.success() => {
                info!("Sent SIGHUP to dnsmasq successfully");
            }
            Ok(_) | Err(_) => {
                warn!("dnsmasq reload signal could not be sent (service may not be running yet)");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_generate_dnsmasq_config_empty() {
        let config = generate_dnsmasq_config(&[]);
        assert!(config.contains("# No active domains blocked"));
    }

    #[test]
    fn test_generate_dnsmasq_config_domains() {
        let domains = vec!["youtube.com".to_string(), "googlevideo.com".to_string()];
        let config = generate_dnsmasq_config(&domains);

        assert!(config.contains("address=/youtube.com/0.0.0.0"));
        assert!(config.contains("address=/youtube.com/::"));
        assert!(config.contains("address=/googlevideo.com/0.0.0.0"));
        assert!(config.contains("address=/googlevideo.com/::"));
    }

    #[test]
    fn test_dns_manager_atomic_write() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();
        let manager = DnsManager::new(&path);

        let domains = vec!["youtube.com".to_string()];
        manager.apply_blocked_domains(&domains).expect("applies config");

        let content = fs::read_to_string(&path).expect("reads config");
        assert!(content.contains("address=/youtube.com/0.0.0.0"));
    }
}
