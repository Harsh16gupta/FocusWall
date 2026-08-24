//! Daemon configuration file parsing and defaults.

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("I/O error reading config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse TOML configuration: {0}")]
    Parse(#[from] toml::de::Error),
}

/// Static daemon configuration loaded from `/etc/focuswall/config.toml`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,

    #[serde(default = "default_dns_conf_path")]
    pub dns_conf_path: PathBuf,

    #[serde(default = "default_socket_path")]
    pub socket_path: PathBuf,

    #[serde(default = "default_log_level")]
    pub log_level: String,

    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,

    #[serde(default = "default_doh_blocking")]
    pub doh_blocking_enabled: bool,

    #[serde(default = "default_dns_backend")]
    pub dns_backend: String,
}

fn default_db_path() -> PathBuf {
    PathBuf::from("/var/lib/focuswall/focuswall.db")
}

fn default_dns_conf_path() -> PathBuf {
    PathBuf::from("/etc/dnsmasq.d/focuswall.conf")
}

fn default_socket_path() -> PathBuf {
    PathBuf::from("/run/focuswall/focuswall.sock")
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_poll_interval() -> u64 {
    15
}

fn default_doh_blocking() -> bool {
    true
}

fn default_dns_backend() -> String {
    "dnsmasq".to_string()
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            db_path: default_db_path(),
            dns_conf_path: default_dns_conf_path(),
            socket_path: default_socket_path(),
            log_level: default_log_level(),
            poll_interval_secs: default_poll_interval(),
            doh_blocking_enabled: default_doh_blocking(),
            dns_backend: default_dns_backend(),
        }
    }
}

impl DaemonConfig {
    /// Loads configuration from a specific file path, or falls back to defaults if not found.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let p = path.as_ref();
        if !p.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(p)?;
        let config: DaemonConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Loads configuration from default system path `/etc/focuswall/config.toml`.
    pub fn load_default() -> Self {
        Self::load_from_file("/etc/focuswall/config.toml").unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        let cfg = DaemonConfig::default();
        assert_eq!(cfg.db_path, PathBuf::from("/var/lib/focuswall/focuswall.db"));
        assert_eq!(cfg.dns_conf_path, PathBuf::from("/etc/dnsmasq.d/focuswall.conf"));
        assert_eq!(cfg.socket_path, PathBuf::from("/run/focuswall/focuswall.sock"));
        assert_eq!(cfg.poll_interval_secs, 15);
        assert!(cfg.doh_blocking_enabled);
    }

    #[test]
    fn test_load_custom_toml() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
            db_path = "/tmp/custom.db"
            dns_conf_path = "/tmp/custom.conf"
            poll_interval_secs = 30
            log_level = "debug"
            "#
        ).unwrap();

        let cfg = DaemonConfig::load_from_file(file.path()).unwrap();
        assert_eq!(cfg.db_path, PathBuf::from("/tmp/custom.db"));
        assert_eq!(cfg.dns_conf_path, PathBuf::from("/tmp/custom.conf"));
        assert_eq!(cfg.poll_interval_secs, 30);
        assert_eq!(cfg.log_level, "debug");
        // default filled in
        assert_eq!(cfg.socket_path, PathBuf::from("/run/focuswall/focuswall.sock"));
    }
}
