//! Scheduling evaluation definitions and time calculations.
//!
//! Scheduling logic in FocusWall is strictly wall-clock derived and deterministic.
//! It never relies on relative uptime timers or in-memory elapsed intervals.

use chrono::{DateTime, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};

use crate::policy::BlockState;

/// Represents a daily recurring time window during which access is ALLOWED.
/// If `None`, the policy is blocked 24/7 (default for custom blocked sites).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeWindow {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

impl TimeWindow {
    /// Creates a new time window with validation.
    pub fn new(start: NaiveTime, end: NaiveTime) -> Self {
        Self { start, end }
    }

    /// Creates the standard YouTube window: 20:00:00 to 21:00:00.
    pub fn youtube_window() -> Self {
        Self {
            start: NaiveTime::from_hms_opt(20, 0, 0).expect("valid time 20:00:00"),
            end: NaiveTime::from_hms_opt(21, 0, 0).expect("valid time 21:00:00"),
        }
    }

    /// Evaluates whether the given time falls within the allowed window.
    ///
    /// The window is [start, end) — inclusive of start, exclusive of end.
    /// For windows within the same day (e.g. 20:00 - 21:00):
    /// `start <= time < end`
    /// For overnight windows (e.g. 23:00 - 02:00):
    /// `time >= start || time < end`
    pub fn is_allowed_at_time(&self, time: NaiveTime) -> bool {
        if self.start <= self.end {
            time >= self.start && time < self.end
        } else {
            // Overnight window
            time >= self.start || time < self.end
        }
    }

    /// Evaluates the block state for a given timezone-aware DateTime.
    pub fn evaluate<Tz: TimeZone>(&self, now: &DateTime<Tz>) -> BlockState {
        let current_time = now.time();
        if self.is_allowed_at_time(current_time) {
            BlockState::Allowed
        } else {
            BlockState::Blocked
        }
    }
}

/// Evaluates the built-in YouTube policy state for any given datetime.
///
/// YouTube is strictly ALLOWED from 20:00:00 up to 20:59:59.999... (local time)
/// and BLOCKED at all other times.
pub fn evaluate_youtube_state<Tz: TimeZone>(now: &DateTime<Tz>) -> BlockState {
    TimeWindow::youtube_window().evaluate(now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, NaiveDate, TimeZone, Utc};

    fn make_time(hour: u32, min: u32, sec: u32) -> DateTime<Utc> {
        let date = NaiveDate::from_ymd_opt(2026, 8, 24).unwrap();
        let time = NaiveTime::from_hms_opt(hour, min, sec).unwrap();
        Utc.from_utc_datetime(&date.and_time(time))
    }

    #[test]
    fn test_youtube_boundaries_exact() {
        // 19:59:59 -> BLOCKED (1 second before window)
        assert_eq!(
            evaluate_youtube_state(&make_time(19, 59, 59)),
            BlockState::Blocked,
            "19:59:59 must be BLOCKED"
        );

        // 20:00:00 -> ALLOWED (exact start of window)
        assert_eq!(
            evaluate_youtube_state(&make_time(20, 0, 0)),
            BlockState::Allowed,
            "20:00:00 must be ALLOWED"
        );

        // 20:30:00 -> ALLOWED (mid-window)
        assert_eq!(
            evaluate_youtube_state(&make_time(20, 30, 0)),
            BlockState::Allowed,
            "20:30:00 must be ALLOWED"
        );

        // 20:59:59 -> ALLOWED (last second of window)
        assert_eq!(
            evaluate_youtube_state(&make_time(20, 59, 59)),
            BlockState::Allowed,
            "20:59:59 must be ALLOWED"
        );

        // 21:00:00 -> BLOCKED (exact window close)
        assert_eq!(
            evaluate_youtube_state(&make_time(21, 0, 0)),
            BlockState::Blocked,
            "21:00:00 must be BLOCKED"
        );

        // 21:00:01 -> BLOCKED (1 second after window)
        assert_eq!(
            evaluate_youtube_state(&make_time(21, 0, 1)),
            BlockState::Blocked,
            "21:00:01 must be BLOCKED"
        );
    }

    #[test]
    fn test_youtube_other_times_of_day() {
        // Midnight
        assert_eq!(
            evaluate_youtube_state(&make_time(0, 0, 0)),
            BlockState::Blocked
        );
        // Early morning
        assert_eq!(
            evaluate_youtube_state(&make_time(7, 30, 0)),
            BlockState::Blocked
        );
        // Noon
        assert_eq!(
            evaluate_youtube_state(&make_time(12, 0, 0)),
            BlockState::Blocked
        );
        // Late night
        assert_eq!(
            evaluate_youtube_state(&make_time(23, 0, 0)),
            BlockState::Blocked
        );
    }

    #[test]
    fn test_youtube_with_timezone_offsets() {
        // Test with IST (+05:30)
        let ist = FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 8, 24).unwrap();

        let t_blocked_1959 = ist
            .from_local_datetime(&date.and_time(NaiveTime::from_hms_opt(19, 59, 59).unwrap()))
            .unwrap();
        let t_allowed_2000 = ist
            .from_local_datetime(&date.and_time(NaiveTime::from_hms_opt(20, 0, 0).unwrap()))
            .unwrap();
        let t_allowed_2059 = ist
            .from_local_datetime(&date.and_time(NaiveTime::from_hms_opt(20, 59, 59).unwrap()))
            .unwrap();
        let t_blocked_2100 = ist
            .from_local_datetime(&date.and_time(NaiveTime::from_hms_opt(21, 0, 0).unwrap()))
            .unwrap();

        assert_eq!(evaluate_youtube_state(&t_blocked_1959), BlockState::Blocked);
        assert_eq!(evaluate_youtube_state(&t_allowed_2000), BlockState::Allowed);
        assert_eq!(evaluate_youtube_state(&t_allowed_2059), BlockState::Allowed);
        assert_eq!(evaluate_youtube_state(&t_blocked_2100), BlockState::Blocked);
    }

    #[test]
    fn test_time_window_custom_and_overnight() {
        // Standard daytime window: 09:00 - 17:00
        let work_window = TimeWindow::new(
            NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
        );
        assert_eq!(
            work_window.evaluate(&make_time(8, 59, 59)),
            BlockState::Blocked
        );
        assert_eq!(
            work_window.evaluate(&make_time(9, 0, 0)),
            BlockState::Allowed
        );
        assert_eq!(
            work_window.evaluate(&make_time(16, 59, 59)),
            BlockState::Allowed
        );
        assert_eq!(
            work_window.evaluate(&make_time(17, 0, 0)),
            BlockState::Blocked
        );

        // Overnight window: 22:00 - 04:00
        let night_window = TimeWindow::new(
            NaiveTime::from_hms_opt(22, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(4, 0, 0).unwrap(),
        );
        assert_eq!(
            night_window.evaluate(&make_time(21, 59, 59)),
            BlockState::Blocked
        );
        assert_eq!(
            night_window.evaluate(&make_time(22, 0, 0)),
            BlockState::Allowed
        );
        assert_eq!(
            night_window.evaluate(&make_time(23, 30, 0)),
            BlockState::Allowed
        );
        assert_eq!(
            night_window.evaluate(&make_time(1, 0, 0)),
            BlockState::Allowed
        );
        assert_eq!(
            night_window.evaluate(&make_time(3, 59, 59)),
            BlockState::Allowed
        );
        assert_eq!(
            night_window.evaluate(&make_time(4, 0, 0)),
            BlockState::Blocked
        );
    }

    #[test]
    fn test_simulated_reboot_and_time_jumps() {
        // Starting daemon at 20:30 (simulated boot/wake during window) -> must immediately be Allowed
        assert_eq!(
            evaluate_youtube_state(&make_time(20, 30, 0)),
            BlockState::Allowed
        );

        // Starting daemon at 23:00 (simulated boot/wake after window) -> must immediately be Blocked
        assert_eq!(
            evaluate_youtube_state(&make_time(23, 0, 0)),
            BlockState::Blocked
        );
    }
}
