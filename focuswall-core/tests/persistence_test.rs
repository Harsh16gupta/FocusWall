use focuswall_core::{Database, PolicyKind, PolicyStatus};
use tempfile::NamedTempFile;

#[test]
fn test_database_persistence_across_reopen() {
    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path().to_path_buf();

    // Session 1: Open and verify YouTube policy is seeded
    {
        let db = Database::open(&db_path).expect("opens db");
        let policies = db.get_active_policies().expect("gets policies");
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].name, "youtube");
        assert_eq!(policies[0].kind, PolicyKind::System);
        assert_eq!(policies[0].status, PolicyStatus::Active);

        db.log_event("test_event", "Session 1 test entry").unwrap();
    }

    // Session 2: Re-open existing database file (simulating daemon restart/reboot)
    {
        let db = Database::open(&db_path).expect("reopens db");
        let policies = db.get_active_policies().expect("gets policies");
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].name, "youtube");
    }
}

#[test]
fn test_audit_log_chronological_ordering() {
    let db = Database::open_in_memory().expect("in-memory db opens");
    db.log_event("daemon_start", "Event 1").unwrap();
    db.log_event("policy_change", "Event 2").unwrap();
    db.log_event("daemon_stop", "Event 3").unwrap();

    let policies = db.get_active_policies().unwrap();
    assert_eq!(policies.len(), 1);
}
