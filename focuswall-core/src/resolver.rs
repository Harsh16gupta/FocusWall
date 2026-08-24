//! Domain IP resolution utility for nftables IP-level backstop.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use std::collections::HashSet;
use tracing::warn;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedIps {
    pub ipv4: Vec<Ipv4Addr>,
    pub ipv6: Vec<Ipv6Addr>,
}

/// Checks if an IPv4 address is safe to block in the firewall without impacting local system networking.
pub fn is_safe_firewall_target_v4(v4: &Ipv4Addr) -> bool {
    !v4.is_loopback()
        && !v4.is_unspecified()
        && !v4.is_broadcast()
        && !v4.is_multicast()
        && !v4.is_link_local()
        && !v4.is_private() // Do not block private LAN IPs (192.168.x.x, 10.x.x.x, 172.16-31.x.x)
}

/// Checks if an IPv6 address is safe to block in the firewall without impacting local system networking.
pub fn is_safe_firewall_target_v6(v6: &Ipv6Addr) -> bool {
    !v6.is_loopback()
        && !v6.is_unspecified()
        && !v6.is_multicast()
        && !((v6.segments()[0] & 0xffc0) == 0xfe80) // Link-local fe80::/10
}

/// Resolves a list of domain names into unique IPv4 and IPv6 addresses.
/// Filters out loopback, private LAN, multicast, and broadcast addresses to prevent self-lockouts.
pub fn resolve_domain_ips(domains: &[String]) -> ResolvedIps {
    let mut ipv4_set = HashSet::new();
    let mut ipv6_set = HashSet::new();

    for domain in domains {
        let trimmed = domain.trim();
        if trimmed.is_empty() || !crate::domain::is_valid_hostname(trimmed) {
            continue;
        }

        // Use standard port 443 for socket addr resolution
        let host_port = format!("{}:443", trimmed);
        match host_port.to_socket_addrs() {
            Ok(addrs) => {
                for addr in addrs {
                    match addr.ip() {
                        IpAddr::V4(v4) => {
                            if is_safe_firewall_target_v4(&v4) {
                                ipv4_set.insert(v4);
                            }
                        }
                        IpAddr::V6(v6) => {
                            if is_safe_firewall_target_v6(&v6) {
                                ipv6_set.insert(v6);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!("DNS resolution failed for domain '{}': {}", trimmed, e);
            }
        }
    }

    let mut ipv4: Vec<Ipv4Addr> = ipv4_set.into_iter().collect();
    let mut ipv6: Vec<Ipv6Addr> = ipv6_set.into_iter().collect();
    ipv4.sort();
    ipv6.sort();

    ResolvedIps { ipv4, ipv6 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_empty_domains() {
        let res = resolve_domain_ips(&[]);
        assert!(res.ipv4.is_empty());
        assert!(res.ipv6.is_empty());
    }

    #[test]
    fn test_resolve_localhost() {
        // localhost should be filtered out because it's loopback
        let res = resolve_domain_ips(&["localhost".to_string()]);
        assert!(res.ipv4.is_empty());
        assert!(res.ipv6.is_empty());
    }
}
