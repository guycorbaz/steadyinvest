# Story 2.7: Low-confidence & plausibility surfacing

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want thin history and suspicious inputs surfaced honestly,
so that I am never misled by a confident-looking but unsupported verdict.

## Acceptance Criteria

**AC1 — Low-confidence label, carried into the verdict (FR8 surfacing)**
**Given** a study with fewer than five usable years
**When** the verdict is shown (badge + sticky verdict bar)
**Then** the study carries a visible **"insufficient history / low confidence"** label, attached to the verdict surface — not a separate banner — so the label travels with the verdict wherever it renders.

**AC2 — Plausibility issue surfaced inline at the cell (FR10 surfacing)**
**Given** an input plausibility issue (split/series break, currency mismatch, fiscal-period misalignment, out-of-bound ratio, negative/zero denominator, low-price-above-current)
**When** the engine reports it for a study
**Then** it surfaces as a **neutral inline warning at the affected §2/§3 cell** — a non-colour attention glyph in a channel distinct from the coverage glyph, the stale murmur, and the tri-state review marker.

**AC3 — Distinct from quality flags and from the review tag**
**Given** a cell that simultaneously carries a review tag (`✓`/`?`) and a plausibility warning
**When** both render
**Then** the plausibility glyph is visually and positionally distinct from the review marker, from the coverage/stale markers, and from quality flags (which are methodology signals, not per-cell input warnings) — no two channels collide or are confusable (the confusability gate is honoured).

**AC4 — Neutral voice**
**Given** any low-confidence label or plausibility warning text
**When** it is shown
**Then** the text states the fact only and contains no banned/imperative verb (FR13) — it passes the crate-local posture gate, exactly like every other user-facing string.

**AC5 — Warning detail is keyboard-reachable and reduced-motion-safe**
**Given** a cell carrying a plausibility warning
**When** the cell is focused (keyboard or pointer)
**Then** the human-readable warning fact is revealed (mirroring the existing source-on-demand caption), reachable without a pointer (NFR-U2), with no animation when the OS requests reduced motion.

**AC6 — Honest absence**
**Given** a study with ≥ 5 usable years and no plausibility findings
**When** it renders
**Then** no low-confidence label and no warning glyphs appear — the channels are silent by default (the absence of a warning is itself information; do not show an "OK" badge).

**AC7 — Engine is consumed, never modified; thin-UI rule holds**
**Given** the engine already computes both finding sets and the low-confidence flag (Epic 1)
**When** this story surfaces them
**Then** all detection logic stays in `core` (Cardinal Rule); the app only reads `CanonicalFinancials.findings`, `SsgOutputs.findings`, and the low-confidence flag, maps them to UI addresses, and renders. `core`, `contract`, `persistence` are unchanged; `Cargo.lock` and `deny.toml` are unchanged (no new dependency).

**AC8 — Gates green**
`cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --all --locked`, and `cargo deny check` all pass. Pinned surfaces (`core/`, `contract/`, `persistence/`, `ingestion/`, `report/`, `docs/method/`, `.github/`, `rust-toolchain.toml`, `deny.toml`, `Cargo.lock`) re-diff empty.

## Tasks / Subtasks

- [x] **Task 1 — Surface the input-shape findings out of the engine wiring (AC2, AC7)**
  - [x] In `app/src/viewmodel/engine.rs`, change `build_snapshot` so the **input-shape findings on `CanonicalFinancials` are not discarded**. Today `build_snapshot(study) -> Result<StudySnapshot, NormalizeError>` calls `normalize::normalize(raw)?` and moves the `CanonicalFinancials` into `StudySnapshot::new(...)`, which does **not** re-expose `.findings`. Return both in one coherent frame — e.g. a small struct `pub struct StudyFrame { pub snapshot: StudySnapshot, pub plausibility: Vec<core::normalize::Finding> }` (clone `canonical.findings` before the `new(...)` move) — so the input-shape findings (`split_series_break`, `currency_mismatch`, `fiscal_period_misalignment`) reach the UI. Do **not** re-run `normalize` a second time (would risk a drift between the frame that produced the verdict and the frame that produced the warnings).
  - [x] The calc-time findings (`SsgOutputs.findings`) and the low-confidence flag are already reachable via `snapshot.outputs().findings` and `snapshot.outputs().low_confidence` (or `snapshot.verdict().low_confidence()`). No new path needed for those.
  - [x] Update the single call site in `app/src/main.rs::push_form` to consume the new return shape.

- [x] **Task 2 — Map findings to UI addresses (AC2, AC3)**
  - [x] Add a `plausibility` adapter in `app/src/viewmodel/engine.rs` that turns the two finding sets into per-cell warning state. Each `Finding`/`CalcFinding` carries `key: PlausibilityKey`, a year (`Finding.year: i32`; `CalcFinding.year: Option<i32>`), and `context: &'static str` (a field name). Resolve `(year, context)` to a `(year_index, field)` cell address using the materialized-year window (`viewmodel::form::materialized_year_numbers`) and the context→field table below.
  - [x] **Context → §2/§3 cell field address** (verified against `form::pe_rows` / `form::mgmt_rows`):
    - `"high_price"` → §3 `"a"` · `"low_price"` → §3 `"b"` · `"eps"` → §3 `"c"` · `"dividend"`/`"dividend_per_share"` → §3 `"f"`
    - `"sales"` → §2 `"sales"` · `"net_profit"`/`"pre_tax_profit"`/`"pretax"` → §2 `"pretax"` · `"book_value_per_share"` → §2 `"book"`
  - [x] **Findings with no raw input cell** — anchor, do not drop:
    - Fiscal contexts `"period_months"` / `"fiscal_year_end_month"` are per-year but not a value cell → anchor at the **year (row/column) level** for that `year_index` (the whole year is suspect, not one number).
    - Calc-time derived contexts (`"ptp_pct"`, `"roe_pct"`, `"high_pe"`, `"low_pe"`, `"current_pe"`, `"ttm_eps"`) → anchor at the contributing **input cell** of that year when one exists; otherwise at the year level.
    - Study-level `"forecast_low"` (`LowPriceAboveCurrent`, `year == None`) → anchor near the **§4 / judgment area** (forecast-low), not a §2/§3 cell.
  - [x] Record this mapping + the anchor policy as a Story-2.7 interpretation (new GitHub issue, per the project convention — see "Interpretations" below). It is a spec-underspecified decision.

- [x] **Task 3 — Carry the low-confidence label into the verdict (AC1, AC4, AC6)**
  - [x] Extend `VerdictState` (`app/ui/state.slint`) with `low-confidence: bool` and `confidence-label: string` (empty when not low-confidence).
  - [x] In `engine::verdict_badge`, set them from `snapshot.verdict().low_confidence()` (true ⟺ `< 5` usable years; always `false` for a `Full` verdict by construction). Label text (FR13-neutral, French UI): **`"Historique insuffisant — confiance réduite"`**.
  - [x] In `app/ui/components/verdict_badge.slint`, render the label on **both** `VerdictBadge` and the sticky `VerdictBar` so it travels with the verdict. The verdict is **already** rendered Provisional/Withheld when low-confidence (Story 2.6 drives the texture); 2.7 only adds the explicit textual reason. Do **not** spend colour — the label is neutral ink.
  - [x] The label must show whenever `low_confidence` is true, regardless of whether the verdict is Provisional or Withheld (a study can be both low-confidence and missing inputs).

- [x] **Task 4 — Render the cell-level warning glyph (AC2, AC3, AC5)**
  - [x] Extend `GridCellState` (`app/ui/state.slint`) with `warning: bool` and `warning-key: string` (the `PlausibilityKey::as_str()` value, "" when none). Populate it in `viewmodel::form::editable_cell` from the Task-2 adapter output (thread the per-cell warning lookup into the `pe_rows`/`mgmt_rows` builders).
  - [x] Add a token `warn-glyph` to `app/ui/tokens.slint` — a **neutral outline triangle `"△"`** (deliberately a different shape from gap `"▦"`, stale `"◦"`, review `"?"`/`"✓"`, and the `"n/a"` marker), drawn in neutral ink (`gap-ink` / `text-high`), never a judgment hue.
  - [x] In `app/ui/components/editable_cell.slint`, render `△` when `state.warning` — positioned in a **corner distinct from the trailing review marker and the coverage glyph** (suggest top-leading). It must coexist with `✓`/`?`/stale/gap without overlap or confusion (AC3).
  - [x] On cell focus, reveal the human-readable warning fact via a caption channel mirroring the existing `Studies.active-source` → `@tr("Source : {}", ...)` pattern in `study_screen.slint:268`. Add a parallel `Studies.active-warning` in-out string set by Rust on focus, rendered as `@tr("Signalement : {}", Studies.active-warning)`. No animation under reduced motion.

- [x] **Task 5 — Neutral warning microcopy per key (AC4)**
  - [x] Provide one fact-only French string per `PlausibilityKey`, e.g.:
    - `split_series_break` → `"Rupture de série détectée"`
    - `currency_mismatch` → `"Devise incohérente"`
    - `fiscal_period_misalignment` → `"Période fiscale non alignée"`
    - `out_of_bounds_ratio` → `"Valeur hors de la plage attendue"`
    - `negative_or_zero_denominator` → `"Dénominateur nul ou négatif"`
    - `low_price_above_current` → `"Prix bas supérieur au prix actuel"`
  - [x] If these live in Rust (as `MSG_*` consts), add them to `state::USER_FACING_MESSAGES` so the posture gate scans them; if they live as `@tr()` literals in `.slint`, the `ui_tr_strings_are_neutral_no_banned_verb` test already covers them. Either way they must be scanned (AC4).

- [x] **Task 6 — Headless tests (AC1–AC4, AC6, AC7)**
  - [x] Adapter: a `Finding`/`CalcFinding` with a given `(year, context)` maps to the expected `(year_index, field)` cell address; the materialized-year window aligns (reuse the 2.6 alignment helper).
  - [x] Anchor policy: fiscal-context and study-level findings (`forecast_low`, `year=None`) do not crash and resolve to their year-level / §4 anchors (not dropped, not mis-attached to a value cell).
  - [x] Low-confidence: a study with `< 5` usable years yields `VerdictState.low_confidence == true` and a non-empty `confidence-label`; a ≥ 5-year clean study yields `false`/empty and no cell warnings (AC6).
  - [x] Coexistence: a cell that is both `✓ validated` and warned carries `review == "validated"` **and** `warning == true` (the two channels are independent — AC3).
  - [x] Neutrality: every new warning/label string passes the banned-verb posture gate (extend/parametrize the existing posture test).
  - [x] Pinned-surface guard: assert (or document in the File List) that `core/`, `contract/`, `persistence/`, `Cargo.lock`, `deny.toml` are untouched.

- [x] **Task 7 — Gates & sprint status (AC8)**
  - [x] Run the 4 gates locally `--locked`; record the app test count delta in the Dev Agent Record.
  - [x] GUI click-through of glyph rendering / focus-caption is left for human/AT-SPI verification (sandbox limitation), per the 2.3–2.6 precedent — note it explicitly.

## Dev Notes

### What this story is (and is not)

This is an **app-only surfacing story**. The engine (Epic 1) **already detects** both finding sets and the low-confidence state and **already drives** the verdict texture to Provisional/Withheld when low-confidence (Story 2.6). 2.7 makes two already-computed truths **visible**: (1) the low-confidence *reason* as explicit text on the verdict, and (2) the per-input plausibility findings as neutral cell-level glyphs. No detection, no thresholds, no new method logic — those live in `core` and changing them is out of scope (and would require a `METHOD_VERSION` bump).

### Exact engine API to consume (verified — do NOT modify these crates)

- **Input-shape findings** — `core::normalize::Finding` (`core/src/normalize/types.rs:142`): `{ key: PlausibilityKey, year: i32, context: &'static str }`, carried on `CanonicalFinancials.findings: Vec<Finding>` (`types.rs:191`). Emitted by the currency / fiscal / split-break passes (`normalize/checks.rs`, `normalize/splits.rs`). **These are currently dropped by `build_snapshot` — Task 1 fixes that.**
- **Calc-time findings** — `core::ssg::CalcFinding` (`core/src/ssg/types.rs:174`): `{ key: PlausibilityKey, year: Option<i32>, context: &'static str }`, on `SsgOutputs.findings` (`types.rs:403`). Already reachable via `snapshot.outputs().findings`.
- **`PlausibilityKey`** (`core/src/normalize/types.rs:116`) — 6 variants: `SplitSeriesBreak`, `CurrencyMismatch`, `FiscalPeriodMisalignment`, `OutOfBoundsRatio`, `NegativeOrZeroDenominator`, `LowPriceAboveCurrent`. `.as_str()` → snake_case wire keys.
- **Low-confidence** — `SsgOutputs.low_confidence: bool` (`core/src/ssg/types.rs:405`) = `usable_years < USABLE_YEARS_FLOOR` (`= 5`, `core/src/method/mod.rs:17`). Also queryable on the verdict: `Verdict::low_confidence()` (`core/src/verdict.rs:274`/`342`) — `false` for `Full`, the stored flag for `Provisional`/`Withheld`.
- **Snapshot accessors** (`core/src/verdict.rs`): `StudySnapshot::outputs()` (`:389`), `::verdict()` (`:399`), `::gates()` (`:394`). `StudySnapshot::new(...)` (`:371`) consumes `&CanonicalFinancials` — it does **not** re-expose its `.findings`, which is why Task 1 must clone them before the move.

**Distinction to preserve (architecture + UX):** *quality flags* (`SsgOutputs.quality_flags: Vec<QualityFlagKey>`, FR7) are methodology-threshold signals and are **out of scope** for this story's cell glyph — do not render quality flags as plausibility warnings. They are a different concept and a different channel.

### UI surfaces to touch (verified file:line)

- `app/src/viewmodel/engine.rs` — `build_snapshot` (`:196`, change return shape), new `plausibility` adapter + `verdict_badge` (`:405`) low-confidence fields.
- `app/src/viewmodel/form.rs` — `editable_cell` (`:58`), `pe_rows` (`:101`), `mgmt_rows` (`:135`): thread per-cell warning lookup in.
- `app/src/main.rs` — `push_form` (`~:119`): consume new `build_snapshot` shape; set `Studies.active-warning` on focus.
- `app/ui/state.slint` — `GridCellState` (`:41`, add `warning`, `warning-key`); `VerdictState` (`:156`, add `low-confidence`, `confidence-label`); add `Studies.active-warning`.
- `app/ui/components/editable_cell.slint` — render `△` warning glyph (coverage/stale markers at `:65–92`, review marker mount at `:197–206`; place the new glyph clear of both).
- `app/ui/components/verdict_badge.slint` — `VerdictBadge` (`:44`) + `VerdictBar` (`:99`): render the low-confidence label.
- `app/ui/tokens.slint` — add `warn-glyph: "△"` near the manual-entry textures (`gap-glyph`/`stale-dot`, `~:81`).
- `app/ui/screens/study_screen.slint` — focus caption mirror of the `Source : {}` pattern (`:268`).
- `app/src/state.rs` — if warning strings are Rust consts, add to `USER_FACING_MESSAGES` (`:87`).

### Attention hierarchy & colour budget (UX — do not violate)

- **Saturated colour is spent only on the three judgment zones / the Full verdict.** Everything else is neutral greyscale ink. The plausibility glyph and the low-confidence label are **neutral** (texture/shape/opacity, never a hue). [Source: ux-design-specification.md#Guiding Principle — A Monastic Colour Budget]
- The single sanctioned colour exception is the `✓` validated ink-green (`#4A7C6F`), geofenced and never co-present with zone bands. The warning glyph must **not** reuse it. [Source: ux-design-specification.md#State & Trust Markers]
- Attention hierarchy: missing shouts (bold `▦`), stale murmurs (~60% opacity + `◦`), source revealed on demand. The plausibility warning is a **fourth, distinct** texture channel — inline at the cell, distinct from quality flags and the review tag. [Source: ux-design-specification.md#Feedback Patterns — "Plausibility warnings → inline at the cell, a neutral attention glyph (distinct from quality flags and from the review tag)."]
- **Confusability gate:** the trust markers must reach ≥98% correct identification with <2% pairwise confusion at 14 px on the real background. The new `△` is one more glyph in that set — keep it shape-distinct from `▦ ◦ ? ✓` and from the `n/a` marker (snapshot/perceptual test is forwarded, not in this story, but design to pass it). [Source: ux-design-specification.md#Accessibility Considerations]

### Accessibility (NFR-U1/U2, reduced motion)

- Decision/warning never colour-only: the glyph carries shape; the detail is text on focus. Colour-blind-safe by construction (no hue spent). [NFR-U1]
- Keyboard-first: the warning detail must be reachable by focusing the cell, not only by hover. [NFR-U2]
- Respect OS reduced-motion: no animated warning/label state. [Source: ux-design-specification.md#Responsive Design & Accessibility]

### Project Structure Notes

- Pinned surfaces (CI-guarded; re-diff empty): `core/`, `contract/`, `persistence/`, `ingestion/`, `report/`, `docs/method/`, `.github/workflows/ci.yml`, `rust-toolchain.toml`, the frozen `persistence/tests/corpus/v1.db`, `deny.toml`, **`Cargo.lock`**.
- **No new dependency.** `core`/`contract` are already in the dependency tree and lock file (Story 2.6 first called `core`). This story adds no crate → `Cargo.lock` stays byte-identical.
- The §3 P/E table has **no row cap** (issue #20 fix landed in 2.4 — `PE_TABLE_ROWS = 5` removed). Warnings on year 6+ must surface too; rely on the materialized-year window, not a fixed `[0,5)`.

### Previous Story Intelligence (Story 2.6 — the engine-wiring story)

- The **single construction path** is `engine::build_snapshot` → `StudySnapshot::new(...)` **once** per frame, called once in `push_form`. Keep it one call: outputs, verdict, and now plausibility must come from the **same frame** (no second `normalize`). This is the coherence invariant that makes verdict integrity hold by construction.
- Slint gotchas already hit: `z` and `row` are reserved attached properties (2.6 renamed a `ZoneBar` input to `bar`); `@children` inside a conditional is illegal; ids unreachable from a root-function inside a conditional. The 2.4 `editable_cell.slint` intercepts Ctrl/Cmd chords first in `key-pressed` — adding a glyph doesn't touch input handling, but keep commit-on-focus-out discipline intact.
- `clippy --all-targets` builds the binary without `cfg(test)`, so a new adapter fn must be reached from the production `push_form` path, or it is dead code that fails clippy. Wire it in, don't leave it test-only.
- Data crosses the Rust↔Slint boundary as **pre-formatted strings / bools / floats only** — never a `Decimal` or a domain enum. The warning crosses as `bool` + a snake_case `warning-key` string + the focus caption string. No arithmetic in Slint (Cardinal Rule).
- Test style: pure-Rust headless `#[test]`s on domain types (no `slint::test`), sentence-case assertion messages. App tests were 102 after 2.6; add to that count.

### References

- [Source: epics.md#Story 2.7: Low-confidence & plausibility surfacing] — the two AC clauses (FR8 + FR10 surfacing).
- [Source: epics.md#Functional Requirements] — FR8 (≥5-usable-year floor, queryable low-confidence), FR10 (plausibility warnings distinct from quality flags), FR12 (verdict degraded/withheld when low-confidence), FR13 (neutral voice / banned verbs).
- [Source: core/src/normalize/types.rs:116,142,186] — `PlausibilityKey`, `Finding`, `CanonicalFinancials.findings`.
- [Source: core/src/ssg/types.rs:174,389] — `CalcFinding`, `SsgOutputs` (`findings`, `low_confidence`, `quality_flags`).
- [Source: core/src/verdict.rs:274,371,389,399] — `Verdict::low_confidence`, `StudySnapshot::new/outputs/verdict`.
- [Source: app/src/viewmodel/engine.rs:196,405] — `build_snapshot`, `verdict_badge`.
- [Source: app/src/viewmodel/form.rs:58,101,135] — `editable_cell`, `pe_rows`, `mgmt_rows`.
- [Source: app/ui/state.slint:41,156] — `GridCellState`, `VerdictState`.
- [Source: ux-design-specification.md#Guiding Principle — A Monastic Colour Budget; #State & Trust Markers; #Feedback Patterns; #Accessibility Considerations] — colour discipline, marker channels, confusability gate, NFR-U1/U2.
- [Source: architecture.md#Frontend Architecture; #Implementation Patterns] — immutable-snapshot pattern, Cardinal Rule (no calc outside `core`), plausibility-is-non-blocking.
- Related open issues: #12 (spec: quantify `split_series_break`) — background only, not blocking surfacing.

### Interpretations (file as a new GitHub issue per project convention — Issues are the single source of truth)

Open a "Story 2.7 interpretations" issue (repo `guycorbaz/steadyinvest`) capturing the spec-underspecified decisions made here:
1. The `context → (section, field)` mapping table and the anchor policy for non-cell findings (fiscal year-level; derived-context fallback; study-level `forecast_low` → §4).
2. The chosen warning glyph (`△`) and its cell corner placement (confusability with the existing marker set).
3. The low-confidence label wording and that it renders on both badge and sticky bar, independent of the Provisional/Withheld split.
4. The `StudyFrame` return-shape change to `build_snapshot` (surfacing input-shape findings without a second `normalize`).

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8 (1M context) — `claude-opus-4-8[1m]`

### Debug Log References

- All four gates pass `--locked`: `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --all --locked`, `cargo deny check` (advisories/bans/licenses/sources ok).
- App test count: **97 → 102 (+5)**. New: 4 in `viewmodel::engine` (`plausibility_maps_findings_to_their_cell_addresses`, `non_cell_findings_anchor_at_year_or_study_never_dropped_or_misattached`, `low_confidence_label_only_under_five_usable_years`, `a_clean_five_year_study_is_silent_and_full_confidence`) + 1 in `viewmodel::form` (`a_warned_validated_cell_carries_both_channels_independently`). Workspace total unchanged in `core`/`contract`/`persistence` (no test edits there).
- Slint gotcha hit & fixed: a global `function` must be `public pure function` to be callable cross-component (compiled as a warning that "used to be allowed"; promoted to `public`).

### Completion Notes List

- **Task 1 (StudyFrame):** added `engine::build_frame(study) -> Result<StudyFrame { snapshot, plausibility: Vec<Finding> }>`; `build_snapshot` is now a thin wrapper (`build_frame(study).map(|f| f.snapshot)`), so all Story-2.6 snapshot-only call sites (`state::snapshot_for`, adapter tests) are unchanged. Input-shape findings are cloned off `CanonicalFinancials` BEFORE the `StudySnapshot::new(...)` move — surfaced with **no second `normalize`** (one coherent frame). `push_form` consumes `build_frame`.
- **Task 2 (adapter):** `engine::plausibility(input_findings, calc_findings, year_numbers) -> PlausibilityWarnings`; resolves each finding to `WarningAnchor::{Cell, Year, Study}` via the `context → field` table + the derived-ratio fallback. Fiscal metadata → Year; year-less `forecast_low`/`current_pe` and out-of-window findings → Study (anchored, never dropped/mis-attached). Mapping + anchor policy filed as **GitHub issue #24** (interpretations).
- **Task 3 (low-confidence label):** `VerdictState` gains `low-confidence: bool` + `confidence-label: string`, set in `verdict_badge` from `Verdict::low_confidence()`. Label "Historique insuffisant — confiance réduite" renders on both `VerdictBadge` and the sticky `VerdictBar` (bar disables the embedded badge's copy → shows once); shown independent of Provisional/Withheld; neutral ink, no colour.
- **Task 4 (cell glyph):** `GridCellState` gains `warning: bool` + `warning-key: string`; the `△` `warn-glyph` token renders at the cell top edge (clear of trust marker, coverage/stale markers, lock). Focus reveals the fact via `Studies.active-warning` (mirrors source-on-demand); static glyph → reduced-motion-safe by construction.
- **Task 5 (microcopy):** one fact-only French string per `PlausibilityKey`, kept as `@tr()` literals in the `Warnings` global in `state.slint` — covered by the `ui_tr_strings_are_neutral_no_banned_verb` posture gate (AC4). The `CONFIDENCE_LOW` Rust const is registered in `engine::USER_FACING_LABELS` (posture count 21 → 22).
- **Task 6/7 (tests & gates):** headless tests cover the cell mapping, the anchor policy for non-cell findings, the low-confidence label gate, the clean-study silence (AC6), and the review×warning channel independence (AC3). **GUI click-through** of glyph rendering / focus-caption / AT-SPI is left for human verification (sandbox limitation), per the 2.3–2.6 precedent.
- **AC7/AC8:** detection logic untouched (Cardinal Rule) — pinned surfaces `core/`, `contract/`, `persistence/`, `ingestion/`, `report/`, `docs/method/`, `.github/`, `rust-toolchain.toml`, `deny.toml`, **`Cargo.lock`** re-diff **empty** (`git status` shows only `app/` files); no new dependency.

### File List

- `app/src/viewmodel/engine.rs` — M: `StudyFrame`/`build_frame`; `build_snapshot` delegates; `PlausibilityWarnings`/`PlausibilityWarning`/`WarningAnchor` + `plausibility`/`context_to_field`/`resolve_anchor`; low-confidence fields in `verdict_badge`; `CONFIDENCE_LOW` const (+ `USER_FACING_LABELS`); 4 tests; imports.
- `app/src/viewmodel/form.rs` — M: `editable_cell`/`pe_rows`/`mgmt_rows` thread the per-cell warning lookup into `GridCellState`; 1 test; existing tests pass `&PlausibilityWarnings::default()`.
- `app/src/main.rs` — M: `push_form` consumes `build_frame`, computes warnings, threads them into `pe_rows`/`mgmt_rows`, sets `section4-warning-key`; resets `active-warning` on open.
- `app/src/posture.rs` — M: engine label inventory count 21 → 22.
- `app/ui/state.slint` — M: `GridCellState` (`warning`, `warning-key`); `VerdictState` (`low-confidence`, `confidence-label`); `Studies` (`active-warning`, `section4-warning-key`); new `Warnings` global (neutral microcopy).
- `app/ui/tokens.slint` — M: `warn-glyph: "△"`.
- `app/ui/components/editable_cell.slint` — M: render the `△` glyph; set `active-warning` on focus; import `Warnings`.
- `app/ui/components/verdict_badge.slint` — M: low-confidence label on `VerdictBadge` (+ `show-confidence-label`) and `VerdictBar`.
- `app/ui/screens/study_screen.slint` — M: focus "Signalement" caption + §4 study-level warning; import `Warnings`.
- `_bmad-output/implementation-artifacts/sprint-status.yaml` — M: story 2-7 → in-progress → review.

## Senior Developer Review (AI)

**Reviewer:** Guy · **Date:** 2026-06-14 · **Outcome:** Approve — Status → done (0 Critical / 0 High / 0 Medium)

### Scope verified

Adversarial review of the full File List against git reality and the live `core` API. File List ⇄ git match exactly (9 app source files + `sprint-status.yaml`); no undocumented changes, no claimed-but-absent files.

### AC validation (all IMPLEMENTED)

- **AC1** — `verdict_badge` sets `low_confidence`/`confidence_label` from `Verdict::low_confidence()`; rendered on both `VerdictBadge` (`show-confidence-label`) and the sticky `VerdictBar`, independent of the Provisional/Withheld split. Neutral ink. Verified `engine.rs:566`, `verdict_badge.slint:99,133`.
- **AC2** — Per-cell `△` glyph driven by `GridCellState.warning`; both finding sets mapped to UI addresses via `plausibility`/`resolve_anchor`. **Mapping cross-checked against the real emitters** (`normalize/checks.rs`, `splits.rs`, `ssg/{valuation,management,risk_reward}.rs`): every `context` string core emits resolves correctly — cell contexts → §2/§3 field, fiscal metadata → year anchor, year-less (`current_pe`/`forecast_low`) → study anchor. No emitted context is dropped or mis-attached.
- **AC3** — `warning` is a channel independent of `review`/`coverage`/`stale`; test `a_warned_validated_cell_carries_both_channels_independently` confirms a `✓`-validated cell also carries `warning == true`. `△` is shape- and position-distinct (top, x≈26px) from the trust marker (x:0, ≤22px), the right-edge coverage/stale markers, and the left stale dot — no geometric collision.
- **AC4** — The 6 microcopy strings live as `@tr()` literals in the `Warnings` global and `CONFIDENCE_LOW` is registered in `engine::USER_FACING_LABELS` (count 21→22); both scanned by the posture gate. Posture tests green.
- **AC5** — Focus caption (`Studies.active-warning` / `section4-warning-key`) is keyboard-reachable; the `△` is a static glyph (no animation) → reduced-motion-safe by construction.
- **AC6** — `confidence_label` is `""` and `warning` channels are silent when ≥5 usable years and no findings; test `a_clean_five_year_study_is_silent_and_full_confidence` confirms.
- **AC7** — One `normalize` per frame; findings cloned off `CanonicalFinancials` before the `StudySnapshot::new` move (no second normalize). Pinned surfaces (`core/ contract/ persistence/ ingestion/ report/ docs/method/ .github/ rust-toolchain.toml deny.toml Cargo.lock`) re-diff **empty** — verified via `git status`.
- **AC8** — Re-ran all four gates locally `--locked`: `cargo fmt --all --check` ✓, `cargo clippy --all-targets --all-features --locked -- -D warnings` ✓, `cargo test --all --locked` ✓ (app crate **102** passed, matching the claim), `cargo deny check` ✓. Interpretations issue **#24** confirmed OPEN with the correct title.

### Task audit

All 7 tasks marked `[x]` verified genuinely done against the code (no false completions).

### Observations (LOW — no action; already covered by the documented design in issue #24)

- `cell_key` / `study_key` surface the **first** finding per cell / per study-level anchor; if two findings collide on one address, only one fact shows. This is a deliberate, issue-#24-documented "first warning key" choice (the glyph is a binary attention pointer, the fact is revealed on focus), not a defect. No change made — overriding a tracked design decision under the guise of a fix would be wrong.
- The focus caption (`active-warning`) is not cleared on cell blur, mirroring the established `active-source` behaviour from Story 2.4; consistent, reset on study open. No change.

No HIGH/MEDIUM/CRITICAL findings → no auto-fixes applied. Story approved.

## Change Log

| Date       | Version | Description                                                                                  | Author |
| ---------- | ------- | -------------------------------------------------------------------------------------------- | ------ |
| 2026-06-14 | 0.1     | Story 2.7 implemented: low-confidence label on the verdict + per-cell plausibility glyphs (app-only surfacing; engine consumed, never modified). Interpretations → issue #24. All gates green; pinned surfaces re-diff empty. Status → review. | Amelia (dev agent) |
| 2026-06-14 | 0.2     | Adversarial code review: File List ⇄ git verified; all 8 ACs verified IMPLEMENTED (context→field map cross-checked against the live core emitters); all 7 tasks audited done; 4 gates re-run green (app 102); pinned surfaces re-diff empty; issue #24 confirmed. 0 Critical/High/Medium. Status → done. | Guy (AI review) |
