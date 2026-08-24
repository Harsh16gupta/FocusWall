use chrono::{Duration, Utc};
use focuswall_core::{Database, PolicyKind, PolicyStatus, StorageError};

#[test]
fn test_custom_rule_lifecycle_and_cooldown() {
    let db = Database::open_in_memory().expect("in-memory db opens");

    // 1. Add custom rule for reddit.com
    let domains = vec!["reddit.com".to_string(), "www.reddit.com".to_string()];
    let rule = db.add_custom_rule("reddit.com", &domains, 24).unwrap();
    assert_eq!(rule.name, "reddit.com");
    assert_eq!(rule.kind, PolicyKind::Custom);
    assert_eq!(rule.status, PolicyStatus::Active);
    assert_eq!(rule.removal_cooldown_hours, Some(24));

    let rule_id = rule.id.unwrap();

    // 2. Request removal
    let pending_rule = db.request_removal(rule_id, Some("Need it for research"), None).unwrap();
    assert_eq!(pending_rule.status, PolicyStatus::RemovalPending);
    assert!(pending_rule.removal_requested_at.is_some());
    assert!(pending_rule.earliest_removal_at.is_some());

    // 3. Premature confirmation attempt (e.g. immediately or 1 hour later) -> MUST FAIL
    let now = Utc::now();
    let one_hour_later = now + Duration::hours(1);
    let early_res = db.confirm_removal(rule_id, &one_hour_later);
    assert!(matches!(early_res, Err(StorageError::CooldownNotElapsed { .. })));

    // 4. Confirmation after cooldown elapsed (e.g. 24 hours + 1 minute) -> MUST SUCCEED
    let after_cooldown = now + Duration::hours(24) + Duration::minutes(1);
    let confirmed_rule = db.confirm_removal(rule_id, &after_cooldown).unwrap();
    assert_eq!(confirmed_rule.status, PolicyStatus::Removed);

    // Verify it is no longer in active policies
    let active_policies = db.get_active_policies().unwrap();
    assert_eq!(active_policies.len(), 1); // Only system YouTube remains
}

#[test]
fn test_youtube_system_policy_cannot_be_removed() {
    let db = Database::open_in_memory().expect("in-memory db opens");
    let policies = db.get_active_policies().unwrap();
    let yt_policy = policies.iter().find(|p| p.name == "youtube").unwrap();
    let yt_id = yt_policy.id.unwrap();

    // Attempting request_removal on YouTube MUST fail with SystemPolicyProtected
    let req_res = db.request_removal(yt_id, None, None);
    assert_eq!(req_res, Err(StorageError::SystemPolicyProtected));

    // Attempting confirm_removal on YouTube MUST fail with SystemPolicyProtected
    let conf_res = db.confirm_removal(yt_id, &Utc::now());
    assert_eq!(conf_res, Err(StorageError::SystemPolicyProtected));

    // Attempting cancel_removal on YouTube MUST fail with SystemPolicyProtected
    let cancel_res = db.cancel_removal_request(yt_id);
    assert_eq!(cancel_res, Err(StorageError::SystemPolicyProtected));
}

#[test]
fn test_cancel_removal_request() {
    let db = Database::open_in_memory().expect("in-memory db opens");
    let domains = vec!["twitter.com".to_string()];
    let rule = db.add_custom_rule("twitter.com", &domains, 24).unwrap();
    let rule_id = rule.id.unwrap();

    // Request removal
    db.request_removal(rule_id, None, None).unwrap();
    let pending = db.get_policy_by_id(rule_id).unwrap();
    assert_eq!(pending.status, PolicyStatus::RemovalPending);

    // Cancel removal
    let restored = db.cancel_removal_request(rule_id).unwrap();
    assert_eq!(restored.status, PolicyStatus::Active);
    assert!(restored.earliest_removal_at.is_none());
}
