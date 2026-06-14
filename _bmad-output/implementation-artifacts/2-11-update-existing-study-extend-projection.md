# Story 2.11: Update an existing study & extend its projection

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want to edit and extend a saved study,
so that forging conviction is iterative — keeping a study current is a quick annual ritual, not a rebuild from scratch.

## Acceptance Criteria

(From epics.md §Story 2.11, lines 684–696. BDD, verbatim intent. Scope-resolved 2026-06-14 — see Dev Notes "Scope decision".)

1. **Given** a saved study, **when** I reopen it and **correct a data value** (a §2/§3 cell) or **change a judgment input** (§4/§5), **then** the engine recomputes and the verdict + zones are invalidated/refreshed **in the same coherence frame** (one `snapshot_for` → `build_frame`, never a half-stale view), and the edit is persisted via `put_study` (FR16). This is the existing 2.4–2.6 rail — Story 2.11 **proves it end-to-end on a reopened study** (round-trip regression), it does not rebuild it.
2. **Given** a cell marked **Validated**, **when** I try to type into it, **then** the edit is **soft-locked** (refused with the neutral `MSG_SOFT_LOCKED` notice; I must remove the validation first) — the Story 2.5 rail, FR3. A column **paste** still auto-demotes Validated → ToReview (the recorded 2.4/2.5 interpretation); typing stays blocked.
3. **Given** a saved study whose forward projection currently re-bases off its latest year, **when** I **extend the projection** (the annual roll-forward: add the next actual fiscal year as a new year column), **then** a new `YearData` for `latest_year + 1` is appended (all cells `ToFill`/`None`), persisted via `put_study`, and the engine **re-projects the canonical 5-year horizon off the new latest usable year** so the est-high EPS, zones, upside/downside and verdict recompute in one coherence frame. The forward **horizon stays the canonical 5 years** — it is the *data window* that rolls forward, not the horizon length (see Scope decision).
4. **Given** Story 2.9 undo/redo, **when** I add a year (or edit a reopened study) and then **undo**, **then** the prior study state is restored — adding a year is "any edit" and rides the same `mutate_study` snapshot rail (one undo step per add; never destroys prior data). The change is reflected in the **in-session history** (undo stack + `logical_version` bump). Durable cross-reopen FR51 time-series is **out of scope** (deferred → GitHub issue; see Scope decision).
5. **Given** persistence, **then** adding a year and editing a reopened study are **atomic** (one `put_study` transaction each, `logical_version` bumped) and use the read-only / no-journal / save-failure guards (a neutral notice on refusal, never a silent `.ok()`). Adding a year onto a read-only journal is refused with the read-only notice.
6. **Given** the Definition of Done for a UI story, **then** the round-trip is unit-tested (reopen → edit a cell → recompute coherent + persisted; reopen → soft-lock refuses a typed edit on a Validated cell; add-year → `latest_year + 1` appended + engine re-bases + persisted; undo restores pre-add state), the binary launches and runs the event loop, and the in-GUI click-through is a documented partial (human/AT-SPI, as 2.1–2.10). 4 CI gates green `--locked`; **`core`/`contract`/`persistence`/`ingestion`/`report` + `Cargo.lock` + `deny.toml` + `rust-toolchain` re-diff unchanged** (no method/schema change — see Scope decision).

## Tasks / Subtasks

- [x] **Task 1 — Extend the data window: append the next actual year (app crate, `state.rs`)** (AC: 3, 4, 5)
  - [x] Add `JournalState::extend_history(study_id) -> Result<i32, String>` (returns the new year added, for the UI to scroll/focus it) on the **`mutate_study` rail** (read-only / no-journal / save-failure guards → re-read → mutate → `put_study` → undo `record` only on `before != study`). Append a `YearData { year: latest_year + 1, sales/eps/high_price/low_price: ToFill cells, optionals: None }`, mirroring `viewmodel::entry::materialize_year_window`'s cell skeleton (extract a `tofill_year(year, provenance)` helper so the seeding logic is shared, not duplicated).
  - [x] **Latest year:** `study.years.iter().map(|y| y.year).max()`. Append keeps the Vec in oldest→newest order (newest at the bottom, SSG order — the new year is the new max). On an **empty** study (no years materialized), `extend_history` re-materializes the canonical window from `created_at` (or appends `created_year` as the single newest year) rather than erroring — a degraded-but-safe path; pick the simplest correct behavior and document it.
  - [x] Provenance for the new cells = manual (`Source::Manual`-style, mirroring 2.4's `tofill_cell`), `Coverage::ToFill`. No value is computed (Cardinal Rule — adding a year is structure, not calculation).
  - [x] Headless tests: `extend_history` appends `latest_year + 1`; `get_study` after a fresh `JournalState` (reopen) shows the new year; the engine frame (`snapshot_for`/`build_frame`) re-bases off the new latest usable year once the new year's EPS is entered; **undo** after an add restores the pre-add `years`; adding onto a read-only journal returns `MSG_READ_ONLY_WRITE`.
- [x] **Task 2 — Wire the add-year callback (app crate, `main.rs` + `state.slint`)** (AC: 3, 5)
  - [x] Add a `Studies.extend-history()` callback (`state.slint`) and an `on_extend_history` handler (`main.rs`) mirroring `on_set_rationale`/`on_set_judgment`: resolve `current_study` → `extend_history(id)` → on `Ok` clear notice + re-read + `push_form` (refreshing undo flags); on `Err` set the notice. No format/parse needed (structural edit).
  - [x] `push_form` already re-renders the whole §2/§3 grid from `materialized_year_numbers(study)` — confirm the appended year shows as a new ToFill column with no extra plumbing (it should, since the grid is data-driven off `study.years`).
- [x] **Task 3 — Add-year affordance UI (Slint, `study_screen.slint`)** (AC: 3)
  - [x] A neutral **"+ année"** affordance at the newest edge of the §2/§3 year grids (or near the §1/§2 header) that fires `Studies.extend-history()`. Ink-only (NO new colour budget), constant geometry, reduced-motion-safe. Neutral `@tr` label (e.g. "Ajouter une année") — posture-scanned, no banned verb.
  - [x] Keep it visually subordinate to the data grid (an entry affordance, not a primary action) and consistent with the existing 2.4 entry gestures. Disabled/refused state surfaces via the existing notice rail (read-only journal) — do not invent a new disabled style unless one already exists.
- [x] **Task 4 — Prove the edit-reopened-study path (regression, AC: 1, 2)** (AC: 1, 2, 6)
  - [x] Headless tests on a **reopened** `JournalState` (fresh state on the same temp journal): edit a §3 cell → `build_frame` reflects it in the same frame + persisted; type into a Validated cell → `MSG_SOFT_LOCKED`, value unchanged; change a §4/§5 judgment input → verdict/zone recompute coherent. These exercise existing `edit_cell`/`set_judgment_field`/soft-lock through the reopen boundary (no new production code expected — if a gap surfaces, fix on the existing rail, do not fork it).
- [x] **Task 5 — Gates, posture floors, DoD, deferral issue** (AC: 4, 6)
  - [x] Bump `posture.rs` floors for the new `@tr` label(s) (`@tr` ≥ 162 currently; +1 per new literal; `.slint` count unchanged — no new component file unless you split one out).
  - [x] 4 CI gates green `--locked`; `core`/`contract`/`persistence`/`ingestion`/`report`/`Cargo.lock`/`deny.toml`/`rust-toolchain` re-diff **unchanged** (no method-fingerprint change — `FORECAST_HORIZON_YEARS` stays `5`; no schema bump — `YearData`/`Vec<YearData>` already exist). File List ⇄ git exact (issue #18).
  - [x] **File the deferral:** open a GitHub issue for durable FR51 cross-reopen study history (judgments time-series table is DDL-only today) — "history shows what changed and when" is satisfied in-session by undo (2.9) + `logical_version` in 2.11; durable per-save snapshots are a later story. Reference it in the Completion Notes.
  - [x] DoD: launch + run the event loop; in-GUI click-through = documented partial (human/AT-SPI). Don't mark `[x]` for a non-existent test.

## Dev Notes

### Scope decision (Guy, 2026-06-14) — READ FIRST

The epics AC says *"extend the projection horizon."* Resolved with Guy before authoring:

1. **"Extend the projection" = roll the 5-year data window FORWARD (the annual ritual), NOT a variable horizon.** Add the next actual fiscal year as a new `YearData` column; the engine re-bases its canonical **5-year forward** projection off the new latest usable year. The forward **horizon length stays 5** (`core::method::FORECAST_HORIZON_YEARS = 5`, `core/src/method/mod.rs:20`). A user-configurable horizon (5→N) was **explicitly rejected** for 2.11 because it would deviate from the canonical NAIC SSG and break the Epic-1 **method-fingerprint + determinism gate** (a pinned surface). ⇒ `core`/`contract` are NOT touched; this is an **app-crate story** (the `YearData` type and `Vec<YearData>` already exist — appending a year is in-app behaviour, no schema bump). This realizes the "full year-column add beyond the initial window" that `viewmodel/entry.rs:31` documented as deferred from Story 2.4.
2. **Durable FR51 cross-reopen history = DEFERRED.** "The study's history shows what changed and when" is satisfied **in-session** by the Story-2.9 undo/redo stack + the `logical_version` bump on each `put_study`. Writing per-save snapshots into the (DDL-only) `judgments` time-series table is a separate later story → file a GitHub issue (Task 5).
3. **Provider re-fetch / reconciliation = Epic 3, NOT here.** Journey 2b's "re-fetch + reconcile (validated flags reset on changed cells)" is provider-driven (FR-Epic-3). Story 2.11 is the **manual** edit + extend path only. The validated-flag-reset-on-change behaviour here is the existing **soft-lock** (you must unlock a Validated cell before typing) — not automatic reconciliation.

### The capability is small because the rails already exist (Stories 2.4–2.9)

Most of AC1/AC2 is **already built and tested** — Story 2.11 proves it survives the reopen boundary and adds exactly one new structural mutator (`extend_history`). Reuse, do not reinvent:

- **Edit a cell (soft-lock gated):** `app/src/state.rs:483 edit_cell` (gate at `:496`, `MSG_SOFT_LOCKED` at `:61`); `:617 set_not_available` (gate at `:629`). Column paste auto-demotes: `:710 paste_column` via `Cell::edited`.
- **Edit a judgment input:** `app/src/state.rs:767 set_judgment_field` → `:925 apply_judgment_field` (10 fields); `:779 set_forecast_low_option`.
- **The shared persist/undo backbone:** `:651 mutate_cell`, `:812 mutate_study`, `:852 mutate_judgment` — each re-reads, applies, `put_study`, and **records an undo snapshot only on `before != study`** (the Story-2.9 `record` guard). **`extend_history` rides `mutate_study`** (Story 2.10 added it) — append a year inside the `apply` closure and undo + atomicity come for free.
- **The single engine-call site:** `:897 snapshot_for` (normalize → `StudySnapshot::new` once) → `engine::build_frame` — guarantees AC1's "same coherence frame". `push_form` (`main.rs`) re-renders the whole form from one re-read.
- **Undo/redo:** `:351 undo`, `:357 redo` (in-memory, per-open-study, reset on reopen). Adding a year is one undo step.

### Year model & where the new column comes from

- `contract::Study.years: Vec<YearData>` (`contract/src/study.rs:90`); `YearData { year: i32, sales, eps, high_price, low_price, +3 optionals }` (`:16`). Append-only Vec → adding a year is `years.push(..)`, **no schema change**.
- Fresh study seeds `YEAR_WINDOW = 5` years (`year0-5 .. year0-1`, oldest→newest) via `viewmodel::entry::materialize_year_window` (`app/src/viewmodel/entry.rs:126`), materialized in memory and persisted on first edit. **Extract the per-year ToFill skeleton** (`tofill_cell` at `entry.rs`) into a shared `tofill_year(year, provenance)` so `extend_history` and `materialize_year_window` don't duplicate it.
- The newest year sits at the **bottom** of the SSG table (oldest→newest order) — the appended `latest_year + 1` becomes the new bottom row / rightmost §2 column. The §2 management grid is **transposed** (fields are rows, years are columns) — confirm the new column lands at the correct edge (`entry.rs:158 transposed`).

### How the horizon re-bases automatically (no core change)

The est-high EPS derivation reads `latest_usable(financials)` then `project(base, growth, FORECAST_HORIZON_YEARS)` (`core/src/ssg/growth.rs:95–98`). Once the newly-added year's EPS is entered, `latest_usable` returns the **new** latest year, so the 5-year projection re-bases forward by construction — zones (`core/src/ssg/risk_reward.rs:48 zone_bounds`, geometric thirds of `[forecast_low, forecast_high]`) and the verdict recompute with no engine edit. **Do not touch `core`.**

### Established conventions (carry forward)

- Cardinal Rule: **no calculation in the app layer** — adding a year is structure (a ToFill `YearData`), not arithmetic; all zoning/forecast/verdict math stays in `core`. No `.unwrap()`/`.expect()` in non-test code; no silent `.ok()`; time/IDs via the injected `Clock`/`IdGen` (the new year number derives from existing `study.years`, not wall-clock — deterministic).
- Money/values cross as formatted strings; structural callbacks (`extend-history()`) carry no payload. No `Decimal`/enum into `.slint`.
- Colour budget: the add-year affordance spends **NO** colour (ink only). Neutral microcopy (FR13) for its label.
- 4 CI gates `--locked`; `Cargo.lock`/`deny.toml` unchanged (no new dep); current app `#[test]` count **121** (you add to it).

### Recorded traps to avoid (2.4–2.10)

1. **No-op snapshot** (Story 2.9 P4) — `mutate_study` records undo only on `before != study`; an add-year always changes `years`, so it always records (one step). Fine.
2. **Keep-input-on-refusal** — the §2/§3 cells already follow the 2.4/2.6 re-seed-only-when-unfocused discipline; the add-year affordance is a button (no in-progress text to clobber).
3. **Soft-lock is typing-only** — paste auto-demotes, typing is blocked (`MSG_SOFT_LOCKED`). Don't "fix" this asymmetry; it's the recorded 2.4/2.5 interpretation.
4. **Posture: scan the label, not data** — register the new `@tr` affordance label; never scan cell/judgment values (user data).
5. **Pinned surfaces** — if you reach for `FORECAST_HORIZON_YEARS`, a `core` edit, or a `SCHEMA_VERSION` bump, **stop** — the scope decision forbids it. The story is app-crate only.
6. **File List ⇄ git exact** (issue #18); don't mark `[x]` for a missing test.

### Project Structure Notes

- All work in `steadyinvest-app`. **No `contract`/`core`/`persistence` change.** No new dependency.
- Slint/Rust naming: components `PascalCase`, `.slint` `snake_case`, props/callbacks `kebab-case` (`extend-history`).
- Files to touch: `app/src/state.rs` (new `extend_history` + `tofill_year` helper + tests), `app/src/viewmodel/entry.rs` (share the ToFill skeleton), `app/src/main.rs` (`on_extend_history` callback), `app/ui/state.slint` (`extend-history()` callback), `app/ui/screens/study_screen.slint` (the "+ année" affordance), `app/src/posture.rs` (floor bump).

### Tech stack (pinned)

- Rust workspace MSRV **1.96**; **Slint 1.16.1**; `rusqlite 0.40` (`bundled`). Linux-only dev/CI. 4 gates `--locked`.

### References

- [Source: epics.md#Story 2.11] (684–696: BDD AC). [Source: prd.md] FR3 (665: "update an existing study … and extend its projection"); FR16 (693: enter/override/correct any field by hand); Journey 2b (317–326: annual update ritual, "extends the projection, zones recompute, history shows what changed"); 5-year floor/horizon (201, 314, 445).
- [Source: contract/src/study.rs:16,90] `YearData` / `Study.years: Vec<YearData>`. [core/src/method/mod.rs:20] `FORECAST_HORIZON_YEARS = 5` (PINNED — do not vary). [core/src/ssg/growth.rs:95] `project(base, growth, FORECAST_HORIZON_YEARS)`; [core/src/ssg/risk_reward.rs:48,84] zone bounds + compute.
- [Source: app/src/state.rs] `edit_cell:483` (soft-lock `:496`), `set_judgment_field:767`, `mutate_study:812`, `undo:351`, `snapshot_for:897`. [app/src/viewmodel/entry.rs:34,126] `YEAR_WINDOW`, `materialize_year_window` (+ `:31` note: "Extending the forward projection horizon is Story 2.11").

## Open Questions (for Guy / dev — non-blocking, defaults chosen)

- **Q1 — Add one year per click, or a range?** **Default:** one year per click (`latest_year + 1`), repeatable. The annual ritual adds one year at a time; multi-year backfill is the manual-entry grid's job. Confirm.
- **Q2 — Affordance placement?** **Default:** a small "+ année" entry affordance at the newest edge of the §2/§3 grid, ink-only, visually subordinate. Confirm vs. a header-level action.
- **Q3 — Remove a year?** **Default:** OUT of scope for 2.11 (add-only; the 2.4 issue listed add/remove as a partial — remove stays deferred). Confirm.
- **Q4 — Empty-study extend?** **Default:** `extend_history` on a study with no materialized years re-materializes the canonical window (safe, never errors). Confirm the simplest correct behaviour.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Opus 4.8, 1M context)

### Debug Log References

- `cargo test -p steadyinvest-app` → 126 passed (121 → 126, +5). `cargo clippy --all-targets --locked` → clean.
- Binary launches + runs the event loop (`cargo run`, SIGTERM after 8 s, no panic).

### Completion Notes List

- **App-crate only** — `core`/`contract`/`persistence`/`ingestion`/`report`/`Cargo.lock`/`deny.toml`/`rust-toolchain` re-diff **empty** (verified). `FORECAST_HORIZON_YEARS` stays `5` (no method-fingerprint change); `YearData`/`Vec<YearData>` pre-existed (no schema bump). No new dependency.
- **Extend = roll the window forward** (scope decision): `JournalState::extend_history(study_id)` appends a `entry::tofill_year(latest_year + 1, …)` column on the Story-2.10 `mutate_study` rail (atomic, guarded, undoable). A never-edited study first materializes the canonical 5-year window (the in-memory view), then appends — so "+ année" grows it 5 → 6, never errors. Degraded all-empty case appends `year 0` (safe, no panic). The engine re-bases its 5-year projection off the new latest **usable** year by construction (`core` reads `latest_usable`) — no `core` edit.
- **Shared skeleton** — extracted `entry::tofill_year(year, provenance)` and refactored `materialize_year_window` to call it (no duplicated ToFill seeding).
- **Wiring** — `Studies.extend-history()` callback (`state.slint`) + `on_extend_history` handler (`main.rs`) mirror `on_set_rationale`: structural (no payload) → `extend_history` → on `Ok` clear notice + re-read + `push_form` (grid re-renders with the new column, undo flags refresh); on `Err` set notice. UI affordance = the ink-only, accessible `ActionButton` "Ajouter une année" at the bottom of §2 (no new colour, no new component file).
- **AC1/AC2 are existing rails** (2.4–2.6) — proven across the **reopen boundary** by `editing_and_the_soft_lock_hold_across_a_reopen` (validate → reopen → typed edit refused with `MSG_SOFT_LOCKED`; clear-✓ → edit on the reopened study persists). No new production code for the edit/soft-lock path.
- **History** — in-session via undo (2.9) + `logical_version`; durable cross-reopen FR51 time-series **deferred → GitHub issue #34**.
- 5 new headless tests: append+reopen, roll-forward ×2 (`2021..=2027`), undo restores pre-add window, read-only refused, edit+soft-lock across reopen. posture `@tr` floor 162 → 163 (new "Ajouter une année" label; user data never scanned).
- AC6 in-GUI click-through left as a documented partial (human/AT-SPI sandbox), as 2.1–2.10.

### File List

- `app/src/state.rs` — `extend_history` (on the `mutate_study` rail) + 5 headless tests
- `app/src/viewmodel/entry.rs` — extracted `tofill_year`; `materialize_year_window` reuses it
- `app/src/main.rs` — `on_extend_history` callback (mirrors `on_set_rationale`)
- `app/ui/state.slint` — `extend-history()` callback
- `app/ui/screens/study_screen.slint` — "Ajouter une année" `ActionButton` at the bottom of §2
- `app/src/posture.rs` — `@tr` floor 162 → 163 for the new affordance label

### Senior Developer Review (AI)

3-layer adversarial review (Blind Hunter + Edge Case Hunter + Acceptance Auditor), 2026-06-14.

- **Acceptance Auditor: PASS AC1–AC6.** Scope verified independently — app-crate only, `FORECAST_HORIZON_YEARS=5` intact, no `SCHEMA_VERSION` bump, core/contract/persistence/Cargo.lock/deny.toml re-diff empty, all 5 named tests present + green (126/126).
- **2 patches applied:**
  - [x] [MED] Read-only "Ajouter une année" button never visually disabled (its undo/redo siblings gate via `enabled:`) → added `enabled: !Studies.read-only` (consistent with the button rail; Rust already refused).
  - [x] [LOW] `latest + 1` unguarded against `i32::MAX` overflow → `saturating_add(1)` (defensive; value read from the journal blob).
- **1 deferred → GitHub issue #35** [LOW]: unbounded year growth (§2 widens horizontally) + empty appended year reserves a §1 chart x-slot. No data loss; empty years ignored by the engine (`latest_usable`).
- **Dismissed** (confirmed correct by all reviewers): next-year computation monotonic/collision-free, `FnOnce` move-capture sound, `main.rs` double-borrow safe (mutable temp dropped before shared borrows), undo guaranteed (append always changes `years`), engine re-base via `latest_usable`, both grids render 6+ years (map-by-year-number, no `.take(5)`), cursor `year_count` live.

### Change Log

- 2026-06-14 — Story 2.11 implemented: extend-projection (annual roll-forward) + edit-reopened-study regression. App-crate only; 126 tests; FR51 durable history deferred (#34). Status → review.
- 2026-06-14 — Code review (3-layer): Acceptance PASS AC1–6; 2 patches applied (read-only button gate, saturating_add), 1 LOW deferred (#35). 126 tests re-green, clippy clean.
