# Story 5.4: Restore from backup

Status: done (3-layer review 2026-06-30 — 4/4 ACs; 7 patches applied [self-restore guard, atomic temp+rename swap, TOCTOU re-validate, snapshot+rollback, close study editor, backup filename uniqueness, checkpoint is_read_only], 2 deferred → #67 + Story 5.5; workspace 557 tests, fmt/clippy -D/deny green; NO core/migration/SCHEMA_VERSION change; Cargo.lock/deny.toml unchanged)

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want to restore from a backup safely,
so that I never overwrite good data with an incompatible or corrupt file.

## Scope decision (Guy, 2026-06-29)

**The backup/restore unit is the raw `.db` file** (architecture §"Export / backup format": the raw `.db`
copy is the file-level NAS backup unit; the JSON envelope of Stories 5.2/5.3 is the *exchange/seed*
unit, not the restore unit). 5.4 is **self-contained**: it validates a candidate backup `.db`
(SQLite integrity + schema-version + journal identity) and compares it to the **current** journal,
**before** any overwrite, then applies the restore only after explicit confirmation.

**Out of this story (Story 5.5 territory):** the app-config `(journal_id, last-seen-version)` pointer
and the exact "you saw v57, this is v41" framing (5.4 compares the backup to the **current** journal's
live version — still catches an older/foreign backup); the native file picker; recent-journals /
journal-location selection; sync-folder detection + `journal_mode` switching. **Not this story:** the
JSON whole-journal import + its version arbitration (GitHub #65) — a different code path.

## Acceptance Criteria

1. **AC1 — Validate a backup `.db` BEFORE any overwrite (integrity + schema-version + identity, FR61/NFR-R5).** Pointing the app at a candidate backup `.db` file runs, **read-only and without mutating the backup** (no migration, no WAL write): a **SQLite integrity check** (`PRAGMA integrity_check`), a read of its **schema version** (`PRAGMA user_version`) and its **journal identity** (`journal_meta.journal_id` + `logical_version`). The result is a typed assessment — never a panic. A file that is not a journal (missing `journal_meta`), corrupt (integrity_check ≠ "ok"), or unreadable yields a typed refusal, **not** a restore. **Nothing is written to the live journal during validation.**

2. **AC2 — Surface identity/version + classify staleness/mismatch; never apply silently (FR61).** The assessment surfaces the backup's **`journal_id` and `logical_version`** and classifies it against the **current** journal: **Ok** (same `journal_id`, backup version ≥ current — a safe forward restore), **StaleOlder** (same journal, backup `logical_version` < current → "this backup (vN) is older than your current journal (vM)"), **ForeignJournal** (different `journal_id` → "this backup belongs to a different journal"), **NewerSchema** (backup `user_version` > this build supports → refuse, this build can't read it), **IntegrityFailed** / **Unreadable**. A restore is **never applied silently**: a StaleOlder or ForeignJournal restore requires an explicit **confirm** gesture (the established request→confirm/cancel pattern); NewerSchema/IntegrityFailed/Unreadable are hard refusals (no confirm offered).

3. **AC3 — Apply the restore atomically at the file level, then reopen + re-render (FR61).** On confirm, the live journal connection is **closed first**, then the backup `.db` is copied over the live path and the live **`-wal`/`-shm` sidecars are removed** (a stale WAL must not survive the swap), then the journal is **reopened** from the live path (running any pending forward migration on the now-restored file) and **every surface re-renders** (dashboard, watchlist, portfolio). If the copy fails, the prior journal is reopened from its original path (best-effort) and a neutral failure notice is shown — the app never ends with no journal. A read-only current journal (newer-schema) still permits restore (restore *replaces* it). A simple **"Créer une sauvegarde"** action copies the live `.db` to a `backups/` folder under the OS data dir (so a backup exists to restore and the round-trip is testable; this is a plain file copy, not the JSON export).

4. **AC4 — `core`/method untouched; no schema change; no new dependency; neutral posture.** The work is **`persistence` (read-only inspect + the file-swap helper) + `app` (state rail + confirm flow + UI)**. **No `core::ssg` change**, **no migration** (`PRAGMA user_version` registry unchanged — restore reuses the existing `Journal::open` migration path on the swapped file), **no `contract::SCHEMA_VERSION` bump**, **no new external dependency** (`std::fs` for the copy; `Cargo.lock`/`deny.toml` unchanged). Every new literal goes through `@tr`; the floor is bumped by exactly the number added; any new `MSG_*` is registered; copy is neutral, fact-stating (FR13).

## Tasks / Subtasks

- [x] **Task 1 — `persistence`: read-only backup inspection (AC1, AC2)** — `persistence/src/`
  - [x] New `persistence/src/restore.rs` (module `restore`, registered in `lib.rs`). `pub struct BackupInfo { pub journal_id: Uuid, pub logical_version: u64, pub file_user_version: i64, pub supported_version: u32, pub integrity_ok: bool }`.
  - [x] `pub fn inspect_backup(path: impl AsRef<Path>) -> Result<BackupInfo>`: open the file with **`SQLITE_OPEN_READ_ONLY`** (never read-write — must not migrate or WAL-write the backup). Run `PRAGMA integrity_check` (first row `== "ok"` → `integrity_ok`). Read `migrations::user_version` (→ `file_user_version`), `migrations::latest_version(REGISTRY)` (→ `supported_version`), and `journal_meta` (`journal_id`, `logical_version`) via the existing read helpers. A missing `journal_meta` table / row → `CorruptJournalMeta` (this file is not a journal). Never migrates, never mutates. `Uuid`/`u64` parsing reuses the journal.rs helpers (expose `read_journal_id` + a logical-version reader, or duplicate the two tiny reads locally).
  - [x] Reads that need a connection-level helper (`journal_meta` on an arbitrary read-only `Connection`) — factor a small `read_meta(&Connection) -> Result<(Uuid, u64)>` shared with `journal.rs` (or keep `restore.rs`-local).
  - [x] Unit tests: a fresh journal inspected → `integrity_ok`, correct `journal_id`/`logical_version`, `file_user_version == supported_version`; a non-journal SQLite file → `CorruptJournalMeta`; a truncated/garbage file → a typed error (no panic); inspecting **does not** change the backup's `logical_version` or file (read-only).

- [x] **Task 2 — `persistence`: the file-level restore swap (AC3)** — `persistence/src/restore.rs`
  - [x] `pub fn restore_journal_file(live_path: &Path, backup_path: &Path) -> Result<()>`: **precondition — the caller has already dropped every `Journal` handle on `live_path`** (no open connection). Copy `backup_path` → `live_path` (`std::fs::copy`), then remove `live_path` + `"-wal"`/`"-shm"` sidecars are stale → remove the **live** `-wal`/`-shm` (the backup is a single file; its WAL, if any, was checkpointed on its own close). Best-effort sidecar removal (ignore "not found"). Return a typed IO error on copy failure (map to `Error`).
  - [x] Integration test: create journal A (some rows) at path L; create a separate backup B at path P (different content/version); drop both handles; `restore_journal_file(L, P)`; reopen L → it now holds B's content + B's `journal_id`/`logical_version`; the live `-wal`/`-shm` are gone.

- [x] **Task 3 — App state: the restore request → confirm → apply rail (AC2, AC3)** — `app/src/state.rs`
  - [x] `pub enum RestoreVerdict { Ok, StaleOlder { backup: u64, current: u64 }, ForeignJournal, NewerSchema { found: i64, supported: u32 }, IntegrityFailed, Unreadable }` + `pub struct RestoreAssessment { pub journal_id: Uuid, pub logical_version: u64, pub verdict: RestoreVerdict }`.
  - [x] `request_restore(&mut self, backup_path: &str) -> Result<RestoreAssessment, String>`: `inspect_backup` → compare to the current journal (`self.journal_id()` + `logical_version`) → classify. **Parks** the candidate path + assessment in a pending-restore field (the request→confirm pattern, mirrors `request_unlock`/the dashboard study-action channel); applies nothing. A hard-refusal verdict (NewerSchema/IntegrityFailed/Unreadable) parks **no** pending (so confirm can't fire). Neutral notices for each refusal.
  - [x] `confirm_restore(&mut self) -> Result<(), String>`: only when a pending restore exists. **Drop the current `Journal`** (`self.journal = None`), call `restore_journal_file(live, backup)`, then `Journal::open(live)` → reassign `self.journal`/`self.read_only`; **reset undo history** (the restored journal is a different state). On a copy failure, reopen the original path (best-effort) so the app is never journal-less; surface a neutral failure notice. Clears the pending state.
  - [x] `cancel_restore(&mut self)`: clears the pending state (no write).
  - [x] `create_backup(&self) -> Result<PathBuf, String>`: copy the live `.db` to `data_dir/backups/journal-<journal_id>-v<logical_version>.db` (path returned for the notice). Guarded (no journal). Pure file copy; the live WAL is checkpointed implicitly by SQLite — for a faithful copy, run `PRAGMA wal_checkpoint(TRUNCATE)` on the live connection first (a read-safe checkpoint) so the `.db` is self-contained.
  - [x] New `MSG_*`: `MSG_BACKUP_CREATED`, `MSG_RESTORE_DONE`, `MSG_RESTORE_INTEGRITY`, `MSG_RESTORE_NEWER_SCHEMA`, `MSG_RESTORE_UNREADABLE`, `MSG_RESTORE_NOT_A_JOURNAL`, + a confirm-prompt template surfacing `(journal_id, version)` and the stale/foreign reason (a `{n}`-substitution like `unlock_confirm_message`). Register all in `USER_FACING_MESSAGES`.
  - [x] Tests: inspecting a fresh self-backup → `Ok` verdict; an older backup vs a bumped current → `StaleOlder` with the two versions; a different-journal backup → `ForeignJournal`; a non-journal file → `Unreadable`/`NotAJournal` and **no pending parked**; a confirmed restore swaps content + reopens (round-trip via two `JournalState`s / paths); a hard-refusal verdict cannot be confirmed.

- [x] **Task 4 — main.rs + Slint: the restore UI (AC2, AC3)** — `app/src/main.rs`, `app/ui/`
  - [x] Réglages "Sauvegarde & restauration" panel (next to "Journal complet"): **Créer une sauvegarde** (→ `create_backup`, notice with the path), a **path field** for the backup to restore + **Restaurer** (→ `request_restore`), a **confirm/cancel** banner that surfaces the backup's `(journal_id, logical_version)` + the stale/foreign warning (revealed only when a pending restore is parked), and the outcome notice. Path-based (native picker is Story 5.5).
  - [x] After a confirmed restore, re-render dashboard + watchlist + portfolio (the open study/journal changed). `@tr` floor + `MSG_*` inventory bumped by exactly the number added.

- [x] **Task 5 — Gates (AC4)** — fmt, clippy `-D`, `test --workspace`, `deny` + smoke launch. Confirm `core::ssg` re-diffs clean; **no migration** (`user_version` registry unchanged); **`contract::SCHEMA_VERSION` stays 1**; **`Cargo.lock` / `deny.toml` unchanged** (no new dep); `@tr` floor + `USER_FACING_MESSAGES` inventory bumped exactly.

## Dev Notes

### Scope
The **safe restore of a raw `.db` backup**: validate (SQLite integrity + schema-version + journal identity) **before** any overwrite, surface identity/version, classify and gate a stale/foreign restore behind a confirm, then swap the file and reopen. **Provider-independent.** Pairs conceptually with GitHub **#65** (the JSON import's version arbitration) but is a **distinct path** (#65 is the merge importer; this is a file-level replace).

### Out of scope (deferred)
- **`(journal_id, last-seen-version)` app-config pointer + the precise "you saw v57" framing** → Story 5.5. 5.4 compares the backup to the **current live** journal version (catches older/foreign backups without the pointer).
- **Native file picker, recent journals, journal-location selection, sync-folder detection + `journal_mode`** → Story 5.5.
- **JSON whole-journal import version arbitration** → GitHub #65 (the 5.3 importer, a different path).
- **Automatic/scheduled backups** → out of MVP.

### Architecture decisions this story honours
- [Source: architecture.md §"Export / backup format"] — the raw `.db` copy is the file-level NAS backup/restore unit; the JSON envelope is the exchange/seed unit. 5.4 restores the **`.db`**.
- [Source: architecture.md §"Identity & integrity"] — `journal_id` (UUID) + monotonic `logical_version` live in `journal_meta`; a restore must surface them and never silently overwrite a newer/foreign journal (FR61).
- [Source: architecture.md §"Reliability & Data Integrity — crash-safe/atomic writes; forward-safe migrations"] — the swap removes the stale live `-wal`/`-shm`; reopening runs any pending forward migration on the restored file via the existing `Journal::open` path.
- [Source: persistence/src/journal.rs] — `Journal::open` already opens a newer-schema file **read-only** (NFR-R3); a backup is inspected read-only so it is never mutated. The restore must drop the live handle before the file copy (a single connection per `Journal`).

### Where things live
- **`persistence/src/restore.rs`** (new): `BackupInfo` + `inspect_backup` (read-only validate) + `restore_journal_file` (the guarded file swap). No migration, no contract change.
- **`app/src/state.rs`**: `RestoreVerdict`/`RestoreAssessment` + `request_restore`/`confirm_restore`/`cancel_restore`/`create_backup` + the parked pending-restore state + neutral notices.
- **`app/src/main.rs` + `app/ui/`**: the Réglages "Sauvegarde & restauration" panel + confirm banner + outcome notices.

### Notes & guardrails
- **Never mutate the backup.** `inspect_backup` opens `SQLITE_OPEN_READ_ONLY` — opening read-write would migrate an older backup (changing it) and write a WAL. Read `integrity_check` / `user_version` / `journal_meta` on the read-only handle.
- **Drop the live handle before the swap.** A `Journal` holds one open connection; copying over an open SQLite file is unsafe. `confirm_restore` sets `self.journal = None` first, swaps, then reopens. Never leave the app journal-less on failure (reopen the original on copy error).
- **Stale WAL must die.** After the copy, remove the **live** `-wal`/`-shm` sidecars; a leftover WAL from the pre-restore journal would corrupt the restored file on reopen.
- **Self-contained `.db` for `create_backup`.** Run `PRAGMA wal_checkpoint(TRUNCATE)` on the live connection before copying so the backup `.db` carries all committed data (not split across a `-wal`). (Read-safe; does not change logical data.)
- **Confirm-before-act (FR61 "never silently").** Reuse the request→confirm/cancel pattern (Story 2.5 unlock-all / 2.12 dashboard action). A StaleOlder/ForeignJournal restore reveals the `(journal_id, version)` + reason and only applies on confirm. Hard refusals (integrity/newer-schema/unreadable) park no pending.
- **Undo reset.** A restore replaces the whole journal → `reset_undo` (the in-memory snapshot stack is meaningless across a different journal).
- **No secrets (NFR-S1).** Notices carry only ids/versions and a generic cause — never file contents or full paths in a leaking way.

### Manual on-display GO/NO-GO (Guy)
Réglages → **Créer une sauvegarde** → a `journal-<id>-v<n>.db` appears in `backups/`. Make a change in-app (so the live version advances), then **Restaurer** that older backup → a confirm banner shows the backup's `(journal_id, vN)` and warns it is **older** than the current (vM); confirm → the journal reverts to the backup's content and the dashboard/watchlist/portfolio re-render. Restore a backup from a different journal → "different journal" warning. Point at a non-`.db`/garbage file → neutral "unreadable / not a journal" refusal, no panic, nothing overwritten. (A newer-schema backup, once a future migration exists, → "written by a newer version" hard refusal.)

### Project Structure Notes
- Additive `persistence::restore` module (read-only inspect + file swap) + app state rail + confirm flow + Réglages panel. **No `core` change, no migration, no `SCHEMA_VERSION` bump, no new external dependency** (`std::fs` only).
- Posture floors at story start: `@tr` floor **290** (after Story 5.3), `USER_FACING_MESSAGES` inventory **57**, persistence `Error` inventory **11**. Bump each by exactly the number added.

### References
- [Source: _bmad-output/planning-artifacts/epics.md#Story 5.4] — restore: integrity + schema-version checks **before** overwrite; journal_id/version shown; stale/mismatched backup surfaced ("you saw v57, this is v41") and never applied silently (FR61).
- [Source: _bmad-output/planning-artifacts/prd.md] — FR61 (restore with integrity + version-compatibility checks before overwrite); NFR-R5 (verify integrity + schema version; mismatched/corrupt rejected, never partially applied); FR66 (portable local store an external system can back up).
- [Source: persistence/src/journal.rs] — `Journal::open` (newer-schema → read-only), `read_journal_id`, `logical_version`, `create` (sidecar cleanup pattern), single connection per handle.
- [Source: persistence/src/migrations.rs] — `user_version(&Connection)`, `latest_version(REGISTRY)`; the forward-migration runs on `open`.
- [Source: persistence/src/error.rs] — `CorruptJournalMeta`, `NewerJournalSchema`, `Sqlite`; the posture inventory (sample_errors + exhaustive match + count) must be updated for any new variant.
- [Source: app/src/state.rs] — `request_unlock`/`confirm`/`cancel` confirm pattern; `journal_id()` accessor (Story 5.3); `reset_undo`; the `journal: Option<Journal>` + `path: Option<PathBuf>` handle model.
- [Source: GitHub #65] — the JSON whole-journal import's version arbitration (a *separate* path; not this story).

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8 (1M context) — `claude-opus-4-8[1m]`

### Debug Log References

- **Read-only + `immutable=1` to inspect a backup without touching it.** `inspect_backup` opens the
  candidate via a `file:…?immutable=1` URI with `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_URI` — a plain
  read-only open of a WAL-mode `.db` whose `-wal` is absent/stale can fail recovery; `immutable=1` tells
  SQLite the file won't change, skipping locking/WAL recovery and any sidecar write. A small
  percent-encoder handles `%`/`?`/`#`/space in the path. `read_meta` maps a missing `journal_meta`
  table/row to `CorruptJournalMeta` (→ "not a journal"), so a foreign SQLite file is a clean refusal.
- **The swap removes the LIVE stale sidecars, not the backup's.** `restore_journal_file` copies the
  backup over the live path then removes the live `-wal`/`-shm` — a leftover WAL from the pre-restore
  journal would corrupt the restored file on reopen. The test asserts the sidecars are gone
  **immediately after the swap** (a subsequent `Journal::open` legitimately recreates fresh ones).
- **`create_backup` checkpoints first.** Added `Journal::checkpoint()` (`PRAGMA
  wal_checkpoint(TRUNCATE)`, no-op on a read-only handle) so the copied `.db` is self-contained (no
  recently-committed data stranded in a `-wal`). `create_backup` writes to
  `data_dir/backups/journal-<id>-v<version>.db`.
- **Confirm-before-act via a parked `PendingRestore`.** `request_restore` validates + classifies +
  parks the candidate path; `confirm_restore` drops the live `Journal` handle (one connection per
  handle — copying over an open file is unsafe), swaps, reopens (running any pending forward migration),
  and `reset_undo`s (the undo stack is meaningless across a different journal). On a copy/reopen failure
  the original is reopened best-effort (`reopen_live`) so the app is never journal-less. Hard-refusal
  verdicts (integrity/newer-schema/unreadable/not-a-journal) park **nothing**, so confirm can't fire.
- **`PendingRestore` carries only the path** (not the assessment) — the assessment was already surfaced
  by `request_restore`'s return; storing it again was dead state (clippy `dead_code`). Test
  observability uses a `#[cfg(test)] has_pending_restore()` helper (the project's `undo_depth()`
  pattern), not a prod accessor.
- **Compares against the CURRENT journal version, not a last-seen pointer.** The "you saw v57" framing
  needs the app-config `(journal_id, last-seen-version)` pointer that Story 5.5 introduces; 5.4 compares
  the backup to the live journal's current version (still catches an older/foreign backup). A no-journal
  state treats any valid backup as a forward `Ok`.
- **A backup-copy IO failure is wrapped in `Error::CorruptJournalMeta`** (no generic IO variant in
  `persistence::Error`); the app overrides it with `MSG_RESTORE_FAILED`, so the user sees a sane cause.
  Acceptable v1 (avoids a new Error variant + posture-inventory churn) — flagged for review.

### Completion Notes List

- **AC1** — `persistence::restore::{BackupInfo, inspect_backup}`: read-only + immutable validate
  (`integrity_check` + `user_version` + `journal_meta`), never mutates the backup. 4 tests.
- **AC2** — `state::{RestoreVerdict, RestoreAssessment, request_restore}`: classify Ok / StaleOlder /
  ForeignJournal / NewerSchema / IntegrityFailed vs the current journal; soft verdicts park a pending
  restore + surface `(journal_id, version)` + the stale/foreign warning (`restore_confirm_message`);
  hard refusals park nothing. 3 tests.
- **AC3** — `restore_journal_file` (swap + clear live WAL) + `state::{confirm_restore, cancel_restore,
  create_backup}` + `Journal::checkpoint`; close→swap→reopen→reset_undo, best-effort recovery on
  failure; re-renders all surfaces. 1 swap test + 1 confirm/cancel round-trip test.
- **AC4** — no `core::ssg` change, **no migration** (`user_version` registry unchanged — restore reuses
  `Journal::open`), **no `SCHEMA_VERSION` bump**, **`Cargo.lock`/`deny.toml` unchanged** (`std::fs`
  only, no new dep). `@tr` floor 290→297 (+7), `USER_FACING_MESSAGES` 57→67 (+10). Workspace **555
  tests**; fmt / clippy `-D` / deny green; smoke launch exit 124.

### File List

- `persistence/src/restore.rs` (A) — `BackupInfo` + `inspect_backup` (read-only/immutable) + `restore_journal_file` + 5 tests
- `persistence/src/lib.rs` (M) — register `restore` module + re-exports (`inspect_backup`, `restore_journal_file`, `BackupInfo`)
- `persistence/src/journal.rs` (M) — `Journal::checkpoint` (`wal_checkpoint(TRUNCATE)`)
- `app/src/state.rs` (M) — `RestoreVerdict`/`RestoreAssessment` + `request_restore`/`confirm_restore`/`cancel_restore`/`create_backup` + `reopen_live` + `pending_restore` field + 10 `MSG_*` + `restore_confirm_message` + 4 tests
- `app/src/main.rs` (M) — `on_create_backup`/`on_request_restore`/`on_confirm_restore`/`on_cancel_restore` callbacks (re-render all surfaces)
- `app/src/posture.rs` (M) — `@tr` floor 290→297, message inventory 57→67
- `app/ui/state.slint` (M) — `Prefs` restore callbacks/props (`create-backup`/`request-restore`/`confirm-restore`/`cancel-restore`/`restore-status`/`restore-confirm`)
- `app/ui/screens/settings.slint` (M) — Réglages "Sauvegarde & restauration" panel + confirm banner

### Review Findings (3-layer adversarial — 2026-06-30)

3 layers (Blind / Edge / Acceptance). All **4 ACs satisfied** (Auditor); @tr +7 / MSG +10 exact; no
core/migration/SCHEMA/Cargo.lock/deny.toml change. But the file-swap had real **data-safety holes** —
**7 patches applied · 2 deferred (#67, → 5.5) · rest dismissed.**

- [x] [Review][Patch] **CRITICAL** — restoring the journal onto itself zeroed it (`fs::copy(live, live)` truncates to 0). Added a `same_file_path` guard (canonicalized) → self-restore is a safe no-op. [app/src/state.rs] + the atomic swap below also removes the hazard.
- [x] [Review][Patch] **HIGH** — non-atomic in-place `fs::copy` could corrupt the live journal on a partial failure. `restore_journal_file` now does **copy-to-temp + `fs::rename`** (atomic; a failure leaves the live file untouched). [persistence/src/restore.rs]
- [x] [Review][Patch] **HIGH (TOCTOU)** — the validated path was not re-checked at confirm. `confirm_restore` now **re-`inspect_backup`s** and refuses a now-corrupt/newer-schema/unreadable file **without touching** the live journal. [app/src/state.rs]
- [x] [Review][Patch] **MED** — a reopen failure after a successful swap left the app journal-less with no rollback. `confirm_restore` now checkpoints + **snapshots** the live journal before the swap and **rolls back** to it if the restored file won't open. [app/src/state.rs]
- [x] [Review][Patch] **MED** — the open study editor was not closed across the swap (a stale form could save an old `study_id` into the restored journal). The confirm handler now closes the study view on success. [app/src/main.rs]
- [x] [Review][Patch] **MED** — backup filename keyed only on `(id, version)` silently overwrote a prior backup. Now includes the injected-clock timestamp. [app/src/state.rs]
- [x] [Review][Patch] **LOW** — `checkpoint()` guard tested `newer_file_version` instead of `is_read_only()`. Fixed (equivalent today, robust to future read-only causes). [persistence/src/journal.rs]
- [x] [Review][Defer] **MED** — an externally-copied un-checkpointed backup can silently drop its `-wal` data (`immutable=1` + main-file-only copy). Deferred → **GitHub #67** (app-made backups checkpoint first, so they are safe; documented in `restore.rs`). [persistence/src/restore.rs]
- [x] [Review][Defer] **LOW** — backups land in the default data dir, not beside a user-selected journal. Deferred → **Story 5.5** (journal-location selection; today the journal IS at the default path). [app/src/state.rs]

**Dismissed (with rationale):** `RestoreVerdict` omits the spec's `Unreadable` variant — behaviour is
correct via the `Err` path (unreadable/not-a-journal refuse before classification); copy IO error
wrapped in `CorruptJournalMeta` — the app overrides it with `MSG_RESTORE_FAILED` (never reaches the user
as "corrupt meta"); `CorruptJournalMeta` conflating "not a journal" vs a corrupt real journal — the
restore refuses either way.

2 new safety tests (self-restore no-op; confirm re-validates a tampered backup). Workspace **557 tests**
green; fmt / clippy `-D` / deny clean; smoke launch exit 124.

### Change Log

- 2026-06-29 — Story 5.4 dev complete (5/5 tasks). Safe restore of a raw `.db` backup: read-only +
  immutable validation (SQLite integrity + schema-version + journal identity) BEFORE any overwrite;
  classify Ok/StaleOlder/ForeignJournal/NewerSchema/IntegrityFailed vs the current journal; confirm-
  before-act (never silent, FR61); file-level swap (close→copy→clear-WAL→reopen→reset-undo) with
  best-effort recovery; `create_backup` (checkpoint + copy). Path-based Réglages UI (native picker is
  Story 5.5). NO core/migration/SCHEMA_VERSION change; `Cargo.lock`/`deny.toml` unchanged (std::fs).
  Workspace 555 tests green.
