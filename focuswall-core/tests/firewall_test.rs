use focuswall_core::{
    generate_nftables_ruleset, NftablesManager, PUBLIC_DOH_IPV4, PUBLIC_DOH_IPV6,
};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use tempfile::tempdir;

#[test]
fn test_nftables_ruleset_structure_and_ports() {
    let ipv4_list = vec![
        Ipv4Addr::from_str("172.217.16.206").unwrap(),
        Ipv4Addr::from_str("142.250.190.46").unwrap(),
    ];
    let ipv6_list = vec![
        Ipv6Addr::from_str("2607:f8b0:4005:805::200e").unwrap(),
    ];

    let ruleset = generate_nftables_ruleset(&ipv4_list, &ipv6_list, true);

    // Verify table definition
    assert!(ruleset.contains("table inet focuswall {"));

    // Verify elements in sets
    assert!(ruleset.contains("172.217.16.206"));
    assert!(ruleset.contains("142.250.190.46"));
    assert!(ruleset.contains("2607:f8b0:4005:805::200e"));

    // Verify DoH sets
    for doh_v4 in PUBLIC_DOH_IPV4 {
        assert!(ruleset.contains(doh_v4), "Expected DoH IPv4 {} in ruleset", doh_v4);
    }
    for doh_v6 in PUBLIC_DOH_IPV6 {
        assert!(ruleset.contains(doh_v6), "Expected DoH IPv6 {} in ruleset", doh_v6);
    }

    // Verify filter rules
    assert!(ruleset.contains("ip daddr @blocked_ipv4 tcp dport { 80, 443, 8080, 8443 } drop"));
    assert!(ruleset.contains("ip daddr @blocked_ipv4 udp dport { 80, 443 } drop"));
    assert!(ruleset.contains("ip6 daddr @blocked_ipv6 tcp dport { 80, 443, 8080, 8443 } drop"));
    assert!(ruleset.contains("ip daddr @doh_ipv4 tcp dport { 53, 853, 443 } drop"));
}

#[test]
fn test_nftables_manager_file_creation() {
    let dir = tempdir().unwrap();
    let manager = NftablesManager::new(dir.path());

    let ipv4 = vec![Ipv4Addr::from_str("1.2.3.4").unwrap()];
    let ipv6 = vec![];

    manager.apply_rules(&ipv4, &ipv6, false).unwrap();

    let cache_file = dir.path().join("nftables_focuswall.nft");
    assert!(cache_file.exists());
    let content = std::fs::read_to_string(cache_file).unwrap();
    assert!(content.contains("1.2.3.4"));
}
