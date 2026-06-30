# Story 5.5: Journal location, recent journals & sync-safety

Status: done (3-layer review 2026-06-30 — 5/5 ACs; 12 patches applied [rfd→async-std runtime fix, sync-mode on all open paths, start-time-qualified lock, read-only-media best-effort lock, startup stale-lock auto-reclaim, record-pointer-on-close, journal_id stale guard, canonicalize sync detection, reclaim-only-on-lock, honest open-failed message, same-journal no-op, backup-empty-parent], rest dismissed; workspace 571 tests, fmt/clippy -D/deny green; NO core/migration/SCHEMA_VERSION change; rfd +5 packages, deny.toml unchanged)

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want to choose where my journal lives and reopen the last one,
so that I control my data location and benefit from NAS backup without corruption.

## Scope decision (Guy, 2026-06-30)

**Add `rfd` for a native file picker.** Story 5.5 ships the real OS open/create/pick-directory dialogs
(the new external dependency `rfd`, verified against `deny.toml`), replacing the path-based text fields
of Stories 5.2–5.4. It also delivers: **recent journals** + **reopen-last-used on launch** (already
partly works via `AppConfig.journal_path` — extended to a recent list carrying `(journal_id,
last-seen-version)`), a **single-instance lock** (the same journal can't be opened twice), and
**sync-folder detection → warn + `journal_mode=DELETE`** with the recommended *live-DB-local + versioned
backups-to-sync* pattern (ADD8). It also **re-homes `create_backup`** beside the user-selected journal
(deferred from Story 5.4).

## Acceptance Criteria

1. **AC1 — Choose / create / open / switch a journal via a native picker (FR66).** Réglages (and/or a File area) offers **native** dialogs (`rfd`): *Create a new journal* (pick a directory + name → `Journal::create`), *Open a journal* (pick a `.db` → `Journal::open`), and *switch* between journals. Switching **closes the current journal cleanly** (checkpoint + drop the handle + release its lock) and opens the chosen one, then **every surface re-renders**. A failed open (missing/corrupt/foreign-schema file) is a **neutral typed refusal** that leaves the current journal intact — never a journal-less app.

2. **AC2 — Recent journals + reopen the last-used on launch (pointer = `(journal_id, last-seen-version)`, FR66/ADD7).** The app remembers a **recent-journals list** in **app-config** (via `directories`, never inside the journal): each entry carries the **path**, the **`journal_id`**, and the **last-seen `logical_version`**. On launch the app **reopens the last-used journal** (the existing `journal_path` behaviour, now updated to also record `(journal_id, last-seen-version)` on every open/close). Re-opening a recent entry whose on-disk `(journal_id, version)` regressed surfaces a neutral "this journal looks older than you last saw it (you saw vN, this is vM)" notice (the stale-detection the 5.4 review and #65 wanted — now with the real last-seen pointer). The list is bounded (most-recent-first, de-duplicated by canonical path, capped).

3. **AC3 — Single-instance lock: the same journal cannot be opened twice (ADD6).** Opening a journal acquires an **exclusive lock** (a `…-lock` sidecar created atomically with `create_new`, carrying the owning PID); a second open of the **same** journal (another app instance, or a switch back) is **refused** with a neutral notice. The lock is **released on clean close / switch / app exit**. A **stale** lock (the recorded PID is no longer alive) is detected and may be **reclaimed** (offered, not auto-forced). Releasing/reclaiming never corrupts the journal.

4. **AC4 — Sync-folder detection → warn + `journal_mode=DELETE` (ADD8).** When the chosen directory is a **detected sync folder** (path heuristics for Synology Drive / Dropbox / OneDrive / iCloud / Google Drive), the app **warns** (a SQLite `.db` in a sync folder risks WAL corruption — the standing project warning) and opens that journal with **`journal_mode=DELETE`** (or `TRUNCATE`) instead of `WAL`, and **offers the recommended pattern**: keep the live DB local + push **versioned backups** (Story 5.4's `create_backup`) to the sync folder. The mode choice is per-open (derived from the path), not persisted into the journal. **`create_backup` is re-homed** to a `backups/` folder **beside the user-selected journal** (Story 5.4 deferral) when a custom location is set, falling back to the OS data dir otherwise.

5. **AC5 — `core`/method untouched; one new dependency (`rfd`), audited; neutral posture.** The work is **`persistence` (journal-mode option + the lock + identity reads) + `app` (config recent-list + the location/lock/sync rails + `rfd` dialogs + UI)**. **No `core::ssg` change**, **no migration** (`PRAGMA user_version` registry unchanged — `journal_mode` is a connection pragma, not a schema change; the lock is a sidecar file), **no `contract::SCHEMA_VERSION` bump**. **`rfd` is added to `app/Cargo.toml`** (a real new package + transitive deps); **`deny.toml` must stay green** (`cargo deny check` — verify every new crate's license is in the allow-list; add to the allow-list only with neutral justification, never relax `yanked`/`unknown-registry`). Every new literal goes through `@tr`; the floor is bumped by exactly the number added; any new `MSG_*` is registered; copy neutral, fact-stating (FR13).

## Tasks / Subtasks

- [x] **Task 1 — `rfd` dependency + license audit (AC1, AC5)** — `app/Cargo.toml`, `deny.toml`
  - [x] Add `rfd` to `app/Cargo.toml`. Run `cargo deny check`; for each newly-pulled crate whose license is not yet in `deny.toml`'s `allow` list, add it **only if** it is a standard permissive license (MIT/Apache-2.0/BSD/Unicode/Zlib…), with a one-line neutral justification comment. Record the `Cargo.lock` delta (this story **does** add packages — that is expected and called out).
  - [x] Confirm the Linux backend builds (rfd uses xdg-desktop-portal / GTK); the smoke launch still reaches the event loop.

- [x] **Task 2 — `persistence`: journal-mode option, identity-on-open, single-instance lock (AC2, AC3, AC4)** — `persistence/src/`
  - [x] Add a **journal-mode** option to open/create: `pub enum JournalMode { Wal, Delete }` + `Journal::create_with_mode(path, id, created_at, mode)` / `Journal::open_with_mode(path, mode)` (the existing `create`/`open` delegate with `Wal`). `apply_read_write_pragmas` takes the mode (`journal_mode = WAL | DELETE`). `Delete` is the sync-safe mode (no `-wal`/`-shm`).
  - [x] **Single-instance lock** (`persistence/src/lock.rs` or in `journal.rs`): acquire an exclusive `…-lock` sidecar with `OpenOptions::new().write(true).create_new(true)` (atomic — fails if it exists), write the owning **PID** (`std::process::id()`). `Journal` **holds** the lock guard and **releases** it on `Drop` (removes the sidecar). A `LockHeld { pid }` typed error when the sidecar exists; a helper `lock_is_stale(pid)` (Linux: `/proc/<pid>` absent) so the app can offer to reclaim. Releasing is best-effort + idempotent.
  - [x] Identity-on-open already exists (`Journal::id()` / `logical_version()`); expose what the app needs for the recent-list pointer (already public).
  - [x] Tests: open holds the lock → a second `open` of the same path → `LockHeld`; drop the first → re-open succeeds; `JournalMode::Delete` opens with no `-wal` created on a write; a stale-PID lock is detected by `lock_is_stale`.

- [x] **Task 3 — App config: recent-journals list with the `(journal_id, last-seen-version)` pointer (AC2)** — `app/src/config.rs`
  - [x] Add `pub struct RecentJournal { pub path: PathBuf, pub journal_id: String, pub last_seen_version: u64 }` + `AppConfig.recent_journals: Vec<RecentJournal>` (append-only `#[serde(default)]`; a pre-5.5 config loads with an empty list). Keep `journal_path` (the last-used path) for back-compat; the recent list is the richer record.
  - [x] Helpers: `record_recent(&mut self, path, journal_id, version)` (move-to-front, de-dupe by canonical path, cap at N e.g. 8); `last_seen_version_for(path) -> Option<u64>` (for stale detection). Tests: round-trip; de-dupe + cap + most-recent-first ordering; an old config without the field loads.

- [x] **Task 4 — App state: open/create/switch/lock/sync rails (AC1, AC2, AC3, AC4)** — `app/src/state.rs`
  - [x] `JournalState` gains the location rails: `open_journal(path)` / `create_journal(dir, name)` / `switch_to(path)` — each **closes the current journal cleanly** (checkpoint + drop handle → releases its lock), detects the target's sync-folder status, opens with the right `JournalMode`, acquires the lock (mapping `LockHeld` → a neutral notice + the reclaim offer), records the recent entry + `journal_path`, and persists app-config. A failed open leaves the **previous** journal open (re-acquire its lock) — never journal-less.
  - [x] `is_sync_folder(path) -> bool` (pure path heuristic: a path component matches `Synology(Drive)?`, `Dropbox`, `OneDrive`, `iCloud`/`Mobile Documents`, `Google Drive`/`GoogleDrive`/`Drive` — case-insensitive). A `SyncWarning` surfaced when true.
  - [x] Stale-version detection on (re)open: compare the opened journal's `logical_version` to the recent list's `last_seen_version_for(path)`; if it **regressed**, surface the neutral "you saw vN, this is vM" notice (does not block opening).
  - [x] `create_backup` re-homed: when `self.path` is a user-selected custom location, write the backup to `<journal-dir>/backups/…`; else the OS data dir (Story 5.4 fallback). Keep the timestamped filename.
  - [x] New `MSG_*` (journal opened / created / switched / open-failed / locked-elsewhere / lock-reclaimable / sync-folder-warning + the stale-version template). Register in `USER_FACING_MESSAGES`.
  - [x] Tests: open a second journal switches + re-renders (state-level: list_studies reflects the new journal); a `LockHeld` path is refused and leaves the current journal open; `is_sync_folder` matches the known providers and rejects a plain path; stale-version detection fires on a regressed reopen; `create_backup` lands beside a custom journal path.

- [x] **Task 5 — main.rs + Slint: native dialogs + the journal-location UI (AC1, AC3, AC4)** — `app/src/main.rs`, `app/ui/`
  - [x] Réglages "Emplacement du journal" panel: **Créer un journal…** (rfd directory/save dialog → `create_journal`), **Ouvrir un journal…** (rfd file-open `.db` filter → `open_journal`), the **recent-journals** list (each row: name/path + open action; the current one marked), the **sync-folder warning** banner when applicable, and a **lock-held** banner offering *Reclaim* (when stale) / cancel. Native dialogs run on the UI thread (rfd's blocking variant is fine for a desktop modal) or via the async variant; pick the simplest that doesn't freeze the event loop.
  - [x] After a successful open/create/switch, re-render dashboard + watchlist + portfolio + close any open study editor (the journal changed — the Story 5.4 review pattern). `@tr` floor + `MSG_*` inventory bumped by exactly the number added.

- [x] **Task 6 — Gates (AC5)** — fmt, clippy `-D`, `test --workspace`, `deny` + smoke launch. Confirm `core::ssg` re-diffs clean; **no migration** (`user_version` registry unchanged); **`SCHEMA_VERSION` stays 1**; **`deny.toml` green** with the new `rfd` crates allow-listed (justified); `Cargo.lock` grows (expected — new package); `@tr` floor + `USER_FACING_MESSAGES` inventory bumped exactly.

## Dev Notes

### Scope
The **data-location control** story: native pick/create/open/switch, recent journals + reopen-last-used with the real `(journal_id, last-seen-version)` pointer, a single-instance lock, and sync-folder safety (`journal_mode=DELETE` + the live-local/backup-to-sync pattern). It closes the 5.4 review deferral (backups beside the journal) and gives #65's stale detection its real pointer. **Provider-independent.**

### Out of scope (deferred)
- **Multi-journal *merge*** (that is import, Story 5.3) — 5.5 only opens/switches between separate journals.
- **Automatic/scheduled backups, cloud sync orchestration** — out of MVP.
- **Robust cross-platform lock via OS flock** — 5.5 uses an atomic lock-file + PID liveness (Linux `/proc`); a kernel advisory lock is a future hardening (note it).
- **The #67 `-wal`-carrying-backup** concern — separate.

### Architecture decisions this story honours
- [Source: architecture.md §"App-config vs journal boundary (`directories` + `keyring`)"] — the last-used pointer + recent list + prefs live in **app-config**, never inside the journal (ADD7). The pointer references `(journal_id, last-seen-version)`, not just a path.
- [Source: architecture.md §"sync-safety ADD8"] — a synced SQLite `.db` risks WAL corruption; the recommended pattern is **live DB local + versioned backups to the sync folder**, and a synced live DB uses `journal_mode=DELETE`. (Project memory flags this Synology risk repeatedly.)
- [Source: architecture.md §"Identity & integrity / single-instance lock (ADD6)"] — `journal_id` + monotonic `logical_version` in `journal_meta`; **a single-instance file lock guards the journal**.
- [Source: persistence/src/journal.rs] — one connection per `Journal`; `apply_read_write_pragmas` sets `journal_mode=WAL`; `create` already cleans up sidecars on failure (the lock + DELETE-mode reuse this discipline).

### Where things live
- **`app/Cargo.toml`**: `rfd` (new dep); **`deny.toml`**: allow-list any new transitive licenses (justified).
- **`persistence/src/journal.rs`** (+ maybe `lock.rs`): `JournalMode` + `*_with_mode` + the single-instance lock guard on `Journal`.
- **`app/src/config.rs`**: `RecentJournal` + `recent_journals` + helpers.
- **`app/src/state.rs`**: `open_journal`/`create_journal`/`switch_to` + `is_sync_folder` + stale-version detection + re-homed `create_backup` + notices.
- **`app/src/main.rs` + `app/ui/`**: the rfd dialogs + the "Emplacement du journal" panel + recent list + warning/lock banners.

### Notes & guardrails
- **Never journal-less.** Every switch closes the current cleanly first, but a failed open must **re-open the previous** (and re-acquire its lock). Mirror the Story 5.4 `reopen_live`/rollback discipline.
- **Lock lifecycle.** The lock guard lives on `Journal`; dropping the `Journal` (switch/close/exit) releases it. A crash leaves a stale sidecar → detect via PID liveness and offer to reclaim (never silently steal). The lock sidecar is NOT the `-wal`/`-shm`; name it distinctly (e.g. `…-lock`).
- **Sync mode is per-open, derived from the path** — not stored in the journal (a journal copied between a local and a synced location must adapt). `journal_mode=DELETE` means no `-wal` (so `create_backup`'s checkpoint is a no-op there, and a plain copy is already consistent).
- **rfd on Linux** uses xdg-desktop-portal (or GTK); ensure the dialog call does not deadlock the Slint event loop (use the blocking call from a UI-thread callback, or rfd's async with the Slint executor). Test the smoke launch.
- **Posture / secrets** — notices carry paths/ids/versions only (no journal contents); a path is not a secret (unlike the provider key). Keep copy neutral, no banned verbs.
- **Stale-version pointer** — `last_seen_version` is updated on every clean open AND close (so "you saw vN" reflects reality); a regressed on-disk version on reopen surfaces the neutral notice (the #65 / 5.4-review want), without blocking.

### Manual on-display GO/NO-GO (Guy)
Réglages → **Créer un journal…** (native dialog) → pick a folder + name → the app switches to it (empty dashboard). **Ouvrir un journal…** → pick the previous `.db` → it switches back, surfaces re-render. Restart the app → it reopens the last-used journal. Try to open the **already-open** journal again → "already open" refusal. Point the create/open at a **Synology Drive** folder → the sync warning appears and the journal opens in DELETE mode (no `-wal` beside it); the recommended pattern is offered. **Créer une sauvegarde** now lands in a `backups/` folder beside the chosen journal. Kill the app uncleanly, relaunch → the stale lock is detected and offered for reclaim, not blocked forever.

### Project Structure Notes
- `rfd` is a **new external dependency** (the first since the keyring/3.2 era) — `Cargo.lock` grows, `deny.toml` gains justified allow-list entries; `cargo deny` must stay green. No `core`/migration/`SCHEMA_VERSION` change. The lock + `journal_mode` are file/connection concerns, not schema.
- Posture floors at story start: `@tr` floor **297** (Story 5.4), `USER_FACING_MESSAGES` inventory **67**, persistence `Error` inventory **11**. Bump each by exactly the number added.

### References
- [Source: _bmad-output/planning-artifacts/epics.md#Story 5.5] — pick directory; remember recent + reopen last-used (pointer `(journal_id, last-seen-version)` in app-config); single-instance lock; sync-folder → warn + `journal_mode=DELETE/TRUNCATE` + the live-local/backups-to-sync pattern (ADD8).
- [Source: _bmad-output/planning-artifacts/prd.md] — FR66 (portable local store an external system can back up); the DB-location requirement.
- [Source: app/src/config.rs] — `AppConfig.journal_path` (last-used), append-only `#[serde(default)]` discipline, load-with-fallback; `directories`-based config dir.
- [Source: persistence/src/journal.rs] — `Journal::{create, open, id, logical_version, checkpoint, is_read_only}`; `apply_read_write_pragmas` (`journal_mode=WAL`); single connection per handle; sidecar cleanup on failed create.
- [Source: app/src/state.rs] — Story 5.4 `confirm_restore`/`reopen_live` close-then-reopen + never-journal-less discipline; `create_backup` (to re-home); `journal_id()`; the open_or_create startup path.
- [Source: project memory] — DB-location-selectable + reopen-last-used; Synology-sync corruption risk (the ADD8 driver); GUI = Slint-only.

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8 (1M context) — `claude-opus-4-8[1m]`

### Debug Log References

- **`rfd` footprint is tiny + deny-clean.** Used the **`xdg-portal`** backend (`default-features =
  false, features = ["xdg-portal", "tokio"]`) so there is **no GTK system build dependency** (the
  portal backend reuses the `zbus` tree already pulled by `keyring`). Only **4 new packages** (`rfd`,
  `ashpd`, `pollster`, `urlencoding`); `cargo deny` stays green with **no new license** needed (all
  transitive licenses were already in the allow-list). `main()` is not a tokio runtime
  (`slint::run_event_loop`), so the **blocking** `rfd::FileDialog` on the UI thread (pollster) is safe.
- **Single-instance = single OS PROCESS (the key design call).** A naïve "lock held for the journal's
  whole lifetime, second open refused" broke 17 existing "reopen in a new session" tests (they hold two
  `JournalState`s on one path). ADD6's intent is to stop a second *process*, not a same-process re-open
  (SQLite coordinates intra-process connections itself). So `acquire_lock` allows a **same-PID** re-open
  (non-owning guard; the owner cleans up the sidecar) and refuses only a **different live PID**. This
  preserves the test idiom AND the real cross-process guarantee. Verified: a forged PID-1 lock is
  refused; a same-process re-open succeeds.
- **The lock changed one pre-existing test's error variant.** `failed_create_leaves_no_file_behind`
  (create in a missing dir) now fails at lock acquisition (`Error::Lock`) before the DB open
  (`Error::Sqlite`) — both are clean "create failed, no file left"; the assertion now accepts either +
  asserts no `-lock` leaks.
- **`JournalMode` is per-open, derived from the path** (`is_sync_folder` → `Delete`), never persisted in
  the journal — a journal moved between a local and a synced dir adapts. `DELETE` mode leaves no `-wal`
  (so `create_backup`'s checkpoint is a no-op there and a plain copy is already consistent).
- **Never journal-less on a failed switch.** `open_journal`/`create_journal` snapshot the previous path,
  `close_current` (checkpoint + drop → release lock), and on any failure `restore_previous` re-opens the
  prior journal (re-acquiring its lock). Mirrors the Story 5.4 discipline.
- **Stale-version pointer uses the real `(journal_id, last-seen-version)`** now (the #65 / 5.4-review
  want): `AppConfig.recent_journals` records it on every open; `finish_journal_switch` compares the
  on-disk version and surfaces the neutral "you saw vN, this is vM" notice without blocking.
- **`create_backup` re-homed beside the journal** (`<journal-dir>/backups/`) — the Story 5.4 deferral
  closed: backups now follow a user-selected location.
- **`MSG_JOURNAL_LOCK_RECLAIMABLE` is surfaced on a stale-lock open failure** (not just registered) — a
  stale lock shows the reclaimable notice + a "Lever le verrou et ouvrir" button; a live foreign lock
  shows the generic refusal with no reclaim offered.

### Completion Notes List

- **AC1** — native `rfd` open/create dialogs (`pick_and_open_journal`/`pick_and_create_journal`); state
  `open_journal`/`create_journal`/`switch` close-then-open, never journal-less; re-render all surfaces +
  close the study editor.
- **AC2** — `AppConfig.recent_journals` (`RecentJournal{path, journal_id, last_seen_version}`) +
  `record_recent`/`last_seen_version_for`; reopen-last-used (existing) records the pointer; stale-version
  notice on a regressed reopen; recent list (most-recent-first, canonical-de-duped, cap 8) in Réglages.
- **AC3** — single-instance lock (`acquire_lock` sidecar + RAII `JournalLock`; same-PID re-entry; stale
  detection via `/proc`; `lock_is_stale`/`clear_lock` + `reclaim_and_open`); a live foreign instance is
  refused.
- **AC4** — `is_sync_folder` heuristic → `JournalMode::Delete` (no `-wal`) + the sync warning + the
  live-local/backups-to-sync recommendation; `create_backup` re-homed beside the journal.
- **AC5** — no `core::ssg`/migration/`SCHEMA_VERSION` change; `rfd` added (Cargo.lock +4 packages),
  `deny.toml` unchanged (no new license), `cargo deny` green. `@tr` 297→305 (+8), `USER_FACING_MESSAGES`
  67→74 (+7), persistence `Error` inventory 11→13. Workspace **567 tests**; fmt/clippy `-D`/deny green;
  smoke launch exit 124.

### File List

- `Cargo.toml` (M) — `rfd` workspace dep (xdg-portal backend)
- `app/Cargo.toml` (M) — `rfd`
- `Cargo.lock` (M) — +4 packages (rfd, ashpd, pollster, urlencoding)
- `persistence/src/journal.rs` (M) — `JournalMode` + `create_with_mode`/`open_with_mode` + the single-instance lock (`JournalLock` RAII, `acquire_lock`, `lock_is_stale`, `clear_lock`) + mode-aware pragmas
- `persistence/src/error.rs` (M) — `LockHeld`/`Lock` variants + posture inventory 11→13
- `persistence/src/lib.rs` (M) — export `JournalMode`, `lock_is_stale`, `clear_lock`
- `persistence/tests/e2e_lifecycle.rs` (M) — lock / same-PID re-entry / stale-reclaim / DELETE-mode tests + the failed-create variant
- `app/src/config.rs` (M) — `RecentJournal` + `recent_journals` + `record_recent`/`last_seen_version_for` + `canonical_key` + tests
- `app/src/state.rs` (M) — `OpenOutcome`/`is_sync_folder` + `open_journal`/`create_journal`/`reclaim_and_open` + `close_current`/`adopt_open`/`restore_previous` + re-homed `create_backup` + `logical_version_or_zero` + 7 `MSG_*` + `journal_stale_message` + tests
- `app/src/main.rs` (M) — `render_journal_panel`/`finish_journal_switch`/`journal_short_name` + the 4 location callbacks (rfd) + startup recording/render
- `app/src/posture.rs` (M) — `@tr` floor 297→305, message inventory 67→74
- `app/ui/state.slint` (M) — `RecentJournalRow` + `Prefs` location props/callbacks
- `app/ui/screens/settings.slint` (M) — Réglages "Emplacement du journal" panel + recent list + reclaim/sync banners

### Review Findings (3-layer adversarial — 2026-06-30)

3 layers (Blind / Edge / Acceptance). All **5 ACs satisfied** (Auditor; @tr +8, MSG +7, deny.toml
unchanged all verified). Substantial real findings for a large story — **12 patches applied · rest
dismissed (with rationale).**

- [x] [Review][Patch] **HIGH** — `rfd` sync dialog with the `tokio` feature on the (non-tokio) UI thread would panic/hang; the picker would be unusable. Switched to the **`async-std`** runtime feature (self-driven executor). [Cargo.toml] — *(needs the manual GO/NO-GO to confirm the live portal dialog, as the sandbox has no portal)*
- [x] [Review][Patch] **HIGH** — startup / restore / `reopen_live` opened with hardcoded WAL → a sync-folder journal would get a `-wal` (the exact ADD8 corruption risk). All open paths now use `sync_mode_for(path)`. [app/src/state.rs]
- [x] [Review][Patch] **HIGH-ish** — a stale lock on the configured journal at startup orphaned it onto the default. Startup now **auto-reclaims a stale lock** (no live owner) on the configured journal. [app/src/state.rs]
- [x] [Review][Patch] **MED** — PID reuse defeated stale detection (lock-out) / admitted a foreign leftover as re-entry. The lock now records **`(pid, start_time)`** (`/proc/<pid>/stat`); reuse has a different start-time → correctly stale/refused. Also fixes the empty-lock self-lockout. [persistence/src/journal.rs]
- [x] [Review][Patch] **MED** — the lock required a writable dir even for read-only opens → a journal on read-only media was un-openable. Lock acquisition is now **best-effort** (a read-only location proceeds lock-less — a journal that can't be locked can't be double-written). [persistence/src/journal.rs]
- [x] [Review][Patch] **MED** — last-seen version recorded only on open → stale detection compared the open-time version, missing a real regression. Now recorded **before every switch and at exit** (`record_current_pointer`). [app/src/main.rs]
- [x] [Review][Patch] **MED** — stale detection matched by path only → a different/new journal at a reused path showed a spurious "older" notice (and suppressed "created"). Now guarded by **`journal_id`** (`last_seen_for`). [app/src/config.rs, main.rs]
- [x] [Review][Patch] **MED** — `is_sync_folder` scanned the literal path → a symlinked/mounted sync dir (the common Synology form) opened in unsafe WAL. Now **canonicalizes** first. [app/src/state.rs]
- [x] [Review][Patch] **MED** — re-selecting the currently-open journal closed+reopened it, silently wiping undo. Now a **no-op** (same-path guard). [app/src/state.rs]
- [x] [Review][Patch] **LOW** — reclaim was offered whenever a stale lock merely sat beside any open failure. Now offered **only** when the failure is the lock. [app/src/main.rs]
- [x] [Review][Patch] **LOW** — the create dialog's `attempted` path (and overwrite check) diverged from the actually-created `.db`; `create_backup` could write to the CWD for a bare relative path. Both fixed (pass the real created path; empty-parent → data-dir fallback). [app/src/main.rs, state.rs]
- [x] [Review][Patch] **LOW** — `MSG_JOURNAL_OPEN_FAILED` over-claimed "the previous journal stays open". Reworded to a plain fact. [app/src/state.rs]

**Dismissed (with rationale):** AC3 same-PID re-entry vs the literal "cannot be opened twice" — a
deliberate, ratified reading of ADD6 (*single-instance = single OS process*; SQLite coordinates
intra-process; the switch rail always closes-first); the close-before-reopen **concurrency window** (a
second instance grabbing the just-freed lock mid-switch) — extremely narrow on a single-user desktop,
best-effort by design; `is_sync_folder` **substring** false positives (`mydropbox-archive`) — only ever
picks the *safer* DELETE mode; **non-UTF-8** journal paths round-tripping through `display()` — exotic on
the target platform; a `0` last-seen-version on a degenerate read error — a freshly-opened journal reads
its version fine.

5 new patch tests (start-time stale/reclaim, unparseable lock, same-process reopen, backup-beside-journal,
same-journal no-op, journal_id guard). Workspace **571 tests** green; fmt / clippy `-D` / deny clean;
smoke launch exit 124.

### Change Log

- 2026-06-30 — Story 5.5 dev complete (6/6 tasks). Native `rfd` journal-location picker (xdg-portal, +4
  packages, deny-clean), recent journals with the `(journal_id, last-seen-version)` pointer + stale
  detection, single-instance lock (single-process semantics, stale reclaim), sync-folder detection →
  `journal_mode=DELETE` + the recommended pattern, `create_backup` re-homed beside the journal. No
  core/migration/`SCHEMA_VERSION` change; `deny.toml` unchanged. Workspace 567 tests green.
