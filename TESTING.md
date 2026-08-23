# FocusWall Testing Checklist

Use this alongside `IMPLEMENTATION_PLAN.md` §13. Check items off with date + result in `PROGRESS.md`, not here — this file stays a static checklist.

## Application (UI) Tests
- [ ] Close UI normally → blocking unaffected.
- [ ] `kill -9` the UI process → blocking unaffected.
- [ ] Restart UI → shows correct live state immediately (no stale cache).
- [ ] Uninstall/remove UI entirely → daemon keeps enforcing.

## Daemon Tests
- [ ] `kill -9 focuswalld` → systemd restarts it (check `RestartSec`).
- [ ] Repeated crash beyond `StartLimitBurst` → daemon stays down, but last-applied rules remain (YouTube still blocked at the system level).
- [ ] Restart daemon mid-ALLOWED-window → correctly stays ALLOWED (not incorrectly re-blocked).
- [ ] Restart daemon mid-BLOCKED-window → correctly stays BLOCKED.

## System Tests
- [ ] Full reboot → service enabled at boot, correct state restored within a few seconds.
- [ ] Suspend during BLOCKED, resume after window would have opened → resumes ALLOWED.
- [ ] Suspend during ALLOWED, resume after window would have closed → resumes BLOCKED.
- [ ] Disconnect/reconnect Wi-Fi → no change in enforcement correctness.
- [ ] Change network (e.g. switch to mobile hotspot) → enforcement still applies (resolver/firewall reasserted).

## Schedule Boundary Tests
- [ ] System clock at 19:59:59 → BLOCKED.
- [ ] System clock at 20:00:00 → ALLOWED.
- [ ] System clock at 20:59:59 → ALLOWED.
- [ ] System clock at 21:00:00 → BLOCKED.
- [ ] Daemon started fresh at 20:30 → immediately ALLOWED.
- [ ] Daemon started fresh at 23:00 → immediately BLOCKED.

## Network / Bypass Tests
- [ ] `youtube.com` blocked in Chrome, Firefox, Chromium, and a plain `curl`.
- [ ] `googlevideo.com` also blocked (not just the front-end domain).
- [ ] IPv4 blocked.
- [ ] IPv6 blocked.
- [ ] Manually pointing a browser/OS at `8.8.8.8` or `1.1.1.1` as DNS still results in blocked access (firewall backstop catches it).
- [ ] A known public DoH endpoint is unreachable while enforcement is active.
- [ ] Unrelated Google services (Search, Gmail, Drive) remain fully functional at all times, including during YouTube-blocked windows.

## Custom Rule Tests
- [ ] Add `https://www.reddit.com/r/programming` → normalizes to `reddit.com` (+ subdomains), confirmation screen shown before applying.
- [ ] Rule blocks immediately after confirmation.
- [ ] `request_removal` does not lift the block.
- [ ] `confirm_removal` fails before cooldown elapses (server-side time check, not client time).
- [ ] `confirm_removal` succeeds after cooldown elapses.
- [ ] YouTube system policy rejects `request_removal`/`confirm_removal` outright.

## Persistence Tests
- [ ] Policy DB lives outside the user's home directory and is not writable by the unprivileged user.
- [ ] Deleting a hypothetical user-level config file (there shouldn't be one) is a non-issue — confirm no policy-relevant file exists under `~/.config`.
