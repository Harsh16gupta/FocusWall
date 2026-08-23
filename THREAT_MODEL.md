# FocusWall Threat Model

## 1. Overview & Purpose
FocusWall is a system-level website-blocking tool designed for self-control on Linux. Its purpose is to add deliberate friction against impulsive browsing.

FocusWall adheres to two core security principles:
1. **YouTube is allowed only 20:00–21:00 local time every day**, with zero manual overrides, zero temporary unlocks, and zero exceptions.
2. **The UI is never the enforcement mechanism.** A privileged background daemon (`focuswalld`) enforces policy at the system DNS and firewall levels.

---

## 2. Security Boundaries & Privilege Model
- **User Space / UI Layer**: Runs as an unprivileged standard user. Has no sudo/root rights, no ability to directly modify `/etc`, `/var/lib/focuswall`, or kernel nftables.
- **Daemon Layer (`focuswalld`)**: Runs as root (or `CAP_NET_ADMIN` / `CAP_NET_RAW` service user) managed by `systemd`. Owns the SQLite policy database (`/var/lib/focuswall/focuswall.db`, mode `0700`), the local DNS resolver configuration, and nftables rulesets.
- **IPC Interface**: Unix domain socket at `/run/focuswall/focuswall.sock` (mode `0660`, owned by group `focuswall`). Only a strictly whitelisted, command-free JSON protocol is accepted.

---

## 3. In-Scope Threats (Defended Against)
FocusWall is engineered to resist:
- **UI Termination**: Closing, killing (`kill -9`), or uninstalling the Tauri UI. (Enforcement is daemon-side and unaffected).
- **Browser Restarts / Crashes**: Opening new browser tabs, switching browsers (Chrome, Firefox, Chromium), or clearing browser state.
- **Process Termination of Daemon**: Killing `focuswalld` triggers automatic systemd restart (`Restart=on-failure`). Furthermore, firewall and DNS rules are persistent in the system and remain fail-closed even if the daemon is momentarily down.
- **Reboot / Suspend / Sleep / Network Switch**: State is dynamically derived from wall-clock time against policy schedules on every tick and wake event, never from monotonic or relative timers.
- **Impulsive File Modification**: Policy databases are stored in `/var/lib/focuswall/` with `0700` permissions (root-owned), rendering them unwritable and unreadable by standard user accounts.
- **Impulsive Custom Rule Deletion**: Removal of custom rules requires a 24-hour server-side cooldown workflow (`request_removal` -> wait 24h -> `confirm_removal`). Built-in system policies (YouTube) reject removal requests unconditionally at the type level.
- **Basic DNS Bypass**: Local resolver configuration is enforced via root-managed files. Outbound connections to well-known DoH/DoT resolver IPs and port 53 / 853 bypasses are caught by nftables rules.

---

## 4. Out-of-Scope Threats (Non-Goals)
FocusWall explicitly does not attempt to defend against:
- **Deliberate Root Escalation**: A user deliberately using `sudo` to run `systemctl stop focuswalld`, modifying `/etc/focuswall`, or flushing `nftables` as root. (Anti-root arms races are out of scope).
- **Physical Access / Secondary Devices**: Using a separate phone, tablet, or another computer.
- **OS Reinstallation / Live Boot**: Booting into a live USB or another partition.
- **Advanced Isolated Network Namespaces**: Advanced virtual machines or containerized networks with dedicated network interfaces bypassing local host routing.

---

## 5. Fail-Closed Principles
1. **Rule Persistence**: DNS block records and nftables rules remain in the kernel/resolver even if `focuswalld` crashes or hits restart burst limits.
2. **Default Deny**: YouTube is blocked by default at all times outside the explicit 20:00–21:00 window.
3. **No Backdoors**: No debug commands or override endpoints exist in the IPC interface.
