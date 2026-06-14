# Story 2.12: Dashboard search/sort/filter, archive & delete

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want to manage many saved studies — list, search, sort, filter, archive and delete them,
so that I can find and curate my work without it becoming an unmanageable pile.

## Acceptance Criteria

(From epics.md §Story 2.12, lines 698–710. BDD, verbatim intent. Scope-resolved 2026-06-14 — see Dev Notes "Scope decision".)

1. **Given** several saved studies, **when** I use the dashboard, **then** I can **list, search, sort and filter** them and **open** any one (FR54): the list shows ticker + created-date + status; a **search** field filters by ticker (case-insensitive substring); a **sort** control orders by created-date or ticker (both directions); a **status filter** shows *active* (default), *archived*, or *all*. Opening a row reuses the existing `Studies.open-study(id)` rail.
2. **Given** an active study, **when** I **archive** it (with confirmation), **then** its `studies.status` flips to `'archived'`, it is **hidden from the default (active) view**, and the action is **reversible** (un-archive restores it to active). Archiving touches **no** `judgment`/time-series row and **no** Study blob — it is a pure status change (FR55).
3. **Given** a study, **when** I **delete** it (with an explicit, clearly-destructive confirmation), **then** the study row **and its own `judgments` time-series rows** are removed **atomically in one transaction**, leaving **every other study and the journal intact** — never an orphaned/dangling row, never a half-delete (FR55 "without corrupting the journal time-series"). Delete is **irreversible** (distinct from archive's reversible hide).
4. **Given** the archive/delete confirmation, **then** the prompt is a **neutral, fact-stating** message (FR13 — no banned verb), the delete prompt makes the **irreversibility explicit**, and **cancel mutates nothing** (the Story-2.5 unlock-all request→confirm→cancel pattern, reused). The confirm/cancel overlay renders on the **dashboard** surface.
5. **Given** persistence, **then** archive (one `UPDATE`), un-archive (one `UPDATE`) and delete (one multi-statement **transaction**) are atomic, bump `logical_version`, and use the read-only / no-journal / save-failure guards (a neutral notice on refusal, never a silent `.ok()`). Archiving/deleting on a read-only journal is refused with the read-only notice. **No `schema.rs` DDL change and no `SCHEMA_VERSION` bump** — the `status` column already exists in v1; delete uses the existing `judgments` FK relationship.
6. **Given** the Definition of Done for a UI story, **then** it is unit-tested (archive → hidden from active view, shown under archived/all, reversible; delete → row gone from `list_studies`, other studies intact, judgments rows for that study purged in the same tx; the pure search/sort/filter curation; read-only refuses both), the binary launches and runs the event loop, and the in-GUI click-through is a documented partial (human/AT-SPI, as 2.1–2.11). 4 CI gates green `--locked`; the frozen-corpus / journal-roundtrip persistence gates stay green; **`core`/`contract`/`Cargo.lock`/`deny.toml`/`rust-toolchain` re-diff unchanged** (see Scope decision).

## Tasks / Subtasks

- [x] **Task 1 — Persistence: archive, un-archive, delete (persistence crate, `studies.rs`)** (AC: 2, 3, 5)
  - [x] `Journal::set_study_status(id, status: &str) -> Result<()>`: `UPDATE studies SET status=?1 WHERE id=?2` + bump `logical_version` (one tx). Archive passes `"archived"`, un-archive passes `"active"`. (Or expose `archive_study`/`unarchive_study` thin wrappers — pick one shape, keep it minimal.)
  - [x] `Journal::delete_study(id) -> Result<()>`: **one transaction** — `DELETE FROM judgments WHERE study_id=?1; DELETE FROM studies WHERE id=?2;` + bump `logical_version`. Deleting the study's own time-series rows in the same tx avoids the FK `RESTRICT` violation and leaves no orphan (FR55). A missing id is a no-op success (idempotent), never an error.
  - [x] Do **not** alter `schema.rs` DDL (the `status` column + `idx_studies_status` already exist; the `judgments` FK already exists). No `SCHEMA_VERSION` bump.
  - [x] Persistence tests: archive flips status (re-`list_studies` shows `'archived'`); delete removes the row (gone from `list_studies`) **and** purges its judgments rows while leaving a second study's rows intact (insert a raw `judgments` row in the test to prove the purge + the FK-safe path); delete of an absent id is Ok.
- [x] **Task 2 — State + curation (app crate, `state.rs` + `viewmodel/studies.rs`)** (AC: 1, 2, 3, 5)
  - [x] `JournalState::archive_study(id)`, `unarchive_study(id)`, `delete_study(id)` → guarded (read-only / no-journal / save-failure → neutral notice, never `.ok()`) → call the persistence fn. These are **dashboard-level** actions, **not** part of the per-open-study undo stack (do NOT record undo; archive is reversed by un-archive, delete is intentionally irreversible). If the deleted/archived study is the currently-open one, clear/close it (mirror the existing "select Études closes the open study" nav rail).
  - [x] A **pure curation** function in `viewmodel/studies.rs`: `curate(summaries: &[StudySummary], query, sort_key, sort_dir, status_filter) -> Vec<StudyRow>` — case-insensitive ticker substring match, stable sort by created-date or ticker (asc/desc), status filter (active/archived/all). Deterministic, no I/O — the testable heart of AC1.
  - [x] Hold the current `query`/`sort`/`filter` as app state (e.g. `Rc<RefCell<DashboardView>>`); `refresh_studies` runs `list_studies()` → `curate(...)` → pushes `Studies.rows`.
- [x] **Task 3 — Dashboard callbacks + confirm flow (app crate, `main.rs` + `state.slint`)** (AC: 1, 2, 3, 4, 5)
  - [x] Slint callbacks (`state.slint`): `set-search(string)`, `set-sort(string, bool)` (key, descending), `set-status-filter(string)` → update the view state + re-curate + re-push. `request-study-action(string, string)` (action `"archive"`/`"unarchive"`/`"delete"`, study id), `confirm-study-action()`, `cancel-study-action()`; pushed state `study-action-confirm-visible: bool`, `study-action-message: string`, `study-action-destructive: bool` (so the UI can label the delete button distinctly).
  - [x] `main.rs`: mirror the unlock-all wiring (`request-unlock`→`count`→park→overlay; `confirm`→mutate→refresh; `cancel`→noop). On `request-study-action`, park `(action, id)` in `Rc<RefCell<Option<(String, Uuid)>>>`, build a neutral fact-stating message (delete = explicit irreversible wording, e.g. "Cette suppression est définitive."), set the overlay. On confirm, take the parked intent → call the matching `JournalState` fn → on Ok clear notice + `refresh_studies`; on Err set notice. On cancel, clear the parked intent + overlay (mutate nothing).
- [x] **Task 4 — Dashboard UI: search/sort/filter controls, row actions, confirm overlay (Slint, `dashboard.slint`)** (AC: 1, 2, 3, 4)
  - [x] A search `TextInput`, a sort selector (date/ticker + direction), and a status-filter selector (active/archived/all) above the list. Reuse existing primitives (`ChoiceChip`/`ActionButton`/`TextField`); ink-only where possible — **no new saturated colour** (status is shown as low-contrast text, as today).
  - [x] Per-row actions: an **Archive**/**Désarchiver** affordance (depending on current status) and a **Supprimer** affordance, each firing `request-study-action(...)`. Keep them visually subordinate to the row's open gesture (don't make a stray click destructive — the open gesture stays the row's primary action; the action buttons are explicit, separate hit targets).
  - [x] A confirm/cancel overlay on the dashboard (mirror the §-screen unlock overlay): `if Studies.study-action-confirm-visible`, show the message + a **Confirmer/Annuler** pair, with the destructive (delete) confirm clearly labeled (e.g. "Supprimer définitivement") when `study-action-destructive`.
- [x] **Task 5 — Gates, posture floors, DoD** (AC: 4, 6)
  - [x] Bump `posture.rs` floors for the new `@tr` labels (search/sort/filter/archive/delete/confirm microcopy). All must pass the banned-verb scan (neutral, fact-stating). User data (tickers) is never scanned.
  - [x] 4 CI gates green `--locked`; persistence **frozen-corpus** + **journal-roundtrip** gates stay green (no DDL change). `core`/`contract`/`Cargo.lock`/`deny.toml`/`rust-toolchain` re-diff **unchanged**. File List ⇄ git exact (issue #18).
  - [x] DoD: launch + run the event loop; in-GUI click-through = documented partial (human/AT-SPI). Don't mark `[x]` for a non-existent test.

## Dev Notes

### Scope decision (Guy, 2026-06-14) — READ FIRST

1. **Archive = soft + Delete = hard (atomic), the two are distinct** (chosen over all-soft and archive-only). **Archive**: `studies.status = 'archived'` — hidden from the default *active* view, **reversible** via un-archive. **Delete**: removes the study row **and its own `judgments` time-series rows in one transaction** — atomic, FK-safe, no orphan, **irreversible**. This realizes the epics AC's "removed/**hidden**" distinction (archive = hidden, delete = removed) "without corrupting the journal time-series" (FR55).
2. **Default dashboard view = active**, with a filter toggling active / archived / all; **search by ticker**; **sort by created-date and ticker** (both directions). Search/sort/filter are **pure app-side curation** over `list_studies()` — deterministic, headless-testable.
3. **`persistence` is a PRIMARY surface here** (archive/delete are SQL operations — their natural home), but **no `schema.rs` DDL change and no `SCHEMA_VERSION` bump**: the `status` column (`DEFAULT 'active'`, indexed `idx_studies_status`) and the `judgments` FK **already exist in v1**. **`core`/`contract` stay PINNED** (no method/verdict/Study-blob change — archive uses the SQL `status` column, NOT a new `Study.status` field). The frozen-corpus `v1.db` already carries the `status` column, so archive (UPDATE) / delete (DELETE) are ordinary operations against the frozen DDL.
4. **NOT this story:** writing the FR51 durable `judgments` time-series (still deferred → issue #34 — the table stays effectively empty today, which is exactly why a hard delete is safe now; the delete tx purges any judgments rows that *do* exist, so it is already correct for when #34 lands). NO provider/reconcile (Epic 3). NO export/import (Epic 5, FR-export).

### What exists today (reuse — do not reinvent)

- **Dashboard screen:** `app/ui/screens/dashboard.slint` — `DashboardScreen` (component ~line 81); `StudyListRow` (~18–79) shows `ticker` (58) + `created-at` (66) + `status` (73, currently the literal `'active'`); a row click / Enter / Space fires `open()` → `Studies.open-study(id)` (~171–173). **No search/sort/filter yet** (deferred here per the file's own comment ~line 3).
- **List model:** `Studies.rows: [StudyRow]` (`app/ui/state.slint` ~264); `StudyRow { id, ticker, created-at, status }` (~20–25). Pushed by `refresh_studies` (`app/src/main.rs` ~87–96): `state.list_studies()` → `viewmodel::studies::to_row()` → `VecModel<StudyRow>` → `Studies.set_rows(...)`.
- **List query:** `Journal::list_studies()` (`persistence/src/studies.rs` ~102–129): `SELECT id, security_ticker, created_at, status FROM studies ORDER BY created_at, id`. Returns `Vec<StudySummary { id, security_ticker, created_at, status }>` (~16–23). `JournalState::list_studies()` (`app/src/state.rs` ~413–425) wraps it (empty vec on error, no panic). **It already returns `status`** — curation reads it directly.
- **Studies table DDL** (`persistence/src/schema.rs` ~44–56): `status TEXT NOT NULL DEFAULT 'active'`, `CREATE INDEX idx_studies_status`. **`judgments`** (~60–67): `study_id TEXT NOT NULL REFERENCES studies(id)` (no `ON DELETE` → SQLite default RESTRICT; `PRAGMA foreign_keys = true` is set in `journal.rs`). ⇒ a hard delete **must** delete the study's judgments rows first (same tx) or RESTRICT would reject it once #34 writes rows.
- **Confirmation pattern (THE template):** the Story-2.5 "unlock all" flow. Slint (`app/ui/state.slint` ~332–348): `request-unlock(kind,arg)`, `confirm-unlock()`, `cancel-unlock()`, `confirm-visible`, `confirm-message`. Rust (`app/src/main.rs` ~697–760): request → `count_validated` → park scope in `Rc<RefCell<Option<…>>>` → set overlay; confirm → take parked → mutate → refresh; cancel → clear, mutate nothing. **Add a parallel `study-action` channel** (do NOT overload the unlock one — they coexist; the dashboard overlay is separate from the study-screen overlay).
- **Close-open-study-on-nav rail:** selecting "Études" already closes the open study (recorded in the 2.8 work). Reuse that close path if the open study is archived/deleted.

### Established conventions (carry forward)

- Cardinal Rule: no calculation here (dashboard curation is sort/filter/string-match, not arithmetic). No `.unwrap()`/`.expect()` in non-test code; no silent `.ok()`; time/IDs via the injected `Clock`/`IdGen`.
- Money/values cross as formatted strings; the row carries plain strings (id/ticker/date/status). No `Decimal`/enum into `.slint`.
- Colour budget: the dashboard spends **NO new saturated colour** — status/affordances are ink only (the §4 zone hues remain the only saturated spend). Neutral microcopy (FR13) for every new label and the confirm prompts; the delete prompt states irreversibility plainly (no scare/banned verb).
- Determinism: `list_studies` is `ORDER BY created_at, id` (stable); the app-side sort must be a **stable** sort with a deterministic tiebreaker (id) so the list never jitters.
- 4 CI gates `--locked`; `Cargo.lock`/`deny.toml` unchanged (no new dep); current app `#[test]` count **126** (you add to it); persistence tests also grow.

### Recorded traps to avoid

1. **FR55 / FK RESTRICT** — never `DELETE FROM studies` alone while a `judgments` row references it; delete the judgments rows in the **same transaction** first. Test it with a manually-inserted judgments row.
2. **No schema drift** — do NOT `ALTER TABLE` or add a column; the frozen `v1.db` corpus gate will break. `status` already exists. NO `SCHEMA_VERSION` bump (no `Study` blob field for status — use the SQL column).
3. **Don't fold archive/delete into the undo stack** — it is per-open-study (2.9) and reset on open; dashboard curation/lifecycle is a different axis. Archive is reversed by un-archive; delete is deliberately irreversible.
4. **Destructive-click safety** — the row's primary gesture stays *open*; archive/delete are separate, explicit hit targets behind a confirm overlay. A stray row click must never delete.
5. **Posture: scan labels, not data** — register the new `@tr` control/confirm labels; never scan tickers/search text (user data).
6. **Separate confirm channel** — the dashboard `study-action` confirm overlay is distinct from the study-screen `unlock` overlay (don't reuse `confirm-visible`/`confirm-message` for both, or one will clobber the other).
7. **File List ⇄ git exact** (issue #18); don't mark `[x]` for a missing test.

### Project Structure Notes

- Work spans **`app`** (state, viewmodel, main, dashboard.slint, state.slint, posture) **and `persistence`** (`studies.rs` — new fns + tests). **No `core`/`contract` change.** No new dependency.
- Slint/Rust naming: components `PascalCase`, `.slint` `snake_case`, props/callbacks `kebab-case` (`set-search`, `request-study-action`, `study-action-confirm-visible`).
- Files to touch: `persistence/src/studies.rs` (archive/un-archive/delete + tests), `app/src/state.rs` (guarded wrappers + close-open-if-affected), `app/src/viewmodel/studies.rs` (the pure `curate` + tests), `app/src/main.rs` (callbacks + confirm wiring + view state), `app/ui/state.slint` (callbacks + pushed view/overlay state), `app/ui/screens/dashboard.slint` (controls + row actions + overlay), `app/src/posture.rs` (floor bump).

### Tech stack (pinned)

- Rust workspace MSRV **1.96**; **Slint 1.16.1**; `rusqlite 0.40` (`bundled`). Linux-only dev/CI. 4 gates `--locked`.

### References

- [Source: epics.md#Story 2.12] (698–710: BDD AC). [Source: prd.md] FR54 (dashboard list/search/sort/filter/open), FR55 (archive/delete without corrupting the journal time-series), FR13 (neutral voice — system strings only).
- [Source: persistence/src/schema.rs:44-67] `studies` DDL (`status` + `idx_studies_status`) + `judgments` FK (RESTRICT). [persistence/src/journal.rs] `PRAGMA foreign_keys = true`. [persistence/src/studies.rs:16-23,102-129] `StudySummary` + `list_studies`.
- [Source: contract/src/versioning.rs:12] `SCHEMA_VERSION = 1` (unchanged). [contract/src/study.rs:81-97] `Study` (no status field — archive via SQL column).
- [Source: app/ui/screens/dashboard.slint] dashboard + `StudyListRow` + `open()`. [app/src/main.rs:87-96] `refresh_studies`. [app/src/viewmodel/studies.rs:14-21] `to_row`. [app/ui/state.slint:332-348] + [app/src/main.rs:697-760] the unlock-all confirm pattern to mirror.

## Open Questions (for Guy / dev — non-blocking, defaults chosen)

- **Q1 — Delete confirmation strength?** **Default:** a single, explicit confirm overlay whose confirm button is distinctly labeled "Supprimer définitivement" + an irreversibility message (the "double-confirmation" intent realized as one clearly-destructive step, consistent with the unlock-all single confirm). A literal two-step / type-the-ticker-to-confirm is heavier — confirm if wanted.
- **Q2 — Default sort?** **Default:** created-date descending (most recent first) on open; ticker sort available. Confirm vs date-ascending (current `list_studies` order).
- **Q3 — Where do row actions live?** **Default:** two explicit subordinate buttons per row (Archive/Désarchiver · Supprimer). Confirm vs an overflow "⋯" menu (heavier; defer).
- **Q4 — Un-archive discoverability?** **Default:** switch the status filter to *archived*/*all*, then each archived row shows a Désarchiver action. Confirm.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Opus 4.8, 1M context)

### Debug Log References

- `cargo test --workspace --locked` → all green (app 126→135, persistence 14→17). `cargo clippy --all-targets --locked` → clean.
- Posture floors bumped after measuring: `@tr` 163→177 (+14 dashboard literals), `USER_FACING_MESSAGES.len()` 14→20 (+6 confirm/done templates).
- Binary launches + runs the event loop (`cargo run`, exit 124).

### Completion Notes List

- **Persistence (primary surface, NO DDL change, NO `SCHEMA_VERSION` bump):** `Journal::set_study_status(id, status)` (UPDATE + version bump) and `Journal::delete_study(id)` (one tx: `DELETE FROM judgments WHERE study_id` then `DELETE FROM studies` + version bump — FK-safe, no orphan, idempotent on absent id). `status` column + `judgments` FK already existed in v1; frozen-corpus + journal-roundtrip gates stay green.
- **State wrappers (`state.rs`):** `archive_study`/`unarchive_study` (via a shared `set_study_status` guarded rail) + `delete_study` (guarded; `reset_undo()` so a deleted study can't be resurrected by Ctrl+Z). Read-only/no-journal/save-failure guarded. NOT undoable (dashboard-lifecycle axis, not per-open-study undo).
- **Pure curation (`viewmodel/studies.rs`):** `curate(summaries, query, SortKey, descending, StatusFilter)` — case-insensitive ticker substring + stable sort (date/ticker, asc/desc) with `id` tiebreaker. `SortKey`/`StatusFilter` with safe `from_wire`. 6 headless tests.
- **Wiring (`main.rs`):** the dashboard view state lives on the `Studies` global (search-query/sort-key/sort-descending/status-filter); `refresh_studies` reads it and curates. `set-search`/`set-sort`/`set-status-filter` callbacks re-curate. Archive/delete via a SEPARATE `study-action` confirm channel mirroring the 2.5 unlock pattern (request→park→overlay; confirm→act→refresh; cancel→noop); closes the open study if it is archived/deleted. Confirm/done copy = posture-gated `MSG_*` templates with `{t}` ticker interpolation (user data, not scanned).
- **UI (`dashboard.slint`):** search `TextField` (live `changed text`), status-filter + sort `ChoiceChip`s, per-row Archiver/Réactiver + Supprimer `ActionButton`s (separate hit targets — the row's open gesture stays primary), and a confirm banner (delete labeled "Supprimer définitivement"). Ink only, no new colour.
- **Scope honored:** app + persistence only; `core`/`contract`/`Cargo.lock`/`deny.toml`/`rust-toolchain`/`schema.rs` re-diff **empty** (verified). FR51 durable time-series still deferred (#34) — the delete tx already purges any judgments rows that exist.
- AC6 in-GUI click-through left as a documented partial (human/AT-SPI sandbox), as 2.1–2.11.

### File List

- `persistence/src/studies.rs` — `set_study_status` + `delete_study`
- `persistence/tests/journal_roundtrip.rs` — 3 archive/delete integration tests
- `app/src/state.rs` — `archive_study`/`unarchive_study`/`delete_study` + `set_study_status` rail; archive/delete `MSG_*` templates + `study_action_*_message` helpers + inventory; 3 tests
- `app/src/viewmodel/studies.rs` — `curate` + `SortKey`/`StatusFilter` + 6 tests
- `app/src/main.rs` — `refresh_studies` curates; 6 dashboard callbacks + `pending_study_action`
- `app/ui/state.slint` — dashboard view properties + study-action callbacks/overlay state
- `app/ui/screens/dashboard.slint` — search/sort/filter controls, row actions, confirm banner
- `app/src/posture.rs` — `@tr` floor 163→177, `USER_FACING_MESSAGES.len()` 14→20

### Senior Developer Review (AI)

3-layer adversarial review (Blind Hunter + Edge Case Hunter + Acceptance Auditor), 2026-06-14.

- **Acceptance Auditor: PASS AC1–AC6.** Scope verified independently — app + persistence only; `core`/`contract`/`Cargo.lock`/`deny.toml`/`rust-toolchain`/`persistence/src/schema.rs` re-diff empty; `SCHEMA_VERSION` still 1; delete genuinely atomic + FK-safe; all named tests present and asserting the AC claims.
- **2 patches applied:**
  - [x] [HIGH] Controls vanished when the active view curated to empty (archive the last active study → no chips to switch to "Archivées" → archived studies unreachable, un-recoverable by restart). Fix: push a total `study-count` and gate the controls on it (`study-count > 0`), not on the curated `rows.length`.
  - [x] [LOW] Phantom `logical_version` bump on an absent-id no-op (sensitive given the SQLite-on-Synology-sync stale-restore detection). Fix: bump the heartbeat only when rows were actually affected, in both `set_study_status` and `delete_study`.
- **Dismissed** (confirmed correct/safe by reviewers): FK delete order + one-tx atomicity + bound params (no injection); `on_confirm_study_action` borrow discipline (no double-borrow); `curate` sort totality/determinism (id tiebreaker, reverse stays total); live-search keeps field focus (text owned by the field, not bound back); double-click parks-then-takes once; no status value escapes active/archived; lexical RFC3339-Z sort = chronological. Latent stale-`current_study`-after-Retour is pre-existing and harmless (the `is_open` net + absent-study guards cover it).

### Change Log

- 2026-06-14 — Story 2.12 implemented: dashboard search/sort/filter + archive (soft, reversible) & delete (hard, atomic FK-safe time-series purge). app + persistence; no schema bump. app tests 126→135, persistence 14→17. Status → review.
- 2026-06-14 — Code review (3-layer): Acceptance PASS AC1–6; 2 patches applied (HIGH controls-reachability via total `study-count`; LOW no phantom version bump). 135 app + 17 persistence tests re-green, clippy clean.
