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

#[test]
fn test_daily_1hour_quota_lifecycle() {
    let db = Database::open_in_memory().expect("in-memory db opens");
    let now = Utc::now();

    // 1. Initially, YouTube has 60m remaining and is not in an active session
    let quota = db.get_quota_status("youtube", &now).unwrap();
    assert_eq!(quota.daily_quota_seconds, 3600);
    assert_eq!(quota.used_seconds_today, 0);
    assert_eq!(quota.remaining_seconds_today, 3600);
    assert!(!quota.is_session_active);
    assert!(!quota.is_exhausted);

    // Initial blocked domains must include YouTube
    let blocked = db.get_blocked_domains(&now).unwrap();
    assert!(blocked.contains(&"youtube.com".to_string()));

    // 2. Start a 20-minute unlock session
    let session = db.start_unlock_session("youtube", Some(20), &now).unwrap();
    assert!(session.is_session_active);
    assert_eq!(session.session_target_seconds, Some(1200));

    // YouTube is now unblocked
    let blocked_after_start = db.get_blocked_domains(&now).unwrap();
    assert!(!blocked_after_start.contains(&"youtube.com".to_string()));

    // 3. Advance time by 10 minutes and pause / stop session
    let ten_mins_later = now + Duration::minutes(10);
    let paused = db.stop_unlock_session("youtube", &ten_mins_later).unwrap();
    assert!(!paused.is_session_active);
    assert_eq!(paused.used_seconds_today, 600);
    assert_eq!(paused.remaining_seconds_today, 3000); // 50m remaining

    // YouTube is blocked again after pausing
    let blocked_after_pause = db.get_blocked_domains(&ten_mins_later).unwrap();
    assert!(blocked_after_pause.contains(&"youtube.com".to_string()));

    // 4. Resume session for remaining 50m
    let session2 = db.start_unlock_session("youtube", None, &ten_mins_later).unwrap();
    assert!(session2.is_session_active);

    // 5. Advance time by 51 minutes (exceeds remaining 50m budget) -> Auto-exhausted!
    let sixty_one_mins_later = ten_mins_later + Duration::minutes(51);
    let _ = db.record_usage_tick("youtube", &sixty_one_mins_later).unwrap();
    let final_quota = db.get_quota_status("youtube", &sixty_one_mins_later).unwrap();
    assert_eq!(final_quota.used_seconds_today, 3600);
    assert_eq!(final_quota.remaining_seconds_today, 0);
    assert!(final_quota.is_exhausted);
    assert!(!final_quota.is_session_active);

    // Attempting to start a new session must fail with QuotaExhausted
    let fail_res = db.start_unlock_session("youtube", Some(15), &sixty_one_mins_later);
    assert!(fail_res.is_err());

    // YouTube is strictly blocked
    let final_blocked = db.get_blocked_domains(&sixty_one_mins_later).unwrap();
    assert!(final_blocked.contains(&"youtube.com".to_string()));
}
