use focuswall_core::DnsManager;
use std::fs;
use tempfile::NamedTempFile;

#[test]
fn test_dns_manager_reconciliation_cycle() {
    let temp_file = NamedTempFile::new().unwrap();
    let config_path = temp_file.path().to_path_buf();
    let manager = DnsManager::new(&config_path);

    // Initial state: Blocked
    let blocked_domains = vec![
        "youtube.com".to_string(),
        "googlevideo.com".to_string(),
    ];
    manager.apply_blocked_domains(&blocked_domains).unwrap();

    let content_blocked = fs::read_to_string(&config_path).unwrap();
    assert!(content_blocked.contains("address=/youtube.com/0.0.0.0"));
    assert!(content_blocked.contains("address=/youtube.com/::"));
    assert!(content_blocked.contains("address=/googlevideo.com/0.0.0.0"));

    // Next state: Window opens (Allowed -> Empty blocked domains list)
    manager.apply_blocked_domains(&[]).unwrap();
    let content_allowed = fs::read_to_string(&config_path).unwrap();
    assert!(content_allowed.contains("# No active domains blocked at this time."));
    assert!(!content_allowed.contains("address=/youtube.com/0.0.0.0"));

    // Next state: Window closes (Blocked again)
    manager.apply_blocked_domains(&blocked_domains).unwrap();
    let content_reblocked = fs::read_to_string(&config_path).unwrap();
    assert!(content_reblocked.contains("address=/youtube.com/0.0.0.0"));
}
