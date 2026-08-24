//! Domain IP resolution utility for nftables IP-level backstop.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use std::collections::HashSet;
use tracing::warn;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedIps {
    pub ipv4: Vec<Ipv4Addr>,
    pub ipv6: Vec<Ipv6Addr>,
}

/// Resolves a list of domain names into unique IPv4 and IPv6 addresses.
/// Ignores 0.0.0.0 and loopback addresses to avoid capturing sinkholed local addresses.
pub fn resolve_domain_ips(domains: &[String]) -> ResolvedIps {
    let mut ipv4_set = HashSet::new();
    let mut ipv6_set = HashSet::new();

    for domain in domains {
        let trimmed = domain.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Use standard port 443 for socket addr resolution
        let host_port = format!("{}:443", trimmed);
        match host_port.to_socket_addrs() {
            Ok(addrs) => {
                for addr in addrs {
                    match addr.ip() {
                        IpAddr::V4(v4) => {
                            if !v4.is_loopback() && !v4.is_unspecified() {
                                ipv4_set.insert(v4);
                            }
                        }
                        IpAddr::V6(v6) => {
                            if !v6.is_loopback() && !v6.is_unspecified() {
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
