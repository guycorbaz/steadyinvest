# Story 3.3: Manual refresh with recompute & freshness

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want a single manual refresh that re-fetches provider data and recomputes,
so that keeping a study current is one deliberate, honest action — and I can see at a glance which cells are fresh, which are stale, and why the verdict moved.

## Acceptance Criteria

(From epics.md §Story 3.3, lines 777–789. FR21, FR29; also FR18 timestamp surfacing, FR65 offline-first. Scope boundary vs. Story 3.4 resolved in Dev Notes "Scope decision".)

1. **AC1 — One user-initiated refresh re-fetches through `normalize`.** Given an **open study with a populated grid**, when Guy triggers the **manual refresh**, then provider data is re-fetched off the UI thread (the Story-3.1/3.2 worker path), mapped, and passed **through `core::normalize`** exactly as the first fetch — no second code path, no second normalize per frame (FR21). The refresh is **idempotent**: re-running it on unchanged provider data produces no spurious change (no phantom undo step, no review demotion, no timestamp churn on untouched cells).

2. **AC2 — Refresh updates provider/derived cells & gaps; never overwrites a manual cell.** A refresh **updates** cells whose `source` is `Provider` or `Derived`, and **fills** empty/`to-fill` cells (the Story-3.1 gap-fill behaviour, now generalised to also re-stamp already-provider cells with the new value + timestamp). A cell whose `source` is **`Manual` is left untouched** by this story — the manual value stands and the divergent fetched value is **not** stored alongside it yet (that non-destructive dual-value reconciliation is **Story 3.4**, FR22). Manual data is therefore safe **by construction**: the refresh rail simply skips `Source::Manual` cells.

3. **AC3 — A divergent value on a `✓` provider cell auto-tags `✓→?` in the same coherence frame.** When the refresh writes a **provider** cell whose new value **diverges** from a previously **`✓ validated`** value, the cell auto-demotes to **`? to-review`** (FR20), and **in the same `build_frame`** the dependent verdict degrades if that cell is load-bearing (the Epic-1 invariant 2b, now driven by a real refresh). A **non-divergent** re-fetch keeps the human `✓` (value equality, not byte equality). This rides the existing `contract::Cell::edited` primitive proven by `seam_check.rs` SEAM 2/3 — **no new demotion logic**, only the new app-side provider rail that threads a `Source::Provider` provenance into it.

4. **AC4 — Each refreshed cell carries freshness (current/stale) + a queryable timestamp.** Every cell the refresh writes is stamped **`Freshness::Current`** with the refresh **timestamp** (`provenance.timestamp`, RFC3339). The per-cell **freshness state** (`current`/`stale`) and the **timestamp** are surfaced to the UI **on demand** (revealed alongside the existing `source` reveal — the attention-hierarchy rule: provenance is a discreet murmur, never equal signposting), so Guy can query "when was this fetched?" without the timestamp shouting on the grid (FR18). The stale `◦` murmur + dimming already render (SEAM 1); this story adds the **timestamp** to the same revealed-on-demand provenance channel. (No cell is *set* stale by this story — stale-on-failure is Story 3.5; the seam is already proven.)

5. **AC5 — The recompute distinguishes its cause (price / input / FX).** After a refresh, the app computes a **`RefreshCause`** from the diff of what actually changed: **price** (any of `high_price` / `low_price` / `current_price` moved), **input** (any fundamental — `sales` / `eps` / `pre_tax_profit` / `book_value_per_share` / `dividend_per_share` — moved), or **none** (nothing changed). The cause is **reported via the neutral notice** ("Recalculé : prix actualisés", "Recalculé : données fondamentales actualisées", both, or "Aucun changement"). **FX is a declared cause slot but inert in this story** — FX acquisition is FR28 **[P2]**, not built; the `RefreshCause` enum reserves an `Fx` variant (or documents its omission) so FR29's "distinguishing the cause" is structurally honoured without speculative FX machinery. The recompute itself stays the single deterministic `engine::build_frame` (no per-cause branching in the math — the cause is *classification of the diff*, not a different calculation).

6. **AC6 — The refresh is the ONLY online action and degrades the UI-thread guarantee (FR65 preserved).** The refresh is user-initiated (a button), runs entirely off the UI thread, returns via `invoke_from_event_loop`, and is the **only** network call the app makes — no background polling, no on-open auto-fetch. The `Studies.fetching` flag disables the button in-flight; a worker-gone or fetch error surfaces the existing neutral `MSG_PROVIDER_FAILED` cause-named notice and leaves the last-known grid **intact** (full failure-cause taxonomy + stale-flagging is Story 3.5; this story must not regress the 3.1/3.2 failure handling).

## Tasks / Subtasks

- [ ] **Task 1 — Provider-refresh rail in `state.rs` (AC1, AC2, AC3)**
  - [ ] Add `JournalState::apply_provider_refresh(study_id, fetched: &FetchedFinancials) -> Result<RefreshReport, String>` alongside the existing `apply_provider_fetch` (state.rs:643). It runs on the **same `mutate_study` rail** (so undo/version/journal-presence guards are reused for free).
  - [ ] Per matching year, per load-bearing + optional field: branch on the **current cell's `source`**:
    - `Source::Manual` → **skip** (AC2; manual wins, dual-value preserve = Story 3.4).
    - empty / `Coverage::ToFill` → **fill** (existing 3.1 gap-fill).
    - `Source::Provider` / `Source::Derived` with a value → **re-stamp** via `cell.edited(new_value, provider_provenance(digest))`. `Cell::edited` (contract/cell.rs:76) auto-demotes `✓→?` **iff** the value diverges and keeps `✓` on equality (AC3) — **do not** re-implement this; call the primitive.
  - [ ] **Idempotency (AC1):** rely on `mutate_study`'s `before != study` guard (state.rs:1041) — an unchanged refresh records no undo step. `Cell::edited` on an equal value must yield an equal cell **including timestamp** for true idempotency — see Open Question on timestamp churn; resolve so an unchanged re-fetch does **not** bump the timestamp (else `before != study` always trips). Preferred: only re-stamp (new timestamp) when the value actually changed; an equal re-fetch is a no-op for that cell.
  - [x] Return a `RefreshReport { updated: usize, filled: usize, cause: RefreshCause }` so `main.rs` can build the cause notice (AC5) without re-diffing in the UI layer. (Added `RefreshReport` + `merge`/`changed` in `state.rs`.)
  - [x] **Fetch-vs-refresh unification: FOLDED.** `apply_provider_fetch` (+ its `provider_year`/`present_cell_count`/`fill_year_gaps` helpers) removed; `apply_provider_refresh` subsumes them (a fresh study seeds empty `empty_provider_year` rows, then the SAME per-cell rail fills + classifies — one accounting path). The button + the 3 existing fetch tests now route through `apply_provider_refresh`.

- [x] **Task 2 — `RefreshCause` classification (AC5)**
  - [x] New `app/src/viewmodel/refresh.rs` (kept in `app`, not `core`): `RefreshCause { price, input, fx }` + `merge` (OR) + `classify_field`. `fx` is always `false` (FR28 P2 reserved slot).
  - [x] Classified from the real per-cell diff in `refresh_year` (a changed/filled cell ORs `classify_field(field)` into the report). Unit-tested (price-only, input-only, unknown→none, merge).
  - [x] **Single field-list source:** `refresh.rs` `PRICE_FIELDS`/`INPUT_FIELDS` are the only grouping; the rail passes the canonical field names — no parallel list. (`current_price` is a judgment input, never refreshed, so it is not a refresh cause.)

- [x] **Task 3 — Surface the per-cell timestamp on demand (AC4)**
  - [x] `GridCellState` (state.slint) gains `timestamp: string` — the as-of date, "" for a gap.
  - [x] Populated in `form::editable_cell` via the new `provenance_date` helper (the `YYYY-MM-DD` prefix of `cell.provenance.timestamp`, only for a present cell). `stale: bool` left as-is.
  - [x] Reveal wired exactly like `source`: `editable_cell.slint` sets `Studies.active-timestamp` on focus; `study_screen.slint` shows it via `@tr("Mis à jour le {}", …)` beside the source caption (revealed-on-demand, never an always-on column).

- [x] **Task 4 — Wire the refresh outcome + cause notice in `main.rs` (AC1, AC5, AC6)**
  - [x] The `WorkerOutcome::Fetch` arm now calls `apply_provider_refresh`, then `push_form`, and sets the notice via `state::refresh_notice(report)` (price/input/both/no-change).
  - [x] `set_fetching(false)` stays Fetch-arm-only (F5 preserved); the worker-gone P1 guard + `MSG_PROVIDER_FAILED` on `Err` untouched.
  - [x] `on_fetch_provider` (key-resolution / keyless / no-key) unchanged — only the outcome handling changed.

- [x] **Task 5 — Messages, posture floors & gates (AC4, AC5, FR13)**
  - [x] Added `MSG_REFRESH_{NOCHANGE,PRICE,INPUT,BOTH}` + registered in `USER_FACING_MESSAGES`; retired the now-unused `MSG_PROVIDER_DONE` (folded). `refresh_notice(report)` maps outcome → message. All banned-verb-clean.
  - [x] Posture floors bumped: message inventory `34 → 37` (−1 retired, +4 new); `@tr` floor `223 → 224` (+1 "Mis à jour le {}").
  - [x] All four gates green `--locked`: fmt ✓, `clippy -- -D warnings` ✓, `cargo test --workspace` ✓ (app 168), `cargo deny check` ✓. Method fingerprint / determinism / golden / corpus re-diff clean (core/contract/persistence untouched).

- [x] **Task 6 — Tests (AC1–AC5)**
  - [x] Refresh-rail unit tests in `state.rs`: re-fetch updates a changed provider cell (value + new digest); a manual cell is left untouched by a **divergent** refresh; a divergent provider `✓` demotes to `?` while an equal re-fetch keeps `✓`; an identical refresh records **no undo step** (`undo_depth` accessor); a fresh study still builds the full grid.
  - [x] `RefreshCause` classifier unit tests in `refresh.rs` (price/input/unknown/merge).
  - [x] Verdict-degradation integration test driven by a **real** `apply_provider_refresh`: an all-validated provider study reads `Full`; a divergent refresh demotes a load-bearing cell → `Provisional` in the same frame.
  - [x] All `seam_check.rs` tests still green (full workspace suite passes).

- [ ] **Task 7 — Manual on-display GO/NO-GO (AC3, AC4, AC5) — Guy on display** *(RESIDUAL — needs Guy's desktop; headless cannot render. Same pattern as 3.1/3.2.)*
  - [ ] On Guy's desktop (real EODHD key in the keychain, or the `demo`/AAPL.US harness): open a study, refresh, and confirm perceptually (the Spike-D residual, now realisable):
    1. a present-but-stale cell shows the `◦` murmur at ~60% **without** out-shouting a `▦ to-fill` gap;
    2. a provider-divergent `✓` cell visibly returns to `?` (validated ink clears);
    3. the verdict badge drops to the **provisional hatched** state with the stale/changed input named;
    4. the per-cell **timestamp** is revealed on focus (not shouting on the grid), and the **cause notice** ("Recalculé : prix actualisés" etc.) reads honestly.
  - [ ] Note: Guy's free-tier EODHD plan returns **403** on `/fundamentals` (Story 3.2 finding) — fundamentals refresh needs a paid plan. Test the price path with `/eod` (free) and/or the `demo` key on AAPL.US; the fundamentals path is exercised via fixtures/`FakeProvider` headless. Record the result; a paid-plan product decision is still open (deferred from 3.2).

## Dev Notes

### Scope decision (the 3.3 / 3.4 / 3.5 boundary — READ FIRST)

Epic 3 splits the "refresh" concept across three stories; staying inside 3.3's lane is the single most important thing here:

- **3.3 (this story) = the refresh ACTION + freshness/timestamp display + cause-of-recompute classification.** It builds the **provider-refresh rail** (the spike's one identified gap: `mutate_cell` hardcodes `manual_provenance()`, so no `Source::Provider` write path exists). The rail updates provider/derived/gap cells and, on a divergent `✓` provider cell, auto-demotes `✓→?` via the existing `Cell::edited` primitive (FR20), degrading the verdict in the same frame.
- **3.4 (NEXT) = non-destructive reconciliation (FR22).** The **manual-wins + preserve-the-fetched-value-alongside** policy. This needs a **second value slot** on the cell (a "fetched candidate" beside the manual value) that does **not** exist in `contract::Cell` today — a contract/schema change. **3.3 deliberately does NOT touch a manual cell** (it skips `Source::Manual`), so 3.3 never overwrites manual data and never has to half-build the dual-value store. Do not add a fetched-alongside field in 3.3.
- **3.5 (LATER) = graceful provider failure (FR23/FR24).** Setting cells **stale** on a failed refresh + the full failure-cause banner (network / quota / invalid-key) + retained last-known values. 3.3 keeps the existing 3.1/3.2 single-notice failure path and must not regress it, but does **not** set any cell stale itself. The stale **display** seam (`◦` murmur + dimming) already renders (SEAM 1) and 3.3 surfaces the timestamp on the same channel — the *production trigger* of stale is 3.5.

**Why this boundary is safe:** `seam_check.rs` (Spike D, GO) already proved all three rails fire through the real contract/engine/form code. 3.3 only adds the **app-side provider write path** that feeds them — the heavy primitives (demotion, verdict degrade, stale murmur) are built and tested. This is an **`app`-crate-only** story (plus posture-floor bumps): `core`/`contract`/`persistence` untouched, no schema/DDL change, no `method_version` change.

### What this story changes vs. preserves (files marked UPDATE — read before editing)

- **`app/src/state.rs` (UPDATE)** — add `apply_provider_refresh` beside `apply_provider_fetch` (line 643) on the `mutate_study` rail (line 1041). Reuse `provider_provenance` (line 627), `provider_cell`/`provider_year`/`fill_year_gaps` (lines 1185–1265). **Preserve:** the gap-fill semantics for empty/`to-fill` cells, the undo `record(before)` only-on-real-change guard, and all journal/read-only guards. **The new behaviour vs. `apply_provider_fetch`:** also re-stamp existing `Source::Provider`/`Derived` cells via `Cell::edited` (which `apply_provider_fetch` never did — it only filled gaps). **Critical:** branch on the **current cell's `source`** to skip `Source::Manual` (AC2).
- **`app/src/main.rs` (UPDATE)** — the outcome handler's `WorkerOutcome::Fetch` arm (around the `set_outcome_handler`/`on_fetch_provider` block, lines ~500–544) calls `apply_provider_refresh` and sets the cause notice. **Preserve:** the 3.2 key-resolution (`resolve_provider_key`, keyless/no-key), the `set_fetching(false)`-only-on-Fetch-arm fix (F5), the worker-gone guard, and `MSG_PROVIDER_FAILED` on error.
- **`app/src/viewmodel/form.rs` (UPDATE)** — `editable_cell` (line 60) populates the new `GridCellState.timestamp` from `cell.provenance.timestamp`. **Preserve:** the existing `stale`/`source`/`review`/`warning` channels and `mgmt_rows` (line 154) shape.
- **`app/src/viewmodel/engine.rs` (UPDATE)** — add `RefreshCause` + classifier (or a new `viewmodel/refresh.rs`). **Preserve:** `build_frame` (line 218) as the single deterministic recompute — the cause is *diff classification*, never a second/branched calculation. Reuse the load-bearing field constants; don't fork a parallel list.
- **`app/ui/state.slint` (UPDATE)** — add `timestamp` to `GridCellState` (line 41). **Preserve:** the struct's existing fields and the "provenance revealed on demand" convention.
- **`app/ui/screens/study_screen.slint` (UPDATE)** — the "Récupérer (fournisseur)" button (lines 270–274). Optionally relabel to "Actualiser"/refresh wording (still `@tr()`), and reveal the per-cell timestamp on focus where `source` is already revealed. **Preserve:** the `Studies.fetching`/`demo-active` enable-guard.
- **`app/src/state.rs` messages + `app/src/posture.rs` (UPDATE)** — new `MSG_REFRESH_*` + floor bumps to the exact measured counts.

### NEW files (optional)

- **`app/src/viewmodel/refresh.rs`** — if the `RefreshCause` type + classifier reads cleaner in its own module than inside `engine.rs`. Either is fine; keep it in `app`, not `core` (it classifies an app-side diff, not a method rule).

### Architecture & constraints

- **Single normalize / single frame (the Story-2.7 invariant, architecture lines 82–85, 404–417):** the refresh re-fetches → one `core::normalize` → one `engine::build_frame`. The UI never shows a fresh number beside an input it does not descend from. Do **not** normalize twice or compute a P/E inside a Slint callback (the Cardinal Rule, architecture line 582).
- **The provider-provenance gap (Spike-D finding, `docs/spikes/spike-d-stale-reconcile.md`):** `mutate_cell` hardcodes `manual_provenance()`/`Source::Manual`; this story adds the missing `Source::Provider` write path. The contract primitive `Cell::edited` already accepts a provider provenance (SEAM 2 proves the `✓→?` demotion + `Source::Provider` flip + `Freshness::Current`).
- **Recompute distinguishing cause (FR29, architecture lines 404–407, 518):** "an input change invalidates the dependent verdict (marked stale) rather than silently overwriting." 3.3 realises the **cause distinction** (price/input/FX) as a classification of the refresh diff; the invalidation-on-divergence is the `✓→?` demotion + verdict degrade already wired.
- **FX is P2 (FR28, FR26/FR27 all P2):** no FX acquisition, no fallback chain, no quota batching in this story. `RefreshCause` reserves the FX slot for structural completeness only.
- **Offline-first (FR65, NFR-S2/S3, PRD line 585):** "the only online action is a user-initiated manual price/data refresh." No background fetch, no on-open auto-fetch — keep the single user-initiated button.
- **Attention hierarchy (PRD line 46, architecture trust_markers):** missing = the only state that shouts; stale = a discreet uniform murmur; auto-vs-manual (and now the timestamp) = revealed on demand. The new timestamp must **not** become an always-on column — reveal it on focus alongside `source`.
- **No calc change:** method fingerprint, determinism hash, golden gate (11 fixtures), frozen `v1.db` corpus must re-diff clean. `app`-crate-only story + posture floors + workspace lock unchanged (no new dep expected — reqwest/keyring/tokio already in the tree from 3.1/3.2).
- **Issue #21 (real provenance digests / logical_version):** `provider_provenance` currently hardcodes `logical_version: 1`. If trivial, thread the journal's real `logical_version` here; otherwise leave it consistent with `apply_provider_fetch` and keep #21 open — do **not** expand scope to chase it.

### Testing standards

- Headless Rust unit/integration tests are the norm (Slint-native, no-web app — the QA e2e/Playwright step is N/A, as every Epic 2/3 story recorded). Use `FakeProvider` (made `pub` in 3.1) + fixtures for the fetch; no network in CI.
- **Reuse, don't re-prove, the seams:** `seam_check.rs` already proves demotion/verdict-degrade/stale-murmur through the real rails. Add a **real-refresh** integration test that drives the verdict degrade through `apply_provider_refresh` (not a hand-set `Freshness`), and keep `seam_check.rs` green.
- Idempotency is a first-class test (AC1): a second identical refresh records **no** undo step and changes **no** timestamp.
- All four gates `--locked`; pinned rustfmt 1.9.0 (`cargo fmt --all --check` stays green — issue #36 realignment is on main).
- UI story → on-display visual verification is part of DoD (Task 7), per the Epic-2 convention (B8) and the Spike-D perceptual residual.

### Open questions for dev (resolve during implementation, don't block)

- **Timestamp churn vs. idempotency (the one real design call):** if every re-fetch re-stamps `provenance.timestamp = clock.now()`, then `before != study` always trips → a phantom undo step + a "changed" study on an unchanged refresh, breaking AC1. **Preferred resolution:** only re-stamp (value + timestamp) a cell whose **value actually changed**; an equal re-fetch is a no-op for that cell (so an all-equal refresh is a true no-op). Confirm this still lets Guy see "last refreshed at" — if a global "last refresh" timestamp is wanted on a no-change refresh, surface it as a study-level/notice fact, not a per-cell provenance bump.
- **Fetch/refresh unification:** fold `apply_provider_fetch` into `apply_provider_refresh` (one path), or keep both? Leaning **fold** (one button, one mental model) — but verify no other caller depends on pure gap-fill before deleting it.
- **Button label:** keep "Récupérer (fournisseur)" or relabel "Actualiser" now that it's a true refresh? Either is fine; if relabelled, it's an `@tr()` literal change (re-measure the floor).
- **`RefreshCause` home:** `engine.rs` vs a new `viewmodel/refresh.rs`. Pick by readability; keep it in `app`.

### Project Structure Notes

- `app`-crate-only (+ posture floors). No new crate, no `ingestion` change (the fetch path + `FetchedFinancials` are unchanged from 3.1/3.2), no schema/DDL change, no `SCHEMA_VERSION` bump, no `method_version` change, no `deny.toml` change, no new workspace dependency.
- Matches the architecture source tree: `app/src/state.rs`, `app/src/viewmodel/{engine,form}.rs`, `app/ui/state.slint`, `app/ui/screens/study_screen.slint`.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.3 (lines 777–789)] — AC source; the 3.3/3.4/3.5 split (lines 791–817).
- [Source: _bmad-output/planning-artifacts/prd.md (FR21 line 701, FR22 lines 702–703, FR29 lines 716–717, FR18 line 695, FR20 lines 697–700, FR65 line 585)] — requirements + the manual-precedence/cause-distinction wording.
- [Source: _bmad-output/planning-artifacts/architecture.md (lines 82–85 transactional recompute, 404–417 state & recompute / verdict integrity, 518 invalidation, 582 Cardinal Rule, 654 reconcile.rs)] — recompute model, the 3.4 `reconcile.rs` that 3.3 stops short of.
- [Source: docs/spikes/spike-d-stale-reconcile.md] — the three seams, the GO verdict, and the "Story 3.3 must add a provider-provenance rail" finding + the on-display perceptual residual folded into Task 7.
- [Source: app/src/state.rs (apply_provider_fetch line 643, provider_provenance line 627, mutate_cell line 847, mutate_study line 1041, provider_cell/year/fill_year_gaps lines 1185–1265)] — the rails to extend.
- [Source: contract/src/cell.rs (Freshness line 20, Source line 11, Cell line 46, Cell::edited line 76); contract/src/provenance.rs (Provenance line 20, Timestamp line 14)] — the primitives, unchanged.
- [Source: app/src/viewmodel/engine.rs (build_frame line 218, cell_to_gate_state line 141, to_input_gates line 160)] — the single recompute + gate mapping that degrades on stale.
- [Source: app/src/viewmodel/form.rs (editable_cell line 60, mgmt_rows line 154); app/ui/state.slint (GridCellState line 41)] — the timestamp-surfacing point.
- [Source: app/src/fetch.rs (WorkerJob/WorkerOutcome lines 40–56, worker bridge lines 61–124); app/src/main.rs (on_fetch_provider lines 500–544)] — the worker + outcome handler to extend.
- [Source: app/src/seam_check.rs] — the regression guard that must stay green; SEAM 2/3 the demotion/verdict rails 3.3 rides.
- [Source: Story 3.2 — 3-2-provider-configuration-api-keys-keychain.md] — the 403-free-plan finding (Task 7), the F5 set_fetching fix to preserve, the env/keychain key path 3.3 leaves intact.
- [Source: memory project-planning-progress — CHECKPOINT 2026-06-25] — 3.2 done; RESUME = 3.3 manual refresh + recompute + freshness.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Opus 4.8, 1M context)

### Debug Log References

- `cargo test -p steadyinvest-app --locked` → app **168** tests pass (158 → 168: +4 `refresh.rs` classifier, +6 `state.rs` refresh-rail/idempotency/cause/verdict-degrade).
- `cargo clippy --workspace --all-targets --locked -- -D warnings` → clean (exit 0). Removed `RefreshCause::any()` (dead in non-test build) to stay warning-free.
- `cargo fmt --all --check` → clean (after `cargo fmt --all` wrapped two long `build_snapshot().verdict()` chains).
- `cargo test --workspace --locked` → all crates green; method fingerprint / determinism / golden / frozen `v1.db` corpus tests pass unchanged (core/contract/persistence untouched).
- `cargo deny check` → ok (the `GPL-3.0 unmatched license allowance` line is a pre-existing, unrelated warning; exit 0).
- `timeout 8 cargo run -p steadyinvest-app --locked` → exit **124** (healthy event loop; the refresh path compiles + links into the live binary).

### Completion Notes List

- **Tasks 1–6 complete; Task 7 (manual on-display GO/NO-GO) is the RESIDUAL** — needs Guy's desktop (perceptual render of the `◦` murmur, the `✓→?` ink clear, the provisional-hatched badge, the revealed timestamp + cause notice). Headless cannot render. Same pattern as Stories 3.1/3.2.
- **Scope held exactly:** 3.3 = the refresh action + freshness/timestamp display + cause classification. The provider-refresh rail updates `Source::Provider`/`Derived` + fills gaps, and **skips a present `Source::Manual` cell** (manual wins by construction — the divergent dual-value preserve is Story 3.4, untouched here). No `contract`/`core`/`persistence` change, no schema/method change, **no new dependency** (Cargo.lock unchanged).
- **Idempotency (AC1 — the flagged design call):** a present provider cell is re-stamped via `Cell::edited` **only when the value actually changed**; an equal re-fetch is a no-op (no timestamp churn → `mutate_study`'s `before != study` guard records no phantom undo step). Proven by `idempotent_refresh_changes_nothing_and_records_no_undo_step` (new `#[cfg(test)] undo_depth()` accessor). A provider that returns no value for an existing cell keeps the last-known value (never blanks it — FR23 spirit).
- **Fetch/refresh FOLDED into one rail** (one button, one mental model): a fresh study seeds `empty_provider_year` rows, then the same `refresh_cell`/`refresh_optional`/`refresh_year` path fills + classifies — a single tally for `filled`/`updated`/`cause`. `apply_provider_fetch` and its 3 helpers were deleted.
- **Cause = diff classification, never a different calculation** (Cardinal Rule): the recompute stays the single `engine::build_frame`; `RefreshCause` is OR-merged from each changed/filled field via `refresh::classify_field`. `fx` is a reserved (always-false) slot for FR28 (P2). `current_price` is a judgment input, not refreshed → not a cause.
- **Verdict degradation now reachable from a real refresh** (not a hand-set `Freshness`): `a_divergent_refresh_degrades_a_full_verdict_to_provisional` builds an all-validated provider study (`Full`), then a divergent refresh demotes the load-bearing cell → `Provisional` in the same frame. Complements `seam_check.rs` SEAM 3 (all seam tests still green).
- **NFR-S1 / FR13:** new notices are fact-stating, banned-verb-clean; the timestamp value is data (not scanned); the api_token never enters any of these surfaces.

### File List

**New**
- `app/src/viewmodel/refresh.rs` — `RefreshCause` (price/input/fx) + `merge` + `classify_field` + `PRICE_FIELDS`/`INPUT_FIELDS`; unit tests.

**Modified**
- `app/src/viewmodel/mod.rs` — register `pub mod refresh;`.
- `app/src/state.rs` — `apply_provider_refresh` (replaces `apply_provider_fetch`); `RefreshReport` (+`merge`/`changed`); `empty_provider_year`/`refresh_cell`/`refresh_optional`/`refresh_year`/`CellRefresh` (replace `provider_year`/`present_cell_count`/`fill_year_gaps`); `MSG_REFRESH_*` consts + `refresh_notice` + inventory (retire `MSG_PROVIDER_DONE`); test-only `undo_depth()`; refreshed/extended tests (`fetched_custom` helper + 6 new 3.3 tests).
- `app/src/main.rs` — outcome handler calls `apply_provider_refresh` + `state::refresh_notice`.
- `app/src/viewmodel/form.rs` — `provenance_date` helper; `GridCellState.timestamp` populated.
- `app/src/posture.rs` — message-inventory floor `34 → 37`; `@tr` floor `223 → 224`.
- `app/ui/state.slint` — `GridCellState.timestamp`; `Studies.active-timestamp`.
- `app/ui/components/editable_cell.slint` — set `active-timestamp` on focus.
- `app/ui/screens/study_screen.slint` — "Mis à jour le {}" revealed caption beside the source.

### Change Log

- 2026-06-27 — Story 3.3 implemented (manual refresh + recompute + freshness, FR21/FR29). App-crate-only provider-refresh rail (updates provider/derived + fills gaps, skips manual cells; idempotent re-stamp only on real change), `RefreshCause` diff classification (price/input/FX-reserved), per-cell freshness timestamp revealed on demand, cause-named notices. Fetch folded into refresh (one button). app 158 → 168 tests; all four gates green `--locked`; core/contract/persistence + Cargo.lock untouched. Status → review. Task 7 (manual on-display GO/NO-GO) pending Guy's display.
- 2026-06-27 — 3-layer adversarial code review (Blind + Edge + Acceptance). Acceptance Auditor: **ACCEPT** (AC1–AC6 all PASS; scope boundaries 3.4/3.5/app-crate-only/posture-floors held). **1 HIGH patch applied** (N/A-accepted cell refilled by refresh — FR19 regression inherited from 3.1), **1 MEDIUM deferred → issue #46** (empty provider payload reads as "no change" — 3.5 territory), 6 dismissed (verified false-positives / by-design). app 168 → 169 tests; all gates re-green. Status → done.

## Review Findings (3-layer adversarial code review, 2026-06-27)

Layers: Blind Hunter (diff-only) + Edge Case Hunter (diff + project) + Acceptance Auditor (diff + spec). Auditor verdict: **ACCEPT** — AC1–AC6 implemented to intent; the only AC-vs-code divergence (`current_price` excluded from the price cause) is a correct narrowing (it is a judgment input, never refreshed). 1 patch · 1 defer · 6 dismissed.

### Patch (applied)

- [x] [Review][Patch] **HIGH — a `NotAvailableAccepted` cell was silently refilled by a refresh** [app/src/state.rs `refresh_cell`/`refresh_optional`] — an N/A-accepted gap carries `value: None` + `source: Manual` + `coverage: NotAvailableAccepted`, so it entered the `value.is_none()` gap-fill branch *before* the manual-skip check, flipping a deliberate FR19 "not available" decision back to a provider value. (Pre-existing behaviour inherited from Story 3.1's `fill_year_gaps`; 3.3 is its natural fix site.) **Fix:** skip a `Coverage::NotAvailableAccepted` cell up front in `refresh_cell`; `refresh_optional` now delegates every present slot to `refresh_cell` so the skip applies uniformly. Regression test `refresh_never_refills_a_not_available_accepted_cell` added (load-bearing + optional).

### Deferred (→ GitHub issue, per the project's issue-tracking convention)

- [x] [Review][Defer] **MEDIUM — an empty (0-year) successful provider payload reads as "no change / up to date"** [app/src/state.rs `apply_provider_refresh`/`refresh_notice`] — a degenerate 200-with-no-years is indistinguishable from a genuine no-op. Honest-degradation gap, not a wrong data signal; the natural home is Story 3.5 (graceful provider failure / payload taxonomy). → **issue #46**.

### Dismissed (verified false-positive / by-design)

- **MEDIUM (edge) — a manually-cleared `ToFill` cell is refilled by a refresh.** By design: AC2 + Story-3.1 gap-fill fill empty/to-fill cells; a *deliberate* blank uses N/A-accepted (now protected by the patch above). A `ToFill` cell is semantically "to be filled".
- **LOW (blind+edge) — a first fetch says "Recalculé … actualisés" rather than "filled".** The notice wording is truthful and neutral; the 4-message cause-named design is intentional (`MSG_PROVIDER_DONE`'s fill-count is deliberately retired).
- **LOW (blind+edge) — `refresh_notice` `(false,false) → MSG_REFRESH_INPUT` fallback.** Documented as unreachable in this story (`fx` is hardwired `false`; the FX story will add its own message when FR28 lands).
- **LOW (blind) — `provenance_date` could leak a sentinel timestamp.** Verified safe: `view_provenance().timestamp` is the empty string, and skeleton cells have `value: None` → the present-cell guard returns `""`.
- **MEDIUM (blind) — idempotency depends on unseen `mutate_study`.** Verified: `mutate_study` records undo only on `before != study` (state.rs); the equal-re-fetch branch never mutates → no phantom step.
- **LOW (blind) — `Money` equality scale-sensitivity could break idempotency.** Verified: `rust_decimal`/`Money` `==` is value-based across scale (contract test `equal_value_edit_keeps_validated_even_across_scales`).
