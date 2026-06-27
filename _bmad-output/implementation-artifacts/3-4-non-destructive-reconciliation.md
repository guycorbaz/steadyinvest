# Story 3.4: Non-destructive reconciliation

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want refreshes to never overwrite my manual work or my sign-offs,
so that reconciliation is safe and my validations stay meaningful.

## Acceptance Criteria

(From epics.md §Story 3.4, lines 791–804. FR22, NFR-R4; rides FR20 + the Epic-1 invariant 2b. Scope-resolved in Dev Notes "Scope decision".)

1. **AC1 — A manual value takes precedence; the divergent fetched value is preserved alongside (non-destructive).** Given a refresh that returns a value **differing** from an existing **`Source::Manual`** cell, when the refresh runs, then the **manual value stands** (never overwritten) and the **fetched value is preserved alongside it** — held on the cell as a *pending provider divergence* (value + provider provenance), surfaced to Guy, **never silently merged** into the value (FR22, NFR-R4). A fetched value that **equals** the manual value is not a divergence (no pending, no surfacing).

2. **AC2 — A divergent value on a `✓` validated cell auto-tags `✓→?` and degrades the verdict in the same frame.** When the differing cell was **`✓ validated`** (manual or provider), the refresh auto-tags it **`? to-review`**, and **in the same `build_frame`** the dependent verdict degrades if the cell is load-bearing (the Epic-1 invariant 2b, now driven by refresh). For a **manual** `✓` cell this composes with AC1: the value stays, the pending is preserved, **and** the `✓` demotes (the provider disagreeing is exactly the signal to re-check). A **provider** `✓` cell keeps the Story-3.3 behaviour (value updates in place + demotes) — 3.4 does not regress it.

3. **AC3 — A manual value and a provider value are never silently merged.** No code path blends a manual value and a fetched value into one number. The two are always **distinct and attributed**: the live value carries `Source::Manual`; the pending provider value carries `Source::Provider` provenance. The divergence is **resolved only by an explicit user act** (AC4) — never automatically.

4. **AC4 — Guy can resolve a pending divergence explicitly.** A cell carrying a pending provider divergence offers two neutral, fact-stating resolutions: **accept the provider value** (the cell becomes the provider value, `Source::Provider`, `Review::ToReview` so it is re-checked, pending cleared) or **keep the manual value** (pending cleared, manual value stands, `Review::ToReview` until Guy re-validates). Resolution is also **implicit**: a manual edit of the cell, or re-validating it (`set_review(Validated)`), clears the pending (the user has reconciled). No resolution ever happens without a user act (AC3).

5. **AC5 — The pending divergence persists (survives reopen) and never corrupts the journal.** The pending provider value is stored on the cell and **round-trips through the journal** (save → reopen) so an unresolved reconciliation is not lost (NFR-R4 "preserved"). The new cell field is **additive** (`#[serde(default)]`, forward- AND backward-compatible) — **no `SCHEMA_VERSION` bump, no migration, no `v2.db`** (the Story-2.2 pattern). An older journal (no pending field) loads with `pending = None`.

6. **AC6 — No method/calculation change; the data-shape change stays clean.** Adding the pending field is a **`contract` data-shape change, NOT a method change**: `core` calc, the method fingerprint, determinism hash, golden gate (11 fixtures), and the frozen `v1.db` corpus must re-diff **clean** (an unresolved pending never feeds the engine — only the live value does, exactly as today). The engine ignores `pending` entirely.

## Tasks / Subtasks

- [x] **Task 1 — Contract: the `pending` cell field + `Cell::reconcile` primitive (AC1, AC2, AC3, AC5, AC6)**
  - [x] `PendingProvider { value: Option<Money>, provenance: Provenance }` added in `contract/src/cell.rs`.
  - [x] `#[serde(default)] pub pending: Option<PendingProvider>` added to `Cell` (last field). Additive, **no `SCHEMA_VERSION` bump**. Round-trip + old-JSON-without-pending→None tests added.
  - [x] Every `Cell { … }` struct literal updated with `pending: None` — **13 full literals** across `contract/src/cell.rs`, `contract/tests/roundtrip.rs`, `core/tests/verdict_coherence.rs`, `persistence/tests/{corpus_gate,e2e_lifecycle,journal_roundtrip}.rs`, `app/src/{state,seam_check,viewmodel/{form,chart,engine,verify}}.rs`. The spread-syntax literals (`..base`/`..cell`/`..tofill_cell(...)`) needed no change. **No `core`/`persistence` SRC literals** (only their test helpers) — method/golden/corpus SRC untouched.
  - [x] `Cell::reconcile(&self, fetched, provenance) -> Cell` added (sibling of `edited`): live value/source/coverage/freshness untouched; divergence → `pending = Some(...)` + demote ✓→?; agreement → `pending = None` + keep ✓.
  - [x] `Cell::edited` now clears `pending` (a fresh edit resolves any divergence).
  - [x] Unit tests (divergent/equal/non-validated/edited-clears) + a `reconcile_rail_*` proptest (the live value is never touched; pends only on divergence).

- [x] **Task 2 — App: route the refresh Manual branch through `reconcile` (AC1, AC2)**
  - [x] `refresh_cell` Manual branch now reconciles a divergent fetch (was `Skipped`), returning `CellRefresh::Reconciled`; an agreeing/no-value fetch is `Unchanged` (idempotent: `reconcile(==) == cell` → no re-stamp, no phantom undo step). The `NotAvailableAccepted` skip stays first.
  - [x] `RefreshReport.reconciled: usize` added (+ `merge`/`changed`); a reconciled divergence feeds the cause. `refresh_optional` already delegates to `refresh_cell`.
  - [x] Tests: divergent manual cell preserved + pending + demote; agreement keeps ✓ + idempotent; verdict degrades via a real reconciling refresh.

- [x] **Task 3 — App: resolution actions (AC4)**
  - [x] `accept_provider_value` (takes the pending via `edited(pending.value, pending.provenance)` → Provider/ToReview/pending cleared) and `keep_manual_value` (clears pending only) on the `mutate_cell` rail; neutral no-op when no pending.
  - [x] `set_review(.., Validated)` clears any pending (re-validating reconciles). Tests for all three resolutions.

- [x] **Task 4 — UI: surface the divergence + the resolve affordance (AC1, AC4)**
  - [x] `GridCellState.pending` (state.slint) + `form::pending_value` populate it; `Studies.active-pending` set on focus (editable_cell.slint).
  - [x] `study_screen.slint` reveals `@tr("Fournisseur : {}", …)` beside the source/timestamp + two resolve controls shown only when a pending exists: `@tr("Accepter (fournisseur)")` / `@tr("Ignorer (fournisseur)")` (the keep-manual label was reworded from "Garder" — a banned verb — to "Ignorer", neutral) → `Studies.accept-provider`/`keep-manual` callbacks.
  - [x] `main.rs` wires `on_accept_provider`/`on_keep_manual` → the state rails → `push_form` (mirrors `on_set_review`).

- [x] **Task 5 — Persistence round-trip + gates (AC5, AC6)**
  - [x] `a_pending_divergence_survives_reopen` (close + reopen the journal, pending intact) + the contract old-JSON-without-pending→None test. AC6 guard test `the_engine_ignores_a_pending_divergence` (same verdict frame with/without pending).
  - [x] No new `MSG_*`; `@tr` floor bumped `224 → 227` (the three new reconciliation literals). The byte-pinned corpus JSON re-captured to the additive `"pending":null` shape (the Story-2.2 precedent; **no `SCHEMA_VERSION` bump, no new corpus file** — the frozen v1.db still reads back equal via `serde(default)`).
  - [x] All four gates green `--locked`: fmt ✓, `clippy -- -D warnings` ✓, `cargo test --workspace` ✓ (app 176, contract +6, full suite green), `cargo deny check` ✓. Method fingerprint / determinism / golden / v1.db corpus clean. `Cargo.lock`/`deny.toml` unchanged (no new dep).

- [ ] **Task 6 — Manual on-display GO/NO-GO (AC1, AC2, AC4) — Guy on display** *(RESIDUAL — needs Guy's desktop.)*
  - [ ] On Guy's desktop: enter a manual value, validate it (`✓`), then refresh with a divergent provider value. Confirm perceptually: the manual value **stays**, the `✓` visibly returns to `?`, the verdict badge degrades, and the pending provider value is revealed on focus (fact-stating, not shouting). Click **Accepter (fournisseur)** → the cell takes the provider value (`?`); re-do and click **Garder (manuel)** → the manual value stands, pending gone. Confirm a manual value is **never** silently replaced.
  - [ ] Test with `demo`/AAPL.US or fixtures (Guy's free EODHD plan 403s `/fundamentals`).

## Dev Notes

### Scope decision (the 3.4 lane — what 3.3 left for this story)

Story 3.3 deliberately **skipped** present `Source::Manual` cells on a refresh (manual safe by omission). 3.4 fills that lane: a refresh now **reconciles** a divergent manual cell instead of ignoring it.

- **3.4 = non-destructive reconciliation (FR22/NFR-R4).** The manual value wins and is never overwritten; the divergent provider value is **preserved alongside** (a new `Cell.pending` field) and surfaced; a divergent `✓` demotes to `?` + degrades the verdict (composing AC1 with the Epic-1 invariant 2b); the user resolves explicitly (accept-provider / keep-manual / edit / re-validate). **Never silently merged.**
- **The one architectural cost:** this is the **first Epic-3 story to change `contract`** (3.1–3.3 were app-crate-only). The change is a single **additive `#[serde(default)]` field** + a sibling primitive — NOT a method change. **No `SCHEMA_VERSION` bump** (additive field, contract forward-compat policy), **no migration, no `v2.db`**, **no method-fingerprint/golden/corpus impact** (the engine never reads `pending`).
- **Out of scope:** the full annual-update ritual (re-fetch a saved study, extend projection, history of what changed) = **Story 3.6**. 3.4 provides the reconciliation *mechanism* 3.6 orchestrates. Graceful provider failure / stale-on-failure = **Story 3.5** (untouched here).

### Where the reconcile primitive lives — `contract`, NOT `core`

The architecture source tree comments a `core/.../reconcile.rs` (line 654), but the **divergence→? demotion already lives in `contract::Cell::edited`** (the mutation rail, proven by `seam_check.rs` SEAM 2). `Cell::reconcile` is the same family (a snapshot cell-state transform), so it belongs **in `contract` beside `edited`**, not in `core`. Putting it in `core` would be wrong (core is calc/method) and would risk the method-fingerprint gate for zero benefit. **Document this as a deliberate deviation from the tree comment.** `core` stays untouched (AC6).

### What this story changes vs. preserves (files marked UPDATE — read before editing)

- **`contract/src/cell.rs` (UPDATE)** — add `PendingProvider` + `Cell.pending`; add `Cell::reconcile`; make `Cell::edited` clear `pending`. **Preserve:** `edited`'s existing semantics (divergence→? on the LIVE value, `Freshness::Current`, provenance verbatim) — `reconcile` is additive, it does not change `edited`'s value/review logic, only adds `pending: None` to its output. Update the 9 in-file `Cell {…}` literals (test helpers + proptest `cell()` generator — give it a `pending` strategy or `Just(None)`).
- **`app/src/state.rs` (UPDATE)** — `refresh_cell` Manual branch → `reconcile`; `CellRefresh::Reconciled` + `RefreshReport.reconciled`; `accept_provider_value` / `keep_manual_value` on the `mutate_cell`/`mutate_study` rails; `set_review(Validated)` clears pending. **Preserve:** the 3.3 idempotency (equal re-fetch = no-op, no phantom undo step), the `NotAvailableAccepted` skip (3.3 review fix, stays the first check), the manual-skip-on-agreement, and the cause classification. Update the 5 `Cell {…}` literals here (`provider_cell`, tests).
- **`app/src/viewmodel/{entry,form,chart,engine,verify}.rs` + `seam_check.rs` + `posture.rs` (UPDATE)** — add `pending: None` to their `Cell {…}` literals (mechanical). `form::editable_cell` ALSO populates the new `GridCellState.pending`. **engine.rs:** confirm the engine path reads only `cell.value` (never `pending`) — add the AC6 same-frame test.
- **`app/ui/state.slint` (UPDATE)** — `GridCellState.pending`; `Studies.active-pending` + the `accept-provider`/`keep-manual` callbacks. **Preserve:** the revealed-on-demand convention (3.2 source / 3.3 timestamp) — `pending` joins that channel.
- **`app/ui/components/editable_cell.slint` (UPDATE)** — set `Studies.active-pending` on focus (mirror `active-timestamp`).
- **`app/ui/screens/study_screen.slint` (UPDATE)** — the revealed caption naming the pending provider value + the two resolve controls (shown only when the focused cell has a pending). **Preserve:** the source/timestamp caption row.
- **`app/src/main.rs` (UPDATE)** — wire `on_accept_provider`/`on_keep_manual`. **Preserve:** the 3.3 refresh outcome handler + notice.

### Architecture & constraints

- **NFR-R4 (PRD line 862):** "Reconciliation **never destroys** a manual value or judgment; the provider value is preserved." Enforced by construction: `reconcile` never writes the live value; the divergent fetched value lands in `pending`; the engine ignores `pending`.
- **FR22 (PRD lines 702–703):** manual takes precedence, fetched preserved, non-destructive.
- **Invariant 2b (FR20, architecture lines 154–155):** "non-destructive reconciliation; divergence → auto-?." The demotion is the existing `Cell::edited` logic, reused by `reconcile`.
- **Contract forward-compat (`contract/src/lib.rs:12–14`, `versioning.rs`):** new *fields* with `#[serde(default)]` + no `deny_unknown_fields` are tolerated both directions → **no `SCHEMA_VERSION` bump**. (Story 2.2 added the four `Judgment` fields exactly this way — `study.rs:49–53`.) A new *enum variant* WOULD bump; `PendingProvider` is a struct, not a variant.
- **Engine ignores `pending` (AC6):** the verdict/zone math reads `cell.value` only (`engine::build_frame` → `to_raw_financials`/`to_input_gates`). A pending-bearing cell must yield a byte-identical frame to the same cell with `pending = None` — assert it. This keeps method-fingerprint/determinism/golden/corpus green despite the contract change.
- **Persistence (additive, no migration):** the journal stores `Study` as a JSON blob; an additive optional field needs no DDL/`schema.rs` change and no `SCHEMA_VERSION`/`v2.db` (mirrors Story 2.2 + 2.12). The pending round-trips for free.
- **Attention hierarchy (PRD line 46):** the pending divergence is a *revealed-on-demand* fact, not an always-on column; the `? to-review` marker is the shout. No colour spent on provenance.

### Previous-story intelligence (3.1 → 3.3)

- **3.3 built the refresh rail** (`apply_provider_refresh`, `refresh_cell`/`refresh_optional`/`refresh_year`, `RefreshReport`, `RefreshCause`). 3.4 extends `refresh_cell`'s Manual branch + `RefreshReport` (a `reconciled` count) — do not rewrite the rail.
- **3.3 review HIGH (issue carried):** the `NotAvailableAccepted` skip is the FIRST check in `refresh_cell` — keep it first; an N/A-accepted cell is never reconciled either (it is a deliberate "no value" decision).
- **3.3 idempotency trap:** re-stamp / mutate only on a real change, else `mutate_study`'s `before != study` records a phantom undo step. `reconcile` on an EQUAL value must yield a cell equal to the input (pending cleared only if it was set) — verify an all-agree refresh stays a no-op.
- **`Money` equality is value-based across scale** (`equal_value_edit_keeps_validated_even_across_scales`) — `reconcile`'s divergence test reuses it.
- **Open issue #46** (3.3 review): an empty 0-year provider payload reads as "no change" — out of scope here (Story 3.5).

### Testing standards

- Headless Rust unit/integration tests (Slint-native, no-web — the QA e2e step is N/A). Use the 3.3 `fetched_for`/`fetched_custom` helpers + `set_review`/`edit_cell` to build divergence scenarios.
- **Contract:** `reconcile` semantics (divergent/equal/non-validated/edited-clears-pending) + round-trip + old-JSON-without-pending → None.
- **App:** manual value preserved on divergent refresh; pending set + surfaced; `✓` demotes; verdict degrades through a real refresh; accept-provider / keep-manual / re-validate clear the pending; idempotent agree-refresh is a no-op.
- **AC6 guard:** a pending-bearing cell produces the same `build_frame` as `pending = None` (the engine ignores it).
- **Keep `seam_check.rs` green.** All four gates `--locked`; pinned rustfmt 1.9.0.
- UI story → on-display GO/NO-GO is part of DoD (Task 6).

### Open questions for dev (resolve during implementation, don't block)

- **`keep_manual` review effect:** leave review as-is (already demoted to `?` by the divergence in Task 2) vs force `?`. Leaning leave-as-is (the divergence already demoted it; keep-manual just clears the pending). If the cell was never `✓`, keep-manual leaves it untouched but clears pending.
- **Equal-refresh clears a stale pending?** If a cell has an old pending and a new refresh now AGREES with the manual value, should the pending clear? Leaning **yes** (`reconcile` on equal → `pending = None`), so a resolved-upstream divergence self-heals. Confirm it still counts as "changed" only when it actually clears something (idempotency).
- **One pending slot vs history:** one `Option<PendingProvider>` (latest divergence wins) is enough for v1; a list of historical divergences is out of scope.
- **Resolve-control placement:** inline near the focused cell vs in the source/timestamp caption row. Either; keep it keyboard-reachable (NFR-U2) and revealed-on-demand (not an always-on per-cell pair of buttons).

### Project Structure Notes

- **First Epic-3 `contract` change** (additive field + primitive). `core`/`persistence` source untouched (no `Cell {…}` literals there; `Cargo.lock`/`deny.toml` unchanged). No schema/DDL change, no `SCHEMA_VERSION` bump, no `method_version` change.
- Matches the architecture source tree: `contract/src/cell.rs`, `app/src/state.rs`, `app/ui/...`. The `core/reconcile.rs` tree comment is superseded — the rail lives in `contract::Cell` (documented above).

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.4 (lines 791–804)] — AC source; the 3.3/3.4/3.5/3.6 split.
- [Source: _bmad-output/planning-artifacts/prd.md (FR22 lines 702–703, NFR-R4 line 862, FR20 lines 697–700, line 39 data-model, line 46 attention hierarchy, lines 315–325 Journey-2b reconcile)] — requirements.
- [Source: _bmad-output/planning-artifacts/architecture.md (lines 154–155 non-destructive reconciliation / divergence→?, line 654 reconcile.rs tree comment [superseded], lines 82–85 transactional recompute)].
- [Source: contract/src/cell.rs (Cell, Cell::edited lines 76–95, the proptest rail)] — the primitive to extend.
- [Source: contract/src/lib.rs:12–14 + versioning.rs:12 + study.rs:49–53] — forward-compat policy + the Story-2.2 additive-field precedent (no SCHEMA_VERSION bump).
- [Source: app/src/state.rs — apply_provider_refresh / refresh_cell / refresh_optional / RefreshReport (Story 3.3)] — the refresh rail to extend; the `NotAvailableAccepted` skip + idempotency to preserve.
- [Source: app/src/viewmodel/form.rs editable_cell + app/ui/state.slint GridCellState + editable_cell.slint + study_screen.slint] — the revealed-on-demand provenance channel (source 3.2 / timestamp 3.3) `pending` joins.
- [Source: Story 3.3 — 3-3-manual-refresh-recompute-freshness.md + commit 4902c42] — the refresh rail, the N/A-accepted skip, the idempotency invariant, the verdict-degrade test pattern.
- [Source: seam_check.rs SEAM 2/3] — the divergence-demotion + verdict-degrade rails reconcile rides; keep green.
- [Source: memory project-planning-progress — CHECKPOINT 2026-06-27] — 3.3 done; 3.4 needs the 2nd value slot on contract::Cell.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Opus 4.8, 1M context)

### Debug Log References

- `cargo build --workspace --tests` → listed the `missing field pending` sites; 13 full `Cell {…}` literals updated (the spread-syntax ones needed no change).
- `cargo test -p steadyinvest-contract` → contract 24 unit + 14 round-trip pass (reconcile + edited-clears-pending + pending round-trip + old-JSON→None + the reconcile proptest).
- `cargo test -p steadyinvest-app` → app **176** (169 → 176: +7 reconciliation/resolve/reopen/AC6-engine-ignores).
- **Gate failure caught + fixed:** `persisted_study_shape_is_byte_pinned` (corpus_gate) failed — the additive `pending` field serializes as `"pending":null` on every cell, changing the pinned JSON. Per the project's own precedent (the Story-2.2 note in that test), **re-captured the pinned string** (each cell gains a trailing `"pending":null`) — NOT a `SCHEMA_VERSION` bump, no new corpus file; the frozen v1.db still reads back equal via `serde(default)`.
- **Banned-verb caught + fixed:** the keep-manual button label "Garder (manuel)" tripped the posture gate (`garder` ∈ `BANNED_VERBS_FR`, as is `conserver`). Reworded to the neutral "Ignorer (fournisseur)" (dismiss the provider divergence → the manual value stands).
- `cargo fmt --all --check` ✓ · `cargo clippy --workspace --all-targets -- -D warnings` ✓ (0 issues) · `cargo deny check` ✓ · `timeout 8 cargo run` → exit **124** (healthy event loop).

### Completion Notes List

- **Tasks 1–5 complete; Task 6 (manual on-display GO/NO-GO) is the RESIDUAL** (needs Guy's desktop — the perceptual check of the preserved manual value, the `✓→?`, the revealed pending, and the two resolve buttons). Same pattern as 3.1–3.3.
- **First Epic-3 `contract` change, kept minimal:** a single additive `#[serde(default)] Cell.pending` field + the `Cell::reconcile` primitive (beside `Cell::edited`). **No `SCHEMA_VERSION` bump, no migration, no `v2.db`, no method change.** `core`/`persistence` **SRC** untouched — only their *test* helpers gained `pending: None` and the corpus pin was re-captured. Method fingerprint / determinism / golden / frozen v1.db all green.
- **Non-destructive by construction (FR22/NFR-R4):** `reconcile` never writes the live value; a divergent provider value lands in `pending` (distinct, attributed); a divergent `✓` demotes (invariant 2b) and the verdict degrades in the same frame; the engine never reads `pending` (AC6 guard test). Manual and provider values are never merged.
- **Idempotency preserved (3.3 invariant):** a manual cell is only re-stamped when `reconcile(...)` actually changes it; an agreeing re-refresh is a no-op (no phantom undo step — tested).
- **Resolution (AC4):** explicit only — accept-provider / keep-manual buttons, plus implicit resolution by editing the cell (`edited` clears pending) or re-validating it (`set_review(Validated)` clears pending). No automatic merge.
- **Reconcile primitive lives in `contract`** (beside `edited`), NOT `core` — a deliberate, documented deviation from the architecture tree's `core/reconcile.rs` comment (the demotion logic already lives in `contract`; a `core` change would risk the method-fingerprint gate for no benefit).

### File List

**Modified — contract (the data-shape change)**
- `contract/src/cell.rs` — `PendingProvider` struct; `Cell.pending` field; `Cell::reconcile`; `edited` clears pending; reconcile unit tests + proptest; 9 in-file literals updated.

**Modified — app (the rail + resolution + UI)**
- `app/src/state.rs` — `refresh_cell` Manual branch → `reconcile`; `CellRefresh::Reconciled`; `RefreshReport.reconciled`; `accept_provider_value`/`keep_manual_value`; `set_review` clears pending; `provider_cell`/`tofill_cell` literals; reconciliation/resolve/reopen/AC6 tests.
- `app/src/viewmodel/form.rs` — `pending_value` helper; `GridCellState.pending` populated; test literal.
- `app/src/main.rs` — `on_accept_provider`/`on_keep_manual` callbacks.
- `app/src/posture.rs` — `@tr` floor `224 → 227`; test literal.
- `app/src/viewmodel/entry.rs` — `tofill_cell` literal + test literals.
- `app/src/viewmodel/{chart,engine,verify}.rs`, `app/src/seam_check.rs` — test literals updated.
- `app/ui/state.slint` — `GridCellState.pending`; `Studies.active-pending` + `accept-provider`/`keep-manual` callbacks.
- `app/ui/components/editable_cell.slint` — set `active-pending` on focus.
- `app/ui/screens/study_screen.slint` — pending reveal caption + the two resolve controls.

**Modified — test helpers / pin (no SRC change)**
- `contract/tests/roundtrip.rs`, `core/tests/verdict_coherence.rs`, `persistence/tests/{e2e_lifecycle,journal_roundtrip}.rs` — `pending: None` in test cell helpers.
- `persistence/tests/corpus_gate.rs` — `pending: None` in the test helper + the byte-pinned JSON re-captured to the `"pending":null` shape (additive; documented).

### Change Log

- 2026-06-27 — Story 3.4 implemented (non-destructive reconciliation, FR22/NFR-R4). First Epic-3 contract change: additive `Cell.pending` (`#[serde(default)]`, no SCHEMA_VERSION bump) + `Cell::reconcile` primitive. The refresh rail reconciles a divergent manual cell (manual wins, provider value preserved alongside, ✓→? demote, verdict degrades) instead of skipping it; accept-provider / keep-manual resolution + implicit resolution by edit/re-validate; the divergence is surfaced revealed-on-demand. Engine ignores `pending` (AC6). Corpus pin re-captured; "Garder"→"Ignorer" (banned verb). app 169 → 176 tests, contract +6; all four gates green; method/golden/corpus/v1.db clean; Cargo.lock untouched. Status → review. Task 6 (manual on-display GO/NO-GO) pending Guy's display.
- 2026-06-27 — 3-layer adversarial code review (Blind + Edge + Acceptance). Acceptance Auditor: **ACCEPT** (AC1–AC6 all PASS; scope held: additive field only, no SCHEMA_VERSION bump, core/persistence SRC untouched, corpus pin per precedent, reconcile in contract, posture clean). **3 patches applied** (2 MEDIUM + 1 LOW), 0 deferred, rest dismissed (verified). app 176 → 178 tests; all gates re-green. Status → done.

## Review Findings (3-layer adversarial code review, 2026-06-27)

Layers: Blind Hunter (diff-only) + Edge Case Hunter (diff + project) + Acceptance Auditor (diff + spec). Auditor verdict: **ACCEPT** — AC1–AC6 implemented to intent; the core `Cell::reconcile`/`edited` primitives and state rails were verified well-covered. The findings were all at the UI seam + one idempotency churn. 3 patch · 0 defer · several dismissed.

### Patches (applied)

- [x] [Review][Patch] **MEDIUM — the resolve controls go stale after accept/keep** [app/src/main.rs, ui] — `Studies.active-pending` is set only on cell focus; the accept/keep callbacks rebuilt the grid but never cleared it, so the "Fournisseur : {}" caption + the Accepter/Ignorer buttons lingered for the resolved cell, and a re-click hit the no-op path. **Fix:** the callbacks now `set_active_pending("")` on success (hides the controls immediately), AND `accept_provider_value`/`keep_manual_value` early-return a true no-op when there is no pending (no `mutate_cell`, no journal write) — guarded test `accept_or_keep_with_no_pending_is_a_true_noop` (undo depth + `logical_version` unchanged).
- [x] [Review][Patch] **MEDIUM — a repeated divergent refresh re-stamps the pending (churn)** [contract/src/cell.rs `Cell::reconcile`] — a per-fetch provenance timestamp made every re-refresh of the SAME unresolved divergence a "change" → a phantom undo step + journal revision (the Story-3.3 churn trap, now for `pending`). **Fix:** `reconcile` returns `self` unchanged when the existing `pending` already holds the same divergent value (idempotent — keep the original provenance). Tests `reconcile_is_idempotent_on_a_repeated_same_value_divergence` (contract) + `a_repeated_divergent_refresh_is_idempotent` (app).
- [x] [Review][Patch] **LOW — accept-provider on a `value:None` pending would blank the manual value** [app/src/state.rs `accept_provider_value`] — defensive: `PendingProvider.value` is `Option<Money>`, and `edited(None, …)` would clear the manual value. Currently unreachable (the refresh path only reconciles `Some(v)`), but `reconcile` is public. **Fix:** accept treats a `None`-valued pending as keep-manual (never destroys the live value).

### Dismissed (verified / acceptable)

- **MEDIUM/LOW (edge) — a verdict-neutral reconcile still emits a "Recalculé …" notice.** A reconcile on an un-validated manual cell (or a self-heal) changes `pending` but not the verdict. The notice truthfully states a refresh reconciled data; the cause-named wording is acceptable and the design is the intentional 4-message scheme. Not worth a distinct "reconciled, verdict unchanged" message.
- **LOW (blind) — `PendingProvider.value` has `serde(default)` but `provenance` does not.** Informational asymmetry; a pending is always written whole. No bug.
- **(auditor minors) — AC6 guard asserts verdict-equality not full-frame byte-equality; the reconcile proptest's `cell()` starts `pending:None`.** Both adequate: the verdict is the load-bearing engine output, and the stale-pending self-heal is covered by a dedicated unit test.
