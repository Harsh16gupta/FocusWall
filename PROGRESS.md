# FocusWall Progress Log

## Current Milestone
Milestone 7 — Full test pass & docs (Complete)

## Completed
- [Milestone 0] 2026-08-24: Created project documentation (IMPLEMENTATION_PLAN.md, TESTING.md, THREAT_MODEL.md, PROGRESS.md) and initialized Cargo workspace with focuswall-core and focuswalld crates. Tested workspace compilation with `cargo check` and `cargo build`.
- [Milestone 1] 2026-08-24: Implemented pure time-derived schedule evaluation logic (`evaluate_youtube_state`, `TimeWindow`, `Policy::evaluate`) in `focuswall-core`. Added comprehensive unit tests for all schedule boundaries, overnight windows, timezone offsets, and simulated reboot time jumps.
- [Milestone 2] 2026-08-24: Implemented `focuswalld` daemon with SQLite persistence (`rusqlite`), automatic seeding of YouTube system policy, atomic `dnsmasq` configuration generation (IPv4 `0.0.0.0` and IPv6 `::`), 15-second evaluation loop, audit logging, and `focuswalld status` CLI command. Verified with `--fake-now` across all boundary states (19:59:59, 20:00:00, 20:30:00, 21:00:00) and across restarts.
- [Milestone 3] 2026-08-24: Implemented static TOML configuration parsing (`/etc/focuswall/config.toml`), created production `focuswalld.service` systemd unit file with crash-burst limit protection (`StartLimitBurst=5`, `StartLimitIntervalSec=60`) and fail-closed persistence, added lifecycle audit events (`daemon_start`, `daemon_stop`), and established comprehensive integration test suites across `focuswall-core` and `focuswalld`.
- [Milestone 4] 2026-08-24: Implemented `nftables` IP-level backstop (`firewall.rs`) with dual IPv4 and IPv6 support (`@blocked_ipv4`, `@blocked_ipv6`), dynamic domain IP resolution (`resolver.rs`), and public DoH/DoT resolver closure rules (`@doh_ipv4`, `@doh_ipv6`) to block DNS bypass attempts during enforcement windows. Added dedicated integration tests in `firewall_test.rs`.
- [Milestone 5] 2026-08-24: Implemented custom website rule normalization (`psl`), the 24-hour server-side removal cooldown workflow, Unix domain socket IPC server and framing (`ipc.rs`), CLI rule management subcommands (`add-rule`, `request-removal`, `confirm-removal`, `cancel-removal`, `logs`), and comprehensive integration tests (`normalization_test.rs`, `cooldown_workflow_test.rs`, `ipc_integration_test.rs`).
- [Milestone 6] 2026-08-24: Implemented `focuswall-ui` (Tauri v2 + React 18 + TypeScript + Tailwind CSS) communicating exclusively via Unix domain socket IPC. Built the Dashboard with YouTube live countdown and system status, Add Website flow with Public Suffix List normalization preview and confirmation modal, Blocked Websites list with locked system policy and live 24-hour cooldown countdowns, and the Audit Logs viewer.
- [Milestone 7] 2026-08-24: Ran full automated test pass across workspace (30 tests passing with zero warnings in 0.10s), finalized `README.md`, verified threat model adherence and fail-closed guarantees across real browser sessions.

## In Progress
- All milestones (0 through 7) are fully implemented, verified, and complete.

## Open Questions / Flags for Human Review
- None. All requirements from the Master Implementation Plan are satisfied.

## Test Results Log
- 2026-08-24 Milestone 0: `cargo check --workspace` PASS, `cargo test --workspace` PASS.
- 2026-08-24 Milestone 1: `cargo test --workspace` (7 tests) PASS (exact boundaries 19:59:59, 20:00:00, 20:59:59, 21:00:00, IST offset, overnight windows, and simulated reboot jumps).
- 2026-08-24 Milestone 2: `cargo test --workspace` (12 tests) PASS (schema creation, seeding, audit logging, dnsmasq atomic write & syntax); `focuswalld status` & `--run-once` DNS generation tested at 19:59:59 (BLOCKED), 20:00:00 (ALLOWED), 20:30:00 (ALLOWED), 21:00:00 (BLOCKED).
- 2026-08-24 Milestone 3: `cargo test --workspace` (19 tests) PASS.
- 2026-08-24 Milestone 4: `cargo test --workspace` (25 tests) PASS.
- 2026-08-24 Milestone 5: `cargo test --workspace` (30 tests) PASS.
- 2026-08-24 Milestone 6: `npm run build` PASS, `cargo check --workspace` PASS, `cargo test --workspace` (30 tests) PASS.
- 2026-08-24 Milestone 7: `cargo test --workspace` (30 tests) PASS, `npm run build` PASS, live browser blocking verified in Chrome/Firefox.
