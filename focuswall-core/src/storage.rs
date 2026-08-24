//! Persistent storage layer backed by SQLite.

use std::path::Path;
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection, Result};
use thiserror::Error;

use crate::policy::{BlockState, Policy, PolicyKind, PolicyStatus};
use crate::schedule::TimeWindow;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("SQLite database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("System policy cannot be modified or removed")]
    SystemPolicyProtected,
    #[error("Policy not found: id {0}")]
    NotFound(i64),
}

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Opens or creates a SQLite database at the specified path and initializes the schema.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init_schema()?;
        db.seed_system_policies()?;
        Ok(db)
    }

    /// Opens an in-memory database (useful for unit tests).
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        db.seed_system_policies()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS policies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL CHECK (kind IN ('system','custom')),
                name TEXT NOT NULL,
                domains TEXT NOT NULL,
                schedule_start TEXT,
                schedule_end TEXT,
                timezone TEXT NOT NULL DEFAULT 'system',
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT NOT NULL,
                removal_requested_at TEXT,
                removal_cooldown_hours INTEGER,
                earliest_removal_at TEXT,
                removal_reason TEXT
            );

            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts TEXT NOT NULL,
                event_type TEXT NOT NULL,
                detail TEXT
            );
            ",
        )?;
        Ok(())
    }

    /// Seeds the default YouTube policy if it does not already exist.
    fn seed_system_policies(&self) -> Result<(), StorageError> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM policies WHERE kind = 'system' AND name = 'youtube'")?;
        let count: i64 = stmt.query_row([], |r| r.get(0))?;
        if count == 0 {
            let yt = Policy::youtube_system_policy();
            let domains_json = serde_json::to_string(&yt.domains)?;
            let (start, end) = match &yt.schedule {
                Some(w) => (Some(w.start.format("%H:%M").to_string()), Some(w.end.format("%H:%M").to_string())),
                None => (None, None),
            };

            self.conn.execute(
                "INSERT INTO policies (kind, name, domains, schedule_start, schedule_end, timezone, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    "system",
                    yt.name,
                    domains_json,
                    start,
                    end,
                    yt.timezone,
                    "active",
                    yt.created_at,
                ],
            )?;

            self.log_event("daemon_start", "Database initialized and YouTube system policy seeded")?;
        }
        Ok(())
    }

    /// Writes an audit log entry.
    pub fn log_event(&self, event_type: &str, detail: &str) -> Result<(), StorageError> {
        let ts = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO audit_log (ts, event_type, detail) VALUES (?1, ?2, ?3)",
            params![ts, event_type, detail],
        )?;
        Ok(())
    }

    /// Retrieves all non-removed policies.
    pub fn get_active_policies(&self) -> Result<Vec<Policy>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, domains, schedule_start, schedule_end, timezone, status, created_at,
                    removal_requested_at, removal_cooldown_hours, earliest_removal_at, removal_reason
             FROM policies
             WHERE status != 'removed'
             ORDER BY id ASC",
        )?;

        let policy_iter = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let kind_str: String = row.get(1)?;
            let name: String = row.get(2)?;
            let domains_json: String = row.get(3)?;
            let schedule_start: Option<String> = row.get(4)?;
            let schedule_end: Option<String> = row.get(5)?;
            let timezone: String = row.get(6)?;
            let status_str: String = row.get(7)?;
            let created_at: String = row.get(8)?;
            let removal_requested_at: Option<String> = row.get(9)?;
            let removal_cooldown_hours: Option<u32> = row.get(10)?;
            let earliest_removal_at: Option<String> = row.get(11)?;
            let removal_reason: Option<String> = row.get(12)?;

            let kind = match kind_str.as_str() {
                "system" => PolicyKind::System,
                _ => PolicyKind::Custom,
            };

            let status = match status_str.as_str() {
                "removal_pending" => PolicyStatus::RemovalPending,
                "removed" => PolicyStatus::Removed,
                _ => PolicyStatus::Active,
            };

            let domains: Vec<String> = serde_json::from_str(&domains_json).unwrap_or_default();

            let schedule = match (schedule_start, schedule_end) {
                (Some(s), Some(e)) => {
                    let start_t = chrono::NaiveTime::parse_from_str(&s, "%H:%M")
                        .or_else(|_| chrono::NaiveTime::parse_from_str(&s, "%H:%M:%S"))
                        .ok();
                    let end_t = chrono::NaiveTime::parse_from_str(&e, "%H:%M")
                        .or_else(|_| chrono::NaiveTime::parse_from_str(&e, "%H:%M:%S"))
                        .ok();
                    match (start_t, end_t) {
                        (Some(st), Some(et)) => Some(TimeWindow::new(st, et)),
                        _ => None,
                    }
                }
                _ => None,
            };

            Ok(Policy {
                id: Some(id),
                kind,
                name,
                domains,
                schedule,
                timezone,
                status,
                created_at,
                removal_requested_at,
                removal_cooldown_hours,
                earliest_removal_at,
                removal_reason,
            })
        })?;

        let mut policies = Vec::new();
        for p in policy_iter {
            policies.push(p?);
        }
        Ok(policies)
    }

    /// Evaluates which domains are currently BLOCKED across all active policies at time `now`.
    pub fn get_blocked_domains<Tz: TimeZone>(&self, now: &DateTime<Tz>) -> Result<Vec<String>, StorageError> {
        let policies = self.get_active_policies()?;
        let mut blocked_domains = Vec::new();
        for policy in policies {
            if policy.evaluate(now) == BlockState::Blocked {
                for domain in policy.domains {
                    if !blocked_domains.contains(&domain) {
                        blocked_domains.push(domain);
                    }
                }
            }
        }
        Ok(blocked_domains)
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
    fn test_db_seeding_and_evaluation() {
        let db = Database::open_in_memory().expect("in-memory db opens");
        let policies = db.get_active_policies().expect("retrieves policies");
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].name, "youtube");
        assert_eq!(policies[0].kind, PolicyKind::System);

        // Outside window (e.g. 19:30) -> YouTube domains must be in blocked_domains
        let blocked = db.get_blocked_domains(&make_time(19, 30, 0)).expect("evaluated blocked domains");
        assert!(blocked.contains(&"youtube.com".to_string()));
        assert!(blocked.contains(&"googlevideo.com".to_string()));

        // Inside window (e.g. 20:30) -> blocked domains list is empty
        let allowed = db.get_blocked_domains(&make_time(20, 30, 0)).expect("evaluated allowed domains");
        assert!(allowed.is_empty());
    }

    #[test]
    fn test_audit_logging() {
        let db = Database::open_in_memory().expect("in-memory db opens");
        db.log_event("policy_change", "Added test rule").expect("logs event");

        let mut stmt = db.conn.prepare("SELECT event_type, detail FROM audit_log ORDER BY id DESC LIMIT 1").unwrap();
        let (evt, detail): (String, String) = stmt.query_row([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(evt, "policy_change");
        assert_eq!(detail, "Added test rule");
    }
}
