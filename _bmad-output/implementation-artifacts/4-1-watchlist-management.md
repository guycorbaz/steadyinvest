# Story 4.1: Watchlist management

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want to maintain a watchlist of securities I'm interested in,
so that I can track candidates toward their buy zone.

## Acceptance Criteria

(From epics.md §Story 4.1, lines 837–848. FR34. The first Epic-4 story — it opens the watchlist persistence surface and carries the project's **first schema migration** (`user_version` 1 → 2). Scope-resolved in Dev Notes.)

1. **AC1 — Add / edit / remove / reorder a watched security; it persists and the list reflects it (FR34).** Given the Watchlist surface, when Guy **adds** a security (by ticker), **edits** it, **removes** it, or **reorders** the list, then the change is persisted to the journal's `watchlist_items` table and the list re-renders in the new order. Order is an explicit, contiguous `position` (0-based, ascending) maintained across all four operations.

2. **AC2 — Each entry can reference a saved study/snapshot for its zone (FR34).** A watchlist entry can be **linked to a saved study** (by the study's id) or left unlinked; the link can be set and cleared. This is the hook Story 4.2 (neutral buy-zone alerts) reads to know a watched security's zone. A linked study that is later **deleted** (Story 2.12 `delete_study`) must not leave a dangling reference — the watchlist row's `study_id` is cleared (set NULL) in the same delete, never an orphaned FK.

3. **AC3 — The first schema migration (`user_version` 1 → 2) adds the `study_id` column; forward-safe (NFR-R3).** The v1 `watchlist_items` table (`id, security_ticker, position, created_at`, created DDL-only by Story 1.10) gains a **nullable `study_id TEXT`** column via a new **migration step 2** appended to `migrations::REGISTRY`. An existing **v1 journal opens and migrates forward** to v2 automatically (the existing harness, `run_pending`); a v2 file opened by a v1-only build is refused read-only (already handled). The frozen `v1.db` corpus still opens + migrates + reads back its canonical study unchanged.

4. **AC4 — The SQL-schema axis moves, the serde-blob axis does NOT.** This bumps **`PRAGMA user_version` 1 → 2** only. **`contract::SCHEMA_VERSION` stays `1`** — the watchlist is a **normalized table** (typed columns), not a `contract::Study`-style serde blob, so no blob shape changes and **no `core`/`contract`/method change**. The method fingerprint / determinism / golden gate / the studies byte-pinned corpus shape are untouched (the migration only adds a column to an Epic-4 table the engine never reads).

5. **AC5 — Persistence CRUD on the `mutate`/`logical_version` rail.** New `Journal` methods — `add_watch_item` / `list_watch_items` / `update_watch_item` / `delete_watch_item` / `set_watch_position(s)` (reorder) — each mutating call runs in **one transaction that also bumps `journal_meta.logical_version`** (the studies.rs pattern, NFR-R2). `list_watch_items` returns rows ordered by `position`. Decimal/text discipline + injected `Clock`/`IdGen` (ADD15) reused (a watch item carries a `created_at` + a generated id).

6. **AC6 — A neutral, FR13-clean Watchlist surface in the app.** The app exposes a **Watchlist** screen (a new nav destination beside Études/Réglages) listing watched securities by ticker, each showing its optional study link, with add / edit / remove / reorder affordances. All prose `@tr()` + banned-verb-clean; tickers are user data (not scanned). Gates green `--locked`; the persistence + migration tests prove the data layer headlessly (the screen is the on-display GO/NO-GO residual).

## Tasks / Subtasks

- [x] **Task 1 — Schema migration v2: `watchlist_items.study_id` (AC2, AC3, AC4)**
  - [x] `migrate_to_v2` = `ALTER TABLE watchlist_items ADD COLUMN study_id TEXT` (`schema.rs`); `DDL_V1` untouched (frozen) — a v1 file gains the column on open.
  - [x] `REGISTRY` gains `(2, migrate_to_v2)`; the `latest_version == 1` test → `== 2`; the `fresh_database_migrates_to_v1`/`rerun`/`newer-file` tests updated to the new latest; the test-local `fake_v2`/`TWO_STEP_REGISTRY` scaffold repointed to **v3** on top of the real registry (`fake_v3`/`THREE_STEP_REGISTRY`), so the harness's append-a-future-step path stays proven; `readonly_newer.rs` `supported: 1` → `2`.
  - [x] **`contract::SCHEMA_VERSION` stays `1`** — contract crate **byte-untouched** (the two-axis distinction is documented in `migrations.rs`/`schema.rs`/`watchlist.rs` where it is load-bearing, rather than adding a contract doc line — cleaner: contract unchanged).
  - [x] Tests: `v2_adds_the_watchlist_study_id_column` (fresh → v2, column present, v1 columns intact); the existing `frozen_corpus_v1_opens_and_reads_back_the_canonical_study` (v1.db → open migrates the copy → reads back) stays green; the schema-drift table-set test stays green.

- [x] **Task 2 — Persistence: the `WatchItem` row + CRUD (AC1, AC2, AC5)**
  - [x] `WatchItem { id, security_ticker, position, study_id, created_at }` (new `persistence/src/watchlist.rs`, exported from `lib.rs`).
  - [x] `add_watch_item` (position = max+1) / `list_watch_items` (ORDER BY position) / `update_watch_item` / `delete_watch_item` (re-packs to contiguous) / `set_watch_positions` (reorder) — each on the one-tx `logical_version` rail, with **no-op guards** (`WHERE … IS NOT ?` + a `moved/changed > 0` check so an identical update/reorder/absent-delete writes nothing — the Epic-3 C4 lesson).
  - [x] **AC2:** `delete_study` now `UPDATE watchlist_items SET study_id = NULL WHERE study_id = ?` in the same delete tx (no orphan; counts toward the version bump).
  - [x] 7 integration tests (`persistence/tests/watchlist.rs`): ordering, study-link round-trip, reorder, delete-repack, version-bumps-but-not-on-no-ops, reopen, delete-study-clears-link.

- [x] **Task 3 — App state + view-model (AC1, AC2)**
  - [x] `JournalState` rails in `state.rs` (guarded read-only/no-journal): `list_watch_items` / `add_watch_item` (id+clock from ADD15) / `update_watch_item` / `delete_watch_item` / `move_watch_item(id, up)` (edge-safe swap). `watch_error` maps a persist error to a neutral notice. `MSG_WATCH_NO_STUDY` for the link-no-match case.
  - [x] The `WatchItem → WatchRow` adapter (ticker + resolved study-link ticker) lives in `main.rs::refresh_watchlist`. App tests: add/list/move/delete + edge no-op; blank-ticker refused; study-link set/clear (new `SeqIdGen` test double for distinct ids).

- [x] **Task 4 — Slint Watchlist screen + nav (AC6)**
  - [x] The nav already had a Watchlist destination + placeholder (1.10/2.1 scaffold) — fleshed out `watchlist.slint`: add-field, list rows (ticker · study-link · ▲/▼ reorder · Lier/Délier · Retirer), actionable empty state.
  - [x] `Watchlist` Slint global + `WatchRow` struct (`state.slint`, exported via `app.slint`); callbacks `add-watch`/`remove-watch`/`move-watch`/`link-watch`/`unlink-watch` wired in `main.rs` to the state rails → `refresh_watchlist`. Link = auto-match a same-ticker study (a picker is a later refinement; documented).
  - [x] FR13: all prose `@tr()`; `MSG_WATCH_NO_STUDY` registered (inventory `41 → 42`); `@tr` floor `227 → 236` (the fleshed-out screen).

- [x] **Task 5 — Gates (AC3, AC4)**
  - [x] All four gates green `--locked`: fmt ✓, `clippy -- -D warnings` ✓ (0 issues), `cargo test --workspace` ✓ (app 190, persistence + migration suites green), `cargo deny check` ✓. **Method fingerprint / determinism / golden / studies byte-pin clean** (no calc/blob change). `Cargo.lock`/`deny.toml` unchanged; **no `core`/`contract` change** confirmed.
  - [x] **No `v2.db` corpus file created** — the SQL `user_version` bump is proven by the forward-migration tests (v1.db opens + migrates), not a new frozen blob; the `frozen_corpus_v1` test stays green (its `user_version == 1` assertion is on the on-disk frozen file before `Journal::open` migrates the copy).

- [ ] **Task 6 — Manual on-display GO/NO-GO (AC1, AC2, AC6) — Guy on display** *(RESIDUAL — needs Guy's desktop.)*
  - [ ] On Guy's desktop: open the Watchlist; add a few tickers; reorder them (↑↓); link one to a saved study and confirm the link shows; unlink it; remove one; reopen the app and confirm the list + order + links persisted. Delete a linked study from the dashboard and confirm the watchlist entry survives with its link cleared (no orphan, no crash).

## Dev Notes

### Scope decision (the first migration is NARROW — the tables already exist)

A critical finding from the architecture analysis that **right-sizes this story**: **Story 1.10 already created the full Epic-4/6 table set in the v1 DDL** — `portfolios`, `holdings`, `transactions`, `fx_rates`, and **`watchlist_items` (`id, security_ticker, position, created_at`)** are all frozen in `schema.rs::DDL_V1`, DDL-only, awaiting their typed CRUD. So Epic 4 does **not** create new tables; it adds the typed read/write layer + UI on top of pre-provisioned schema.

- **The only schema change 4.1 needs** is a **nullable `study_id` column on `watchlist_items`** (FR34's "reference a saved study/snapshot"), which the v1 table lacks. That is the project's **first real migration step (v2)** — a one-line `ALTER TABLE ADD COLUMN` appended to the already-built, already-tested migration harness (`migrations.rs` even ships a `fake_v2`/`TWO_STEP_REGISTRY` placeholder proving the v2 path works).
- **Two version axes, only one moves (`migrations.rs` doc lines 7–10):** `PRAGMA user_version` (SQL schema, the migration trigger) goes **1 → 2**; `contract::SCHEMA_VERSION` (serde-blob) **stays 1** because the watchlist is a **normalized table**, not a `Study`-style blob. So **no `contract`/`core`/method change, no new `v2.db` corpus** — the studies blob shape and the frozen `v1.db` are untouched; the migration is proven by the forward-migration test (NFR-R3), not a new frozen file.
- **Retro action C3 ("open Epic 4 with the schema-migration story") is satisfied here, right-sized:** 4.1 IS the migration story, and the migration turns out small because 1.10 was forward-looking. The migration *harness* + the *forward-safe* discipline are the load-bearing parts, and they already exist + are tested.

### Out of scope (later Epic-4 stories)

- **Buy-zone alerts (4.2)** read the linked study's zone — 4.1 only provides the link.
- **Holdings register + reference currency (4.3)** + **per-holding price refresh (4.4)** + **trailing stop (4.5)** — separate stories on the `holdings`/`portfolios`/`transactions` tables; **the reference-currency + EODHD-paid-plan product decisions belong there, NOT 4.1** (a watchlist is just tickers + optional study links — currency-agnostic). 4.1 needs none of Guy's open product calls.
- **FX (4.x / Epic 6 [P2])** — `fx_rates` table stays DDL-only.

### What this story changes vs. preserves (files marked UPDATE — read before editing)

- **`persistence/src/schema.rs` (UPDATE)** — add `migrate_to_v2`. **Preserve `DDL_V1` byte-for-byte** (frozen); the new column arrives only via the migration step. The schema-drift test asserts the *table set* — a new column does not change it; keep it green.
- **`persistence/src/migrations.rs` (UPDATE)** — append `(2, migrate_to_v2)`; bump the `latest_version == 1` test assertion to `2`; reconcile the test-local `fake_v2`/`TWO_STEP_REGISTRY` with the now-real step 2. **Preserve** the harness logic (own-transaction-per-step, newer-file-refused, idempotency) — all already tested.
- **`persistence/src/studies.rs` (UPDATE)** — `delete_study` also nulls `watchlist_items.study_id` in the same tx (AC2). **Preserve** the FK-safe delete order + the `logical_version` bump.
- **`persistence/src/lib.rs` / new `watchlist.rs` (NEW)** — `WatchItem` + the CRUD methods (mirror `studies.rs`).
- **`app/src/state.rs` / `viewmodel/watchlist.rs` (NEW/UPDATE)** — the guarded app rails + the `WatchRow` adapter.
- **`app/ui/...` (NEW/UPDATE)** — `watchlist_screen.slint` + a `Watchlist` nav destination + global/callbacks; `main.rs` wiring; `posture.rs` floor bumps.

### Architecture & constraints

- **Migration harness (`migrations.rs`, Story 1.10):** versioned, ordered, own-transaction-per-step, `PRAGMA user_version`-stamped, append-only `REGISTRY`. A v2 step is the designed extension point (the `fake_v2` test proves it). Forward-safe: an old file opens and migrates; a newer file is refused read-only (NFR-R3, already handled).
- **The two version axes (architecture + `contract::versioning` + `migrations.rs`):** never conflate `user_version` (SQL) with `contract::SCHEMA_VERSION` (blob) with `core::METHOD_VERSION` (method). 4.1 moves only `user_version`.
- **`logical_version` (NFR-R2):** every mutating watch method bumps it in the mutation transaction — and, per the Epic-3 retro lesson (C4), a **no-op** call (e.g. a reorder that changes nothing, an update to identical values) should **not** write/bump. Add the value-equality / no-op pre-check the refresh rails learned (issue-class seen 3× in Epic 3).
- **Normalized vs blob:** studies are serde blobs (`payload TEXT`); the watchlist is **normalized columns** — so `WatchItem` is a plain row struct (no `#[serde(default)]` forward-compat dance, no blob version), and DDL is the schema-of-record. Decimals are TEXT (none here — watchlist has no money).
- **No new dependency:** `rusqlite` is already the persistence engine; `uuid`/`directories` already present. `Cargo.lock` unchanged.
- **The studies corpus byte-pin is unaffected** — the migration touches `watchlist_items`, never `studies`; the pinned canonical-study JSON (Story 2.2/3.4 shape) does not change.

### Previous-story intelligence (Epic 3 + 1.10)

- **The studies.rs CRUD shape is the template:** `put_study` = `INSERT … ON CONFLICT DO UPDATE` + a `journal_meta` `logical_version` bump in the same tx; `delete_study` = ordered FK-safe deletes in one tx; `list_studies` = a `SELECT` → `StudySummary`. Mirror it for `WatchItem`.
- **Epic-3 retro C4 (idempotency-guard):** `mutate_*` persists unconditionally — guard each new watch mutation so a no-op call writes no journal revision (the Synology-sync concern; the churn class bit Epic 3 three times).
- **Epic-3 retro C1/C3:** the on-display verification residuals are piling up — Task 6 adds one more; batch them with the 3.3–3.6 checks.
- **App nav pattern (Story 2.1/2.13):** the shell already has Études + Réglages nav destinations with `NavItem`; add Watchlist the same way. The dashboard's actionable empty-state + curate (2.12/2.13) is the list-surface pattern to mirror.
- **`delete_study` (2.12)** already deletes the study + its `judgments` rows in one tx; 4.1 adds the `watchlist_items.study_id` null in that same tx.

### Testing standards

- Headless Rust unit/integration tests (Slint-native, no-web — QA e2e N/A). The **migration + persistence CRUD are fully headless-provable**; the screen is the on-display residual (Task 6).
- **Migration tests** (persistence): fresh → v2; **v1 → v2 forward** (the load-bearing NFR-R3 test); column present; schema-drift table-set still green; the frozen `v1.db` corpus test stays green.
- **CRUD tests:** add/list/reorder/update/delete + ordering; study-link set/clear; delete-study clears the link; `logical_version` bumps on real change, **not** on a no-op; reopen-stable.
- All four gates `--locked`; pinned rustfmt 1.9.0; method/golden/studies-corpus clean.
- UI story → on-display GO/NO-GO is part of DoD (Task 6).

### Open questions for dev (resolve during implementation, don't block)

- **Reorder UX:** `↑/↓` move buttons (simplest, keyboard-reachable) vs drag-reorder. Leaning **↑/↓ buttons** for v1 (drag is a Slint nicety, deferrable); the persistence `set_watch_positions` supports either.
- **Position re-pack on delete:** re-pack to contiguous 0..n on every delete (clean, more writes) vs tolerate gaps and only normalize on reorder. Leaning **re-pack in the delete tx** (keeps `position` meaningful + contiguous; one tx).
- **Study link UX:** a picker from `list_studies` (ticker + date) vs auto-suggest by matching ticker. Leaning a **picker** (explicit; a watchlist ticker may have several studies or none).
- **Duplicate tickers:** allow the same ticker twice on the watchlist? Leaning **allow** (a user may watch a security against two different studies/scenarios) — no uniqueness constraint; the schema has none.

### Project Structure Notes

- The **first migration story** — `persistence` (schema/migrations/studies/new watchlist module) + `app` (state/viewmodel/ui). No `core`/`contract` change (only a doc line on `SCHEMA_VERSION`), no method/blob-schema change, no new dep, no new corpus file.
- Opens Epic 4 (Watchlist & single-portfolio risk). The watchlist link (`study_id`) is the seam Story 4.2 (buy-zone alerts) builds on.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.1 (lines 837–848) + Epic 4 framing (832–836)] — AC source.
- [Source: _bmad-output/planning-artifacts/prd.md (FR34 watchlist; FR35 buy-zone alert = 4.2; FR63 reference currency = 4.3; Journey-3 lines 328–342)] — requirements; the currency/provider decisions that belong to 4.3/4.4.
- [Source: persistence/src/schema.rs (DDL_V1: `watchlist_items` lines 112–118, the frozen table set; `migrate_to_v1`)] — the v1 schema the migration extends; **do not edit DDL_V1**.
- [Source: persistence/src/migrations.rs (the harness, `REGISTRY`, `run_pending`, `latest_version`, the `fake_v2`/`TWO_STEP_REGISTRY` tests)] — the append-only extension point + the proven v2 path + the test to update.
- [Source: persistence/src/studies.rs (`put_study`/`get_study`/`list_studies`/`delete_study`/`set_study_status` + the `logical_version` bump pattern, `StudySummary`)] — the CRUD template; `delete_study` to extend for AC2.
- [Source: persistence/tests/corpus_gate.rs (`frozen_corpus_v1_opens_and_reads_back_the_canonical_study`, the byte-pin)] — keep green; the migration does not touch studies; **no `v2.db` corpus**.
- [Source: contract/src/versioning.rs (`SCHEMA_VERSION = 1`) + lib.rs forward-compat doc] — the blob axis that stays 1.
- [Source: Epic-3 retrospective — epic-3-retro-2026-06-27.md (C3 schema-migration-first, C4 idempotency-guard, C1 on-display residuals)] — the carried action items this story addresses.
- [Source: app — the Études/Réglages nav + dashboard curate/empty-state (2.1/2.12/2.13)] — the UI surface pattern to mirror.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Opus 4.8, 1M context)

### Debug Log References

- `cargo test -p steadyinvest-persistence` → migration suite + 7 new watchlist integration tests green; fixed 3 existing tests for the new latest version (`fresh_database_migrates_to_latest`, `rerun…`, `readonly_newer` `supported: 2`); repointed the harness scaffold to a fake **v3**.
- `cargo test -p steadyinvest-app` → app **190** tests (added watchlist app-rail tests + a `SeqIdGen` test double — `FixedIdGen` returns one id, which collided on multi-add).
- Posture: message inventory `41 → 42` (`MSG_WATCH_NO_STUDY`); `@tr` floor `227 → 236` (the fleshed-out watchlist screen, net +9 literals).
- `cargo fmt --all --check` ✓ · `cargo clippy --workspace --all-targets -- -D warnings` ✓ (0) · `cargo test --workspace` ✓ · `cargo deny check` ✓ · `timeout 8 cargo run` → exit 124.

### Completion Notes List

- **Tasks 1–5 complete; Task 6 (manual on-display GO/NO-GO) is the RESIDUAL** — needs Guy's desktop to walk the watchlist (add/reorder/link/unlink/remove + reopen + delete-a-linked-study). Joins the 3.3–3.6 on-display batch.
- **The retro's C3 finding, confirmed and right-sized:** Story 1.10 had already created the **whole Epic-4/6 table set** in the v1 DDL (`watchlist_items` included), so "the first migration" is a one-line `ALTER TABLE … ADD COLUMN study_id`. The migration *harness* (own-transaction-per-step, `PRAGMA user_version`, append-only `REGISTRY`, newer-file-refused) already existed + was tested with a placeholder `fake_v2` — this story turns that placeholder into the real step 2.
- **Two version axes, only one moved:** `PRAGMA user_version` **1 → 2** (SQL schema); `contract::SCHEMA_VERSION` stays `1` (the watchlist is a normalized table, not a serde blob) — so **`contract`/`core` byte-untouched**, method fingerprint / golden / determinism / the frozen `v1.db` blob shape all clean, and **no new `v2.db` corpus** (the forward-migration tests prove NFR-R3, not a new frozen file).
- **No-orphan on study delete (AC2):** `delete_study` clears the watchlist soft link in the same transaction (`study_id = NULL`), proven by `deleting_a_linked_study_clears_the_watchlist_link`.
- **C4 idempotency lesson applied:** every watch mutation guards a no-op (`WHERE … IS NOT ?` + a changed-count check) so an identical update / reorder / absent delete writes **no** journal revision (the Synology-sync concern).
- **Scope discipline:** no product decisions needed (a watchlist is tickers + optional same-ticker study links — currency-agnostic); the reference-currency + EODHD-plan calls stay for 4.3/4.4. The study-link uses **auto-match by ticker** (the common case); an explicit picker is a documented later refinement.
- **No new dependency** (`rusqlite`/`uuid` already present); `Cargo.lock` unchanged.

### File List

**New**
- `persistence/src/watchlist.rs` — `WatchItem` + the CRUD (`add`/`list`/`update`/`delete`/`set_positions`) on the `logical_version` rail with no-op guards; `repack_positions`.
- `persistence/tests/watchlist.rs` — 7 integration tests.

**Modified — persistence**
- `persistence/src/schema.rs` — `migrate_to_v2` (ADD COLUMN study_id) + `v2_adds_the_watchlist_study_id_column` test; `DDL_V1` untouched.
- `persistence/src/migrations.rs` — `REGISTRY` += step 2; updated version assertions; scaffold repointed to a fake v3.
- `persistence/src/studies.rs` — `delete_study` nulls the watchlist link.
- `persistence/src/lib.rs` — `mod watchlist` + `pub use WatchItem`.
- `persistence/tests/readonly_newer.rs` — `supported: 1 → 2`.

**Modified — app**
- `app/src/state.rs` — the 5 watchlist `JournalState` rails + `watch_error` + `MSG_WATCH_NO_STUDY` + inventory; app-rail tests + the `watch_state` helper.
- `app/src/main.rs` — `refresh_watchlist` + `apply_watch_result` + `link_watch_to_same_ticker_study` + the 5 `Watchlist` callbacks + the startup refresh.
- `app/src/posture.rs` — message floor `41 → 42`, `@tr` floor `227 → 236`.
- `app/src/clock.rs` — `SeqIdGen` test double.
- `app/ui/state.slint` — `WatchRow` struct + `Watchlist` global; `app/ui/app.slint` — import/export them; `app/ui/screens/watchlist.slint` — the fleshed-out screen.

### Change Log

- 2026-06-27 — Story 4.1 implemented (watchlist management, FR34) — opens Epic 4 and carries the project's **first schema migration** (`PRAGMA user_version` 1 → 2: `watchlist_items.study_id`). Persistence `WatchItem` CRUD (add/list/reorder/update/delete, contiguous positions, no-op-guarded), `delete_study` clears the dangling watchlist link, the app watchlist rails + a fleshed-out Watchlist screen (add/reorder/link/unlink/remove). `contract`/`core` untouched (`SCHEMA_VERSION` stays 1 — normalized table, not a blob); no new `v2.db` corpus (forward-migration tests prove NFR-R3); method fingerprint / golden / studies byte-pin clean; no new dep. app 190 tests + 7 persistence watchlist tests; all four gates green. Status → review. Task 6 (manual on-display GO/NO-GO) pending Guy's display.
- 2026-06-27 — 3-layer adversarial code review (Blind + Edge + Acceptance). Acceptance Auditor: **ACCEPT** (AC1–AC6 PASS; core/contract untouched, SCHEMA_VERSION 1, no v2.db corpus, floors + banned-verbs clean). Blind: no CRITICAL/HIGH. **3 patches applied** (1 MEDIUM + 2 LOW), rest dismissed. app 190 → 191 tests; gates re-green. Status → done.

## Review Findings (3-layer adversarial code review, 2026-06-27)

Layers: Blind (diff-only) + Edge (diff + project) + Acceptance (diff + spec). **Acceptance: ACCEPT** (6/6). **Blind: no real bug** — verified the SQL (positions, `IS NOT ?` null-safe no-op guards, the same-tx re-pack + version bumps), the migration (additive, harness-gated), the move-swap (no UNIQUE on `position` → no transient collision), and the RefCell borrows (no overlap). 3 patch · 0 defer · several dismissed.

### Patches (applied)

- [x] [Review][Patch] **MEDIUM — auto-link defeated by ticker case mismatch** [app/src/main.rs / state.rs] — `link_watch_to_same_ticker_study` compared tickers with `==`, so a watched `"nesn"` failed to find the `"NESN"` study (tickers are stored as entered, not upper-cased) and wrongly raised `MSG_WATCH_NO_STUDY`. **Fix:** extracted `JournalState::study_id_for_ticker` (case-insensitive `eq_ignore_ascii_case`, most-recent) + a unit test.
- [x] [Review][Patch] **LOW — the watchlist surface went stale after a cross-screen study delete** [app/src/main.rs] — `on_confirm_study_action` (delete) cleared the watchlist link in the DB but only called `refresh_studies`, so a linked row still showed its (now-cleared) study until the next watchlist write. **Fix:** also `refresh_watchlist` after a study action.
- [x] [Review][Patch] **LOW — `WatchRow` inferred "linked" from the resolved label, not the cell's `study_id`** [app/ui + main.rs] — the Lier/Délier toggle keyed off the study-ticker *string*, so a link whose study did not resolve rendered as "unlinked". **Fix:** `WatchRow` carries an authoritative `linked: bool` (`study_id.is_some()`); the screen drives the toggle off it; `study-link` is now display-only.

### Dismissed (by-design / documented limitation)

- **Edge LOW — an archived study is auto-linkable and renders as a live link.** Acceptable: a watchlist→study link is a different relationship than dashboard visibility; archiving hides the study from the list but a deliberate watch-link to it is valid (the user can unlink). A product nuance, not a correctness bug.
- **Blind LOW — `move_watch_item` returns `Ok(())` rather than `MSG_NO_JOURNAL` with no journal.** A neutral no-op (nothing to move); harmless, not a wrong signal.
- **Edge info — two same-ticker watch rows auto-link to the same (most-recent) study; `.rev()` tiebreaks on id for same-instant studies.** Documented limitation: an explicit study picker (vs auto-match) is a deferred refinement; the row-id keys the link unambiguously.
