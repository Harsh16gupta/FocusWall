//! Policy types, domain lists, and state definitions.

use chrono::{DateTime, TimeZone};
use serde::{Deserialize, Serialize};

use crate::domain::YOUTUBE_DOMAINS;
use crate::schedule::TimeWindow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockState {
    Allowed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyKind {
    System,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyStatus {
    Active,
    RemovalPending,
    Removed,
}

/// Represents an active or pending policy rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    pub id: Option<i64>,
    pub kind: PolicyKind,
    pub name: String,
    pub domains: Vec<String>,
    pub schedule: Option<TimeWindow>,
    pub timezone: String,
    pub status: PolicyStatus,
    pub created_at: String,
    pub removal_requested_at: Option<String>,
    pub removal_cooldown_hours: Option<u32>,
    pub earliest_removal_at: Option<String>,
    pub removal_reason: Option<String>,
}

impl Policy {
    /// Creates the standard system policy for YouTube.
    pub fn youtube_system_policy() -> Self {
        Self {
            id: None,
            kind: PolicyKind::System,
            name: "youtube".to_string(),
            domains: YOUTUBE_DOMAINS.iter().map(|&d| d.to_string()).collect(),
            schedule: Some(TimeWindow::youtube_window()),
            timezone: "system".to_string(),
            status: PolicyStatus::Active,
            created_at: chrono::Utc::now().to_rfc3339(),
            removal_requested_at: None,
            removal_cooldown_hours: None,
            earliest_removal_at: None,
            removal_reason: None,
        }
    }

    /// Evaluates the block state of this policy at the given time.
    ///
    /// Rules:
    /// - If status is `Removed`, state is `Allowed`.
    /// - If status is `RemovalPending` or `Active`:
    ///   - If a schedule is present, evaluates against the schedule window.
    ///   - If no schedule is present, it is blocked 24/7 (`Blocked`).
    pub fn evaluate<Tz: TimeZone>(&self, now: &DateTime<Tz>) -> BlockState {
        if self.status == PolicyStatus::Removed {
            return BlockState::Allowed;
        }

        match &self.schedule {
            Some(window) => window.evaluate(now),
            None => BlockState::Blocked,
        }
    }

    /// Checks whether this policy is eligible for removal request.
    /// System policies (e.g. YouTube) are NEVER eligible for removal.
    pub fn can_request_removal(&self) -> bool {
        self.kind == PolicyKind::Custom && self.status == PolicyStatus::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveTime, Utc};

    fn make_time(hour: u32, min: u32, sec: u32) -> DateTime<Utc> {
        let date = NaiveDate::from_ymd_opt(2026, 8, 24).unwrap();
        let time = NaiveTime::from_hms_opt(hour, min, sec).unwrap();
        Utc.from_utc_datetime(&date.and_time(time))
    }

    #[test]
    fn test_youtube_system_policy_eval() {
        let policy = Policy::youtube_system_policy();
        assert_eq!(policy.kind, PolicyKind::System);
        assert!(!policy.can_request_removal());

        assert_eq!(policy.evaluate(&make_time(19, 59, 59)), BlockState::Blocked);
        assert_eq!(policy.evaluate(&make_time(20, 0, 0)), BlockState::Allowed);
        assert_eq!(policy.evaluate(&make_time(20, 59, 59)), BlockState::Allowed);
        assert_eq!(policy.evaluate(&make_time(21, 0, 0)), BlockState::Blocked);
    }

    #[test]
    fn test_custom_policy_removal_pending_still_blocks() {
        let mut custom = Policy {
            id: Some(1),
            kind: PolicyKind::Custom,
            name: "reddit.com".to_string(),
            domains: vec!["reddit.com".to_string()],
            schedule: None, // 24/7 block
            timezone: "system".to_string(),
            status: PolicyStatus::RemovalPending,
            created_at: Utc::now().to_rfc3339(),
            removal_requested_at: Some(Utc::now().to_rfc3339()),
            removal_cooldown_hours: Some(24),
            earliest_removal_at: None,
            removal_reason: None,
        };

        // While removal is pending, it must STILL be BLOCKED
        assert_eq!(custom.evaluate(&make_time(12, 0, 0)), BlockState::Blocked);
        assert_eq!(custom.evaluate(&make_time(20, 30, 0)), BlockState::Blocked);

        // Only when status transitions to Removed does it become Allowed
        custom.status = PolicyStatus::Removed;
        assert_eq!(custom.evaluate(&make_time(12, 0, 0)), BlockState::Allowed);
    }
}
