# FocusWall Progress Log

## Current Milestone
Milestone 3 — systemd unit + persistence hardening (Phase 2 target)

## Completed
- [Milestone 0] 2026-08-24: Created project documentation (IMPLEMENTATION_PLAN.md, TESTING.md, THREAT_MODEL.md, PROGRESS.md) and initialized Cargo workspace with focuswall-core and focuswalld crates. Tested workspace compilation with `cargo check` and `cargo build`.
- [Milestone 1] 2026-08-24: Implemented pure time-derived schedule evaluation logic (`evaluate_youtube_state`, `TimeWindow`, `Policy::evaluate`) in `focuswall-core`. Added comprehensive unit tests for all schedule boundaries, overnight windows, timezone offsets, and simulated reboot time jumps.
- [Milestone 2] 2026-08-24: Implemented `focuswalld` daemon with SQLite persistence (`rusqlite`), automatic seeding of YouTube system policy, atomic `dnsmasq` configuration generation (IPv4 `0.0.0.0` and IPv6 `::`), 15-second evaluation loop, audit logging, and `focuswalld status` CLI command. Verified with `--fake-now` across all boundary states (19:59:59, 20:00:00, 20:30:00, 21:00:00) and across restarts.
- [Milestone 3] 2026-08-24: Implemented static TOML configuration parsing (`/etc/focuswall/config.toml`), created production `focuswalld.service` systemd unit file with crash-burst limit protection (`StartLimitBurst=5`, `StartLimitIntervalSec=60`) and fail-closed persistence, added lifecycle audit events (`daemon_start`, `daemon_stop`), and established comprehensive integration test suites across `focuswall-core` and `focuswalld`.

## In Progress
- Completed Milestone 3. Ready for Milestone 4.

## Open Questions / Flags for Human Review
- None at this time.

## Test Results Log
- 2026-08-24 Milestone 0: `cargo check --workspace` PASS, `cargo test --workspace` PASS.
- 2026-08-24 Milestone 1: `cargo test --workspace` (7 tests) PASS (exact boundaries 19:59:59, 20:00:00, 20:59:59, 21:00:00, IST offset, overnight windows, and simulated reboot jumps).
- 2026-08-24 Milestone 2: `cargo test --workspace` (12 tests) PASS (schema creation, seeding, audit logging, dnsmasq atomic write & syntax); `focuswalld status` & `--run-once` DNS generation tested at 19:59:59 (BLOCKED), 20:00:00 (ALLOWED), 20:30:00 (ALLOWED), 21:00:00 (BLOCKED).
- 2026-08-24 Milestone 3: `cargo test --workspace` (19 tests) PASS:
  - `focuswall-core` unit tests (14 passed)
  - `tests/dns_integration_test.rs` (1 passed)
  - `tests/persistence_test.rs` (2 passed)
  - `tests/daemon_integration_test.rs` (2 passed)
