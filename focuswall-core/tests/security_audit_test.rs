use focuswall_core::{
    generate_dnsmasq_config, is_safe_firewall_target_v4, is_safe_firewall_target_v6,
    is_valid_hostname, normalize_domain_input, Database, StorageError,
};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

#[test]
fn test_hostname_injection_attacks_are_rejected() {
    let malicious_inputs = [
        "youtube.com\nserver=/evil.com/1.2.3.4",
        "reddit.com\r\naddress=/bad.org/0.0.0.0",
        "example.com; rm -rf /",
        "google.com`id`",
        "twitter.com$(whoami)",
        "sub..domain.com",
        ".starts-with-dot.com",
        "ends-with-dot.com.",
        "-starts-with-dash.com",
        "ends-with-dash-.com",
        "space in domain.com",
        "bad/path/domain.com",
        "bad\\domain.com",
    ];

    for input in &malicious_inputs {
        assert!(
            !is_valid_hostname(input),
            "Expected input '{}' to be rejected by is_valid_hostname",
            input
        );
        let res = normalize_domain_input(input);
        assert!(
            res.is_err(),
            "Expected input '{}' to be rejected by normalize_domain_input",
            input
        );
    }
}

#[test]
fn test_dns_generator_sanitizes_config_output() {
    let raw_domains = vec![
        "valid-site.com".to_string(),
        "malicious.com\nserver=/attacker.com/1.1.1.1".to_string(),
        "another-valid.org".to_string(),
    ];

    let conf = generate_dnsmasq_config(&raw_domains);

    // Valid domains must be present
    assert!(conf.contains("address=/valid-site.com/0.0.0.0"));
    assert!(conf.contains("address=/another-valid.org/0.0.0.0"));

    // Malicious injected directive MUST NOT be present
    assert!(!conf.contains("server=/attacker.com/1.1.1.1"));
}

#[test]
fn test_safe_firewall_target_ip_filtering() {
    // Dangerous IPs that must NEVER be added to firewall drop lists
    let dangerous_v4 = [
        "127.0.0.1",        // Loopback
        "127.0.0.53",       // systemd-resolved loopback
        "0.0.0.0",          // Unspecified
        "255.255.255.255",  // Broadcast
        "224.0.0.1",        // Multicast
        "169.254.1.1",      // Link-local
        "192.168.1.1",      // Private LAN
        "10.0.0.1",         // Private LAN
        "172.16.0.1",       // Private LAN
    ];

    for ip_str in &dangerous_v4 {
        let ip = Ipv4Addr::from_str(ip_str).unwrap();
        assert!(
            !is_safe_firewall_target_v4(&ip),
            "IP {} must be marked UNSAFE for firewall blocking",
            ip_str
        );
    }

    // Safe public IPs that CAN be blocked
    let safe_v4 = [
        "142.250.190.46",  // YouTube public IP
        "151.101.1.140",   // Reddit public IP
    ];

    for ip_str in &safe_v4 {
        let ip = Ipv4Addr::from_str(ip_str).unwrap();
        assert!(
            is_safe_firewall_target_v4(&ip),
            "Public IP {} should be safe for firewall blocking",
            ip_str
        );
    }

    // IPv6 checks
    let loopback_v6 = Ipv6Addr::from_str("::1").unwrap();
    let unspec_v6 = Ipv6Addr::from_str("::").unwrap();
    let link_local_v6 = Ipv6Addr::from_str("fe80::1").unwrap();
    let public_v6 = Ipv6Addr::from_str("2607:f8b0:4005:805::200e").unwrap();

    assert!(!is_safe_firewall_target_v6(&loopback_v6));
    assert!(!is_safe_firewall_target_v6(&unspec_v6));
    assert!(!is_safe_firewall_target_v6(&link_local_v6));
    assert!(is_safe_firewall_target_v6(&public_v6));
}

#[test]
fn test_system_youtube_policy_cannot_be_removed() {
    let db = Database::open_in_memory().unwrap();
    let policies = db.get_active_policies().unwrap();
    let yt_policy = policies.iter().find(|p| p.name == "youtube").unwrap();

    // Removal attempt on YouTube system rule must fail with SystemPolicyProtected
    let removal_res = db.request_removal(yt_policy.id.unwrap(), Some("bypass attempt"), None);
    assert_eq!(removal_res, Err(StorageError::SystemPolicyProtected));
}

#[test]
fn test_storage_parameter_clamping() {
    let db = Database::open_in_memory().unwrap();

    // Adding rule with extreme cooldown (0 or 999999) must be clamped safely
    let policy = db
        .add_custom_rule(
            "custom-test.com",
            &["custom-test.com".to_string(), "www.custom-test.com".to_string()],
            0, // below min -> should clamp to 1
        )
        .unwrap();

    assert_eq!(policy.removal_cooldown_hours, Some(1));
}
