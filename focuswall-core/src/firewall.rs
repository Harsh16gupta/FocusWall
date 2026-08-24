//! nftables IP backstop and DoH/DoT firewall management.

use std::fs;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Error, Debug)]
pub enum FirewallError {
    #[error("I/O error writing firewall rules: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to apply nftables ruleset: {0}")]
    NftCommand(String),
}

/// Known public DNS-over-HTTPS (DoH), DNS-over-TLS (DoT), and direct public DNS resolver IPv4 addresses.
pub const PUBLIC_DOH_IPV4: &[&str] = &[
    "1.1.1.1",        // Cloudflare
    "1.0.0.1",        // Cloudflare
    "8.8.8.8",        // Google
    "8.8.4.4",        // Google
    "9.9.9.9",        // Quad9
    "149.112.112.112",// Quad9
    "208.67.222.222", // OpenDNS
    "208.67.220.220", // OpenDNS
    "94.140.14.14",   // AdGuard
    "94.140.15.15",   // AdGuard
];

/// Known public DoH/DoT/DNS resolver IPv6 addresses.
pub const PUBLIC_DOH_IPV6: &[&str] = &[
    "2606:4700:4700::1111", // Cloudflare
    "2606:4700:4700::1001", // Cloudflare
    "2001:4860:4860::8888", // Google
    "2001:4860:4860::8844", // Google
    "2620:fe::fe",          // Quad9
    "2620:fe::9",           // Quad9
    "2620:119:35::35",      // OpenDNS
    "2620:119:53::53",      // OpenDNS
    "2a10:50c0::ad1:ff",    // AdGuard
    "2a10:50c0::ad2:ff",    // AdGuard
];

/// Generates a complete, atomic nftables ruleset string for FocusWall.
pub fn generate_nftables_ruleset(
    blocked_ipv4: &[Ipv4Addr],
    blocked_ipv6: &[Ipv6Addr],
    doh_blocking: bool,
) -> String {
    let mut ruleset = String::new();
    ruleset.push_str("# FocusWall generated nftables ruleset\n");
    ruleset.push_str("# Atomic replacement of inet focuswall table\n\n");
    ruleset.push_str("table inet focuswall {\n");

    // Blocked IPv4 set
    ruleset.push_str("    set blocked_ipv4 {\n");
    ruleset.push_str("        type ipv4_addr\n");
    if !blocked_ipv4.is_empty() {
        let items: Vec<String> = blocked_ipv4.iter().map(|ip| ip.to_string()).collect();
        ruleset.push_str(&format!("        elements = {{ {} }}\n", items.join(", ")));
    }
    ruleset.push_str("    }\n\n");

    // Blocked IPv6 set
    ruleset.push_str("    set blocked_ipv6 {\n");
    ruleset.push_str("        type ipv6_addr\n");
    if !blocked_ipv6.is_empty() {
        let items: Vec<String> = blocked_ipv6.iter().map(|ip| ip.to_string()).collect();
        ruleset.push_str(&format!("        elements = {{ {} }}\n", items.join(", ")));
    }
    ruleset.push_str("    }\n\n");

    // Public DoH IPv4 set
    ruleset.push_str("    set doh_ipv4 {\n");
    ruleset.push_str("        type ipv4_addr\n");
    if doh_blocking {
        ruleset.push_str(&format!("        elements = {{ {} }}\n", PUBLIC_DOH_IPV4.join(", ")));
    }
    ruleset.push_str("    }\n\n");

    // Public DoH IPv6 set
    ruleset.push_str("    set doh_ipv6 {\n");
    ruleset.push_str("        type ipv6_addr\n");
    if doh_blocking {
        ruleset.push_str(&format!("        elements = {{ {} }}\n", PUBLIC_DOH_IPV6.join(", ")));
    }
    ruleset.push_str("    }\n\n");

    // Output filter chain
    ruleset.push_str("    chain output {\n");
    ruleset.push_str("        type filter hook output priority 0; policy accept;\n\n");

    if !blocked_ipv4.is_empty() {
        ruleset.push_str("        # Drop outbound TCP/UDP to blocked domain IPv4s\n");
        ruleset.push_str("        ip daddr @blocked_ipv4 tcp dport { 80, 443, 8080, 8443 } drop\n");
        ruleset.push_str("        ip daddr @blocked_ipv4 udp dport { 80, 443 } drop\n\n");
    }

    if !blocked_ipv6.is_empty() {
        ruleset.push_str("        # Drop outbound TCP/UDP to blocked domain IPv6s\n");
        ruleset.push_str("        ip6 daddr @blocked_ipv6 tcp dport { 80, 443, 8080, 8443 } drop\n");
        ruleset.push_str("        ip6 daddr @blocked_ipv6 udp dport { 80, 443 } drop\n\n");
    }

    if doh_blocking && (!blocked_ipv4.is_empty() || !blocked_ipv6.is_empty()) {
        ruleset.push_str("        # Prevent DNS bypass via known public DoH/DoT/DNS resolvers while enforcing\n");
        ruleset.push_str("        ip daddr @doh_ipv4 tcp dport { 53, 853, 443 } drop\n");
        ruleset.push_str("        ip daddr @doh_ipv4 udp dport { 53, 853, 443 } drop\n");
        ruleset.push_str("        ip6 daddr @doh_ipv6 tcp dport { 53, 853, 443 } drop\n");
        ruleset.push_str("        ip6 daddr @doh_ipv6 udp dport { 53, 853, 443 } drop\n");
    }

    ruleset.push_str("    }\n");
    ruleset.push_str("}\n");

    ruleset
}

pub struct NftablesManager {
    ruleset_cache_path: PathBuf,
}

impl NftablesManager {
    pub fn new<P: AsRef<Path>>(cache_dir: P) -> Self {
        Self {
            ruleset_cache_path: cache_dir.as_ref().join("nftables_focuswall.nft"),
        }
    }

    /// Generates and applies the nftables ruleset atomically using `nft -f`.
    pub fn apply_rules(
        &self,
        blocked_ipv4: &[Ipv4Addr],
        blocked_ipv6: &[Ipv6Addr],
        doh_blocking: bool,
    ) -> Result<(), FirewallError> {
        let content = generate_nftables_ruleset(blocked_ipv4, blocked_ipv6, doh_blocking);

        if let Some(parent) = self.ruleset_cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&self.ruleset_cache_path, &content)?;

        info!(
            ipv4_count = blocked_ipv4.len(),
            ipv6_count = blocked_ipv6.len(),
            doh_blocking = doh_blocking,
            "Applying atomic nftables ruleset"
        );

        if !crate::is_running_as_root() || cfg!(test) {
            info!("Skipping direct 'nft' execution (running in test/unprivileged mode)");
            return Ok(());
        }

        let output = Command::new("nft")
            .args(["-f", self.ruleset_cache_path.to_str().unwrap_or("")])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                info!("nftables ruleset applied successfully");
                Ok(())
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                warn!("nft command failed: {}", stderr.trim());
                Ok(())
            }
            Err(e) => {
                warn!("nft binary not accessible or missing: {}", e);
                Ok(())
            }
        }
    }

    /// Flushes/deletes the focuswall nftables table.
    pub fn clear_rules(&self) {
        if !crate::is_running_as_root() || cfg!(test) {
            info!("Skipping direct 'nft' flush (running in test/unprivileged mode)");
            return;
        }

        let output = Command::new("nft")
            .args(["delete", "table", "inet", "focuswall"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                info!("Cleared inet focuswall nftables table");
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                warn!("nft delete table returned: {}", stderr.trim());
            }
            Err(e) => {
                warn!("nft binary not accessible: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_generate_ruleset_empty() {
        let ruleset = generate_nftables_ruleset(&[], &[], false);
        assert!(ruleset.contains("table inet focuswall"));
        assert!(ruleset.contains("set blocked_ipv4"));
        assert!(ruleset.contains("set blocked_ipv6"));
        assert!(ruleset.contains("chain output"));
    }

    #[test]
    fn test_generate_ruleset_with_ips_and_doh() {
        let v4 = vec![Ipv4Addr::from_str("142.250.190.46").unwrap()];
        let v6 = vec![Ipv6Addr::from_str("2607:f8b0:4005:805::200e").unwrap()];

        let ruleset = generate_nftables_ruleset(&v4, &v6, true);

        assert!(ruleset.contains("142.250.190.46"));
        assert!(ruleset.contains("2607:f8b0:4005:805::200e"));
        assert!(ruleset.contains("1.1.1.1"));
        assert!(ruleset.contains("8.8.8.8"));
        assert!(ruleset.contains("ip daddr @blocked_ipv4 tcp dport { 80, 443, 8080, 8443 } drop"));
        assert!(ruleset.contains("ip6 daddr @blocked_ipv6 tcp dport { 80, 443, 8080, 8443 } drop"));
        assert!(ruleset.contains("ip daddr @doh_ipv4 tcp dport { 53, 853, 443 } drop"));
    }
}
