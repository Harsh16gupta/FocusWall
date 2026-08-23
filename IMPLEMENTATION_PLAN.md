# FocusWall — Master Implementation Plan

> **READ THIS FIRST, EVERY SESSION.**
> This file is the single source of truth for what FocusWall is and how it must be built.
> Before writing or changing any code, the agent MUST:
> 1. Re-read this entire file.
> 2. Read `PROGRESS.md` to see what's already done and what the next milestone is.
> 3. Only work on the **current milestone**. Do not skip ahead. Do not "improve" future phases early.
> 4. After finishing a milestone: update `PROGRESS.md` (what changed, what was tested, what passed/failed), commit, and stop.
> 5. Never remove or weaken a security/fail-closed rule in this document to make a milestone "pass" more easily. If a rule seems to block progress, flag it in `PROGRESS.md` under `OPEN QUESTIONS` instead of silently working around it.

---

## 0. What FocusWall Is

FocusWall is a **Linux, system-level website-blocking tool for self-control**, built by the user for themselves, running entirely on their own machine. It is not sold, distributed to other people, or used to control anyone else's device or network. Its purpose is to add *deliberate friction* against the user's own impulsive browsing (specifically YouTube, plus any other site they add), not to achieve theoretically unbreakable enforcement.

Two rules define the whole project and must never be contradicted by any later decision:

1. **YouTube is allowed only 20:00–21:00 local time, every day, with no manual override, no temporary unlock, and no "just this once."** Blocking is the default state at all other times, automatically, with no user action required.
2. **The UI is never the enforcement mechanism.** A privileged background daemon enforces policy independently of whether any UI process is running. Closing, killing, or uninstalling the UI must never lift a block.

Everything below exists to serve these two rules.

---

## 1. Explicit Non-Goals (do not build these, ever, even if asked mid-project)

- No "disable protection" button anywhere in the UI.
- No temporary/partial unlock of the YouTube policy (no 15-min, no 30-min, no "skip today").
- No cloud sync, no multi-user accounts, no telemetry to any external server.
- No browser extension as the *primary* mechanism (may exist later purely as a defense-in-depth layer, never as the main blocker).
- No attempt to defeat root/physical access — see Threat Model (§3). Don't waste effort on unwinnable anti-root arms races (kernel-level rootkit-style tricks, anti-debugging, etc.).
- No screenshot monitoring, usage analytics, "addiction scoring," or any surveillance-flavored feature.

---

## 2. Final Architecture (locked)

```
┌─────────────────────────────┐
│  FocusWall UI (Tauri+React) │  runs as normal user, can be closed anytime
└──────────────┬──────────────┘
               │ Unix domain socket, restricted JSON-RPC-style protocol
               ▼
┌─────────────────────────────┐
│  focuswalld (Rust daemon)   │  runs as root (or CAP_NET_ADMIN-scoped user),
│  - Policy engine             │  systemd-managed, auto-restart, starts at boot
│  - Scheduler (YouTube 20-21) │
│  - Rule manager               │
│  - Removal-cooldown manager   │
│  - Enforcement manager         │
└──────────────┬──────────────┘
               │
     ┌─────────┴─────────┐
     ▼                   ▼
┌───────────┐     ┌──────────────┐
│ DNS layer │     │ nftables     │
│ (local    │     │ layer        │
│ resolver  │     │ (IP-based    │
│ override) │     │ backstop)    │
└───────────┘     └──────────────┘
               │
               ▼
           Internet
```

**Enforcement is two-layered on purpose:**

- **DNS layer (primary, domain-aware):** focuswalld runs (or configures) a local DNS resolver that all system DNS queries go through. Blocked domains resolve to `0.0.0.0` / `NXDOMAIN` / a local "blocked" page IP. This is domain-aware, so it can distinguish `youtube.com` from `google.com` without touching shared Google infrastructure.
- **nftables layer (backstop, IP-aware):** because DNS-based blocking can be bypassed by hardcoding a known IP or using DNS-over-HTTPS, focuswalld also maintains dynamically-updated nftables sets of currently-blocked IPs (resolved from the blocked domain list on an interval) and drops/rejects outbound traffic to them on relevant ports (80/443 at minimum). This is IP-scoped to *only the resolved IPs of blocked domains*, not broad CIDR ranges, to avoid collateral damage to unrelated Google/CDN services.
- **DoH/DoT closure:** focuswalld also blocks outbound connections to well-known public DoH/DoT resolver IPs/SNI at the firewall layer (configurable list), and forces the system resolver via `resolv.conf`/`systemd-resolved` configuration, so applications that would otherwise bypass the local resolver via hardcoded DoH are pushed back onto the enforced path. This is explicitly a best-effort measure — document it as such, don't oversell it.

---

## 3. Threat Model (must ship as a doc, `THREAT_MODEL.md`, not just live in someone's head)

**In scope — must be resisted:**
- Closing/killing the UI process.
- Killing/restarting browsers.
- Normal application crashes.
- Accidental or impulsive edits to user-level config files (there shouldn't be any user-level policy files to edit).
- Simple `kill focuswalld` — systemd should restart it and it should re-derive the correct state from time + persisted policy, not from an in-memory timer.
- Reboot, sleep/wake, network change — state must be re-derived from current time on every relevant event, not carried forward from a stale timer.
- Impulsive one-click removal of a custom blocked site — must require the cooldown workflow.
- Basic DNS bypass attempts (switching `/etc/resolv.conf` by hand as a normal user without root) — should fail because the daemon owns resolver config and/or reasserts it periodically, or requires root to change.

**Out of scope — explicitly not defended against, and the docs/UI should say so honestly:**
- Root access used deliberately to dismantle the system (`systemctl stop focuswalld`, `rm -rf /etc/focuswall`, editing nftables as root, etc.).
- Booting another OS / live USB / reinstalling the OS.
- Physical access with another device (blocking one laptop doesn't block a phone).
- A sufficiently patient and technical VPN/proxy setup that routes around the local resolver and firewall entirely (e.g., a VM with its own network namespace). FocusWall should raise the bar, not claim to be unbeatable.

---

## 4. Privilege Separation & IPC

- UI runs as the logged-in user, **no elevated privileges, ever**.
- `focuswalld` runs as root (simplest correct option for nftables/resolver control) or, if feasible in Phase 2+, as a dedicated system user with `CAP_NET_ADMIN` + `CAP_NET_RAW` capabilities set via systemd (`AmbientCapabilities=`).
- Communication: **Unix domain socket** at `/run/focuswall/focuswall.sock`, permissions `0660`, owned by a dedicated `focuswall` group that the invoking user must belong to.
- Protocol: length-prefixed or newline-delimited JSON messages (`serde_json`). No shell execution, no arbitrary command passthrough, ever.
- **Allowed UI → daemon requests (the entire whitelist, nothing else):**
  - `get_status` — current YouTube state, next window, list of custom rules + statuses.
  - `add_rule { input: String }` — propose a new custom blocked domain (subject to normalization + confirmation flow in UI before this is even called).
  - `request_removal { rule_id }` — start cooldown for a custom rule. **Never valid for the built-in YouTube policy** — daemon must reject this by rule type, not just by convention.
  - `confirm_removal { rule_id }` — finalize removal *after* cooldown has elapsed (daemon checks server-side time, not client-claimed time).
  - `cancel_removal_request { rule_id }` — optional, Phase 3+.
  - `get_logs { since }` — read-only recent audit events.
- There is intentionally **no** `disable_protection`, `override_youtube`, `set_config`, or `run_command` endpoint.

---

## 5. Persistent Storage

- Location: `/var/lib/focuswall/focuswall.db` (SQLite), directory mode `0700`, owned by root (or the `focuswall` service user). Not readable by the normal user account.
- `/etc/focuswall/config.toml` — static daemon config (socket path, log level, DNS/firewall backend choice, DoH-blocklist toggle). Root-owned, `0644` read is fine.
- Schema (SQLite, Phase 1 minimum viable subset, extend in later phases):

```sql
CREATE TABLE policies (
  id INTEGER PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('system','custom')),
  name TEXT NOT NULL,              -- e.g. 'youtube', or normalized domain for custom
  domains TEXT NOT NULL,           -- JSON array of domains/subdomain patterns
  schedule_start TEXT,             -- e.g. '20:00', NULL = always-blocked-until-removed
  schedule_end TEXT,               -- e.g. '21:00'
  timezone TEXT NOT NULL DEFAULT 'system',
  status TEXT NOT NULL DEFAULT 'active', -- active | removal_pending | removed
  created_at TEXT NOT NULL,
  removal_requested_at TEXT,
  removal_cooldown_hours INTEGER,
  earliest_removal_at TEXT,
  removal_reason TEXT
);

CREATE TABLE audit_log (
  id INTEGER PRIMARY KEY,
  ts TEXT NOT NULL,
  event_type TEXT NOT NULL,   -- policy_change | daemon_start | daemon_stop | removal_requested | removal_confirmed | enforcement_error
  detail TEXT
);
```

- The YouTube system policy row is **seeded on first daemon start** (`kind='system', name='youtube', domains=[...], schedule_start='20:00', schedule_end='21:00'`) and the daemon must refuse any IPC call that tries to modify or remove a `kind='system'` row.

---

## 6. YouTube Domain List (starting set — verify/expand during Phase 1)

```
youtube.com
www.youtube.com
m.youtube.com
music.youtube.com
youtu.be
youtube-nocookie.com
ytimg.com
googlevideo.com
```

---

## 7. Scheduling Logic (must be time-derived, never timer-derived)

Core evaluation function, run on: daemon start, every N seconds (e.g. every 15s poll, cheap), on wake-from-sleep, and on any manual policy change:

```rust
fn evaluate_youtube_state(now: DateTime<Tz>) -> BlockState {
    let start = today_at(now, "20:00");
    let end   = today_at(now, "21:00");
    if now >= start && now < end { ALLOWED } else { BLOCKED }
}
```

- Timezone: Phase 1 uses system timezone (read via `/etc/localtime` or `iana-time-zone` crate) as the working default.
- On every state transition (BLOCKED→ALLOWED, ALLOWED→BLOCKED), daemon must: update in-memory state, write an audit_log row, and re-apply DNS/nftables rules to match.

---

## 8. Custom Rule Normalization

Given raw input, produce a normalized policy:

1. If input has no scheme, prepend `https://` before parsing.
2. Parse with the `url` crate.
3. Extract host.
4. Strip a leading `www.` to get the "registrable root" via the **Public Suffix List** (`publicsuffix` crate).
5. Resulting policy = `{root_domain, block_subdomains: true}` by default (blocks `root_domain` and `*.root_domain`).
6. Show the user exactly what will be blocked before confirming.

---

## 9. Removal Cooldown Workflow

- Default cooldown: **24 hours** for custom rules. YouTube policy is never eligible.
- `request_removal`: sets `status='removal_pending'`, `removal_requested_at=now`, `earliest_removal_at=now+cooldown`. Site **stays blocked** through this whole period.
- `confirm_removal`: only succeeds if `now >= earliest_removal_at` (checked server-side using daemon's clock). On success, `status='removed'`, domains unblocked, audit logged.

---

## 10. systemd Integration

Unit file `/etc/systemd/system/focuswalld.service`:

```ini
[Unit]
Description=FocusWall Enforcement Daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=notify
ExecStart=/usr/local/bin/focuswalld
Restart=on-failure
RestartSec=2
StartLimitIntervalSec=60
StartLimitBurst=5
NoNewPrivileges=false
ProtectSystem=strict
ReadWritePaths=/var/lib/focuswall /run/focuswall /etc/resolv.conf
RuntimeDirectory=focuswall
RuntimeDirectoryMode=0770

[Install]
WantedBy=multi-user.target
```

---

## 11. Rust Crate Choices

| Purpose | Crate |
|---|---|
| Async runtime | `tokio` |
| CLI/daemon arg parsing | `clap` |
| Serialization | `serde`, `serde_json` |
| SQLite | `rusqlite` |
| URL parsing | `url` |
| Public suffix handling | `publicsuffix` |
| Time/timezone | `chrono`, `iana-time-zone` |
| systemd notify | `sd-notify` |
| nftables control | shell out to `nft` via `std::process::Command` |
| DNS resolver | manage `dnsmasq` config |
| Logging | `tracing` + `tracing-subscriber` |
| IPC | `tokio::net::UnixListener` with newline-delimited JSON frames |

---

## 12. Tauri + React UI (Phase 5 — build last)

- Screens: Status dashboard, Add Website flow with confirmation, Blocked Websites list with removal cooldown countdowns, Audit Logs view.
- All calls go through the fixed IPC whitelist in §4.

---

## 13. Phased Build Plan & Milestones

### Milestone 0 — Repo & scaffolding
- Cargo workspace: `focuswalld/` (daemon bin), `focuswall-core/` (shared policy/scheduling logic lib), `focuswall-ui/` (Tauri app, built last).
- `THREAT_MODEL.md`, `PROGRESS.md`, `TESTING.md`, `IMPLEMENTATION_PLAN.md`.
- DoD: `cargo build` succeeds for the workspace; `PROGRESS.md` exists with Milestone 0 marked done.

### Milestone 1 — Core scheduling logic (pure library, no privilege needed yet)
- Implement `evaluate_youtube_state(now)` in `focuswall-core` with full unit tests.
- DoD: `cargo test` covers boundary cases (19:59:59, 20:00:00, 20:59:59, 21:00:00) and passes.

### Milestone 2 — Minimal daemon, DNS-only enforcement, CLI status (Phase 1 target)
- `focuswalld` binary: seed SQLite YouTube policy if absent, evaluate state, write `dnsmasq` config, reload.
- Poll every 15s, re-evaluate, re-apply if state changed. CLI status subcommand.
- DoD: manual clock / `--fake-now` verify blocked/allowed via `dig`/`curl`, across restart.

### Milestone 3 — systemd unit + persistence hardening (Phase 2 target)
- Install systemd unit, verify restart behavior on crash, verify `/var/lib/focuswall` permissions, audit logging.
- DoD: full checklist from `TESTING.md` §"Daemon Tests" and "System Tests".

### Milestone 4 — nftables backstop + basic DoH/custom-DNS closure
- Resolve blocked-domain IPs on interval, maintain nftables set, drop outbound 80/443 during blocked windows.
- IPv6 and IPv4 parity.
- DoD: `curl -4`/`-6` to IPs blocked, direct `8.8.8.8` DNS queries blocked at firewall.

### Milestone 5 — Custom website rules (Phase 3 target)
- `add_rule` normalization and cooldown workflow (`request_removal`/`confirm_removal`) over IPC socket.
- DoD: domain normalization, cooldown time enforcement on server side.

### Milestone 6 — Tauri + React UI (Phase 5 target)
- Build UI screens wired to IPC whitelist.
- DoD: manual walkthrough, UI closing/killing doesn't affect blocking.

### Milestone 7 — Full test pass & docs
- Run every scenario in `TESTING.md` end-to-end. Finalize docs.

---

## 14. Rules the Agent Must Never Break

1. Never add any IPC endpoint or UI control that can disable, override, or shorten the YouTube block or any active removal cooldown.
2. Never store policy in a location writable by the unprivileged user.
3. Never let a daemon crash result in blocking rules being removed — rules must be fail-closed.
4. Never derive block/allow state from an in-memory timer — always compute from wall-clock time against schedule.
5. Never widen YouTube blocking to unrelated Google domains, or narrow it in a way that leaves `googlevideo.com` reachable.
6. Any deviation from this plan gets written to `PROGRESS.md` under Open Questions.
