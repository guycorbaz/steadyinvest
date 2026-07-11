---
name: verify
description: Launch and drive the steadyinvest Slint GUI headlessly to verify a change end-to-end (isolated journal copy, Xvfb, xdotool). Use when a change needs runtime observation in the real app.
---

# Verify steadyinvest changes in the running GUI

The app is a Slint desktop GUI (`target/debug/steadyinvest-app`). Verify against an
ISOLATED copy of the user's journal — never the live one.

## Setup (once per session)

1. Copy the journal (python, `sqlite3` CLI is not installed):
   ```bash
   python3 -c "
   import sqlite3
   src = sqlite3.connect('/home/gcorbaz/.local/share/steadyinvest/journal.db')
   dst = sqlite3.connect('$SCRATCH/journal.db')
   src.backup(dst)"
   ```
2. Isolated config: copy `~/.config/steadyinvest/config.json` to
   `$SCRATCH/xdg/config/steadyinvest/config.json`, set `journal_path` to the copy and
   empty `recent_journals`. **The app REWRITES `journal_path` in config.json when the
   targeted journal is locked → re-point it before EVERY relaunch.**
3. Studies live in the `studies` table, one JSON `payload` per row (contract `Study`) —
   fabricate test cases by mutating the payload JSON directly in the copy.

## Launch (headless)

```bash
Xvfb :99 -screen 0 1600x1000x24 &
env -u WAYLAND_DISPLAY DISPLAY=:99 SLINT_BACKEND=winit-software \
  XDG_CONFIG_HOME=$SCRATCH/xdg/config XDG_DATA_HOME=$SCRATCH/xdg/data \
  ./target/debug/steadyinvest-app &
```

- Without `SLINT_BACKEND=winit-software` the GL rendering is black under Xvfb.
- Without `-u WAYLAND_DISPLAY` the window opens on the USER'S REAL DESKTOP.
- `cargo test` does not always relink the bin — run `cargo build -p steadyinvest-app`
  before relaunching after a code change.

## Drive & capture

- Drive with `DISPLAY=:99 xdotool mousemove X Y click 1`; scroll with `click 4/5`
  (repeat ~40× to reach §5 in a study).
- Capture with `DISPLAY=:99 import -window root shot.png`.
- Dashboard is the start screen; a study opens by clicking its ticker row.

## Gotchas

- **NEVER `pkill -f steadyinvest-app`** — it also kills the user's real instance
  (same binary path). Kill by saved PID only.
- "Exporter PDF" opens the xdg-desktop-portal file chooser → unavailable under Xvfb;
  the PDF path cannot be driven headlessly.
- `DISPLAY=:0` is the user's real desktop — do not use it.
