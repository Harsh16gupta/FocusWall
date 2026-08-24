# FocusWall — System-Level Website Blocker for Self-Control

FocusWall is a Linux, system-level website-blocking tool designed for self-control, built to add deliberate friction against impulsive browsing.

---

## Two Core Rules

1. **YouTube is allowed only 20:00–21:00 local time every day**, with **no manual override**, **no temporary unlock**, and **no exceptions**. Blocking is the default state at all other times.
2. **The UI is never the enforcement mechanism.** A privileged background daemon (`focuswalld`) enforces policy at the system DNS and firewall levels independently. Closing, killing (`kill -9`), or uninstalling the UI never lifts a block.

---

## Architecture

```
┌─────────────────────────────┐
│  FocusWall UI (Tauri+React) │  runs as normal user, can be closed anytime
└──────────────┬──────────────┘
               │ Unix domain socket (/run/focuswall/focuswall.sock)
               ▼
┌─────────────────────────────┐
│  focuswalld (Rust daemon)   │  systemd-managed, auto-restart, starts at boot
│  - Policy engine             │  Owns SQLite DB (/var/lib/focuswall/focuswall.db)
│  - Scheduler (YouTube 20-21) │  Evaluates wall-clock time
│  - Rule manager               │
│  - Removal-cooldown manager   │
│  - Enforcement manager         │
└──────────────┬──────────────┘
               │
     ┌─────────┴─────────┐
     ▼                   ▼
┌───────────┐     ┌──────────────┐
│ DNS layer │     │ nftables     │
│ (dnsmasq  │     │ layer        │
│ sinkhole) │     │ (IP backstop)│
└───────────┘     └──────────────┘
               │
               ▼
           Internet
```

### Two-Layered Enforcement
- **Layer 1: DNS Sinkholing (`dnsmasq`)** — Maps blocked domains to `0.0.0.0` (IPv4) and `::` (IPv6). Distinguishes `youtube.com` / `googlevideo.com` from `google.com` without touching unrelated Google services.
- **Layer 2: Kernel Firewall (`nftables`)** — Dynamic IP-level backstop dropping outbound TCP/UDP traffic to resolved blocked domain IPs on ports `80`, `443`, `8080`, `8443`, plus dropping connections to known public DoH/DoT resolvers (`1.1.1.1`, `8.8.8.8`, `9.9.9.9`, etc.) on ports `53`, `853`, `443` to prevent DNS bypass.

---

## Project Structure

- [`focuswall-core`](file:///home/harsh-gupta/Projects/FocusWall/focuswall-core): Pure Rust library containing schedule evaluation, policy models, Public Suffix List domain normalization, SQLite storage, DNS generator, domain IP resolver, nftables generator, and IPC protocol.
- [`focuswalld`](file:///home/harsh-gupta/Projects/FocusWall/focuswalld): Privileged background enforcement daemon.
- [`focuswall-ui`](file:///home/harsh-gupta/Projects/FocusWall/focuswall-ui): Unprivileged desktop UI built with Tauri v2, React 18, TypeScript, and Tailwind CSS.

---

## Quick Start

### 1. Build the Workspace
```bash
cargo build --workspace
```

### 2. Run the Daemon (as root / service)
```bash
sudo ./target/debug/focuswalld
```

### 3. Launch the Desktop UI
```bash
npm run tauri dev --prefix focuswall-ui
# or run the web preview at http://localhost:1420:
npm run dev --prefix focuswall-ui
```

### 4. Install as a systemd Service (Production)
```bash
sudo cp target/debug/focuswalld /usr/local/bin/
sudo cp focuswalld/systemd/focuswalld.service /etc/systemd/system/
sudo mkdir -p /etc/focuswall
sudo cp focuswalld/systemd/config.toml.example /etc/focuswall/config.toml
sudo systemctl daemon-reload
sudo systemctl enable --now focuswalld
```

---

## CLI Commands

You can interact with `focuswalld` via CLI:

```bash
# Check current system status and schedule
focuswalld status

# Add a custom website to block (with 24h cooldown)
focuswalld add-rule "https://www.reddit.com/r/programming"

# Request removal for a custom rule (starts 24h countdown, site stays blocked)
focuswalld request-removal 2 --reason "Need access for research"

# Confirm removal after 24 hours have elapsed
focuswalld confirm-removal 2

# Cancel a pending removal request
focuswalld cancel-removal 2

# View audit logs
focuswalld logs --limit 20
```

---

## Testing Suite

Run the full automated test suite (30 unit & integration tests across scheduling boundaries, domain normalization, cooldowns, persistence, DNS, firewall, and IPC):

```bash
cargo test --workspace
```

---

## Operational Guide & Lifecycle

See [`HOW_FOCUSWALL_WORKS.md`](file:///home/harsh-gupta/Projects/FocusWall/HOW_FOCUSWALL_WORKS.md) for a comprehensive explanation of:
- Autonomous background operation (why you never need to keep the UI open)
- Automated YouTube 20:00–21:00 schedule transitions
- The 24-hour custom rule removal cooldown workflow

---

## Security & Threat Model

See [`THREAT_MODEL.md`](file:///home/harsh-gupta/Projects/FocusWall/THREAT_MODEL.md) for full threat model documentation.

- **In Scope**: Resists UI termination (`kill -9`), browser restarts, network switching, accidental user config edits, and premature rule removals.
- **Fail-Closed**: If the daemon stops or hits restart limits, DNS sinkhole records and nftables firewall rules persist in the system and remain enforcing.
- **Out of Scope**: Deliberate root dismantling with `sudo systemctl stop`, live USB booting, or secondary physical devices.
