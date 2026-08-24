//! Persistent storage layer backed by SQLite.

use std::path::Path;
use chrono::{DateTime, Duration, TimeZone, Utc};
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::policy::{BlockState, Policy, PolicyKind, PolicyStatus};
use crate::schedule::TimeWindow;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum StorageError {
    #[error("SQLite database error: {0}")]
    Sqlite(String),
    #[error("JSON serialization error: {0}")]
    Json(String),
    #[error("System policy cannot be modified or removed")]
    SystemPolicyProtected,
    #[error("Policy not found: id {0}")]
    NotFound(i64),
    #[error("Policy is not in removal_pending state (current state: {0:?})")]
    InvalidStateForRemoval(PolicyStatus),
    #[error("Removal cooldown has not elapsed yet. Remaining: {remaining_seconds} seconds (earliest removal: {earliest_removal_at})")]
    CooldownNotElapsed {
        remaining_seconds: i64,
        earliest_removal_at: String,
    },
    #[error("Rule for domain '{0}' already exists")]
    DuplicateRule(String),
}

impl From<rusqlite::Error> for StorageError {
    fn from(e: rusqlite::Error) -> Self {
        StorageError::Sqlite(e.to_string())
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        StorageError::Json(e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: i64,
    pub ts: String,
    pub event_type: String,
    pub detail: String,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Opens or creates a SQLite database at the specified path and initializes the schema.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(p)?;
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
            Self::map_row_to_policy(row)
        })?;

        let mut policies = Vec::new();
        for p in policy_iter {
            policies.push(p?);
        }
        Ok(policies)
    }

    /// Retrieves a single policy by ID.
    pub fn get_policy_by_id(&self, id: i64) -> Result<Policy, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, domains, schedule_start, schedule_end, timezone, status, created_at,
                    removal_requested_at, removal_cooldown_hours, earliest_removal_at, removal_reason
             FROM policies
             WHERE id = ?1",
        )?;

        stmt.query_row(params![id], |row| Self::map_row_to_policy(row))
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound(id),
                other => StorageError::from(other),
            })
    }

    fn map_row_to_policy(row: &rusqlite::Row) -> rusqlite::Result<Policy> {
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
    }

    /// Adds a new custom website blocking rule.
    pub fn add_custom_rule(
        &self,
        name: &str,
        domains: &[String],
        cooldown_hours: u32,
    ) -> Result<Policy, StorageError> {
        // Check if rule already exists and is active
        let mut check_stmt = self.conn.prepare(
            "SELECT COUNT(*) FROM policies WHERE name = ?1 AND status != 'removed'",
        )?;
        let count: i64 = check_stmt.query_row(params![name], |r| r.get(0))?;
        if count > 0 {
            return Err(StorageError::DuplicateRule(name.to_string()));
        }

        let created_at = Utc::now().to_rfc3339();
        let domains_json = serde_json::to_string(domains)?;

        self.conn.execute(
            "INSERT INTO policies (kind, name, domains, schedule_start, schedule_end, timezone, status, created_at, removal_cooldown_hours)
             VALUES ('custom', ?1, ?2, NULL, NULL, 'system', 'active', ?3, ?4)",
            params![name, domains_json, created_at, cooldown_hours],
        )?;

        let id = self.conn.last_insert_rowid();
        self.log_event(
            "policy_change",
            &format!("Added custom blocked site '{}' (cooldown: {}h, id: {})", name, cooldown_hours, id),
        )?;

        self.get_policy_by_id(id)
    }

    /// Initiates a removal cooldown for a custom rule.
    /// Fails immediately if the rule is a System policy (e.g. YouTube).
    pub fn request_removal(
        &self,
        rule_id: i64,
        reason: Option<&str>,
        cooldown_hours_override: Option<u32>,
    ) -> Result<Policy, StorageError> {
        let policy = self.get_policy_by_id(rule_id)?;

        if policy.kind == PolicyKind::System {
            return Err(StorageError::SystemPolicyProtected);
        }

        if policy.status != PolicyStatus::Active {
            return Err(StorageError::InvalidStateForRemoval(policy.status));
        }

        let cooldown_h = cooldown_hours_override
            .or(policy.removal_cooldown_hours)
            .unwrap_or(24);

        let now = Utc::now();
        let earliest_removal = now + Duration::hours(cooldown_h as i64);
        let earliest_removal_str = earliest_removal.to_rfc3339();
        let now_str = now.to_rfc3339();

        self.conn.execute(
            "UPDATE policies
             SET status = 'removal_pending',
                 removal_requested_at = ?1,
                 removal_cooldown_hours = ?2,
                 earliest_removal_at = ?3,
                 removal_reason = ?4
             WHERE id = ?5",
            params![
                now_str,
                cooldown_h,
                earliest_removal_str,
                reason,
                rule_id,
            ],
        )?;

        self.log_event(
            "removal_requested",
            &format!(
                "Removal requested for policy '{}' (id: {}). Cooldown: {}h. Earliest removal at: {}",
                policy.name, rule_id, cooldown_h, earliest_removal_str
            ),
        )?;

        self.get_policy_by_id(rule_id)
    }

    /// Finalizes removal of a rule if the server cooldown has elapsed.
    pub fn confirm_removal<Tz: TimeZone>(
        &self,
        rule_id: i64,
        now: &DateTime<Tz>,
    ) -> Result<Policy, StorageError> {
        let policy = self.get_policy_by_id(rule_id)?;

        if policy.kind == PolicyKind::System {
            return Err(StorageError::SystemPolicyProtected);
        }

        if policy.status != PolicyStatus::RemovalPending {
            return Err(StorageError::InvalidStateForRemoval(policy.status));
        }

        let earliest_str = policy
            .earliest_removal_at
            .as_ref()
            .ok_or(StorageError::InvalidStateForRemoval(policy.status))?;

        let earliest_dt = DateTime::parse_from_rfc3339(earliest_str)
            .map_err(|_| StorageError::Sqlite("Invalid date format in DB".to_string()))?
            .with_timezone(&Utc);

        let current_utc = now.clone().with_timezone(&Utc);

        if current_utc < earliest_dt {
            let remaining = (earliest_dt - current_utc).num_seconds();
            return Err(StorageError::CooldownNotElapsed {
                remaining_seconds: remaining,
                earliest_removal_at: earliest_str.clone(),
            });
        }

        self.conn.execute(
            "UPDATE policies SET status = 'removed' WHERE id = ?1",
            params![rule_id],
        )?;

        self.log_event(
            "removal_confirmed",
            &format!("Policy '{}' (id: {}) successfully removed after cooldown", policy.name, rule_id),
        )?;

        self.get_policy_by_id(rule_id)
    }

    /// Cancels a pending removal request, returning policy back to active.
    pub fn cancel_removal_request(&self, rule_id: i64) -> Result<Policy, StorageError> {
        let policy = self.get_policy_by_id(rule_id)?;

        if policy.kind == PolicyKind::System {
            return Err(StorageError::SystemPolicyProtected);
        }

        if policy.status != PolicyStatus::RemovalPending {
            return Err(StorageError::InvalidStateForRemoval(policy.status));
        }

        self.conn.execute(
            "UPDATE policies
             SET status = 'active',
                 removal_requested_at = NULL,
                 earliest_removal_at = NULL,
                 removal_reason = NULL
             WHERE id = ?1",
            params![rule_id],
        )?;

        self.log_event(
            "policy_change",
            &format!("Cancelled removal request for policy '{}' (id: {})", policy.name, rule_id),
        )?;

        self.get_policy_by_id(rule_id)
    }

    /// Retrieves recent audit log entries.
    pub fn get_audit_logs(&self, limit: u32) -> Result<Vec<AuditLogEntry>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, ts, event_type, detail FROM audit_log ORDER BY id DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit], |r| {
            Ok(AuditLogEntry {
                id: r.get(0)?,
                ts: r.get(1)?,
                event_type: r.get(2)?,
                detail: r.get(3)?,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
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
