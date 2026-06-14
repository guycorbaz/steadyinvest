# Story 2.8: Interactive growth chart — draggable judgment line, live recolor

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As Guy,
I want to drag the judgment trend line on the §1 semi-log growth chart and watch the §4 zones recolour live (or set the same judgment by exact value, kept in sync),
so that the judgment moment is direct, fast and reversible — the signature interaction that turns a spreadsheet into *forged conviction*.

## Acceptance Criteria

(From epics.md §Story 2.8, lines 643–655. BDD, verbatim intent.)

1. **Given** the §1 semi-log growth chart (Sales/EPS/Price, **solid historical / dashed projection**, **5–30 % guide fan**, **1→200 log axis**) rendered **natively in Slint** (`Path` + `TouchArea`), **when** the screen opens, **then** the chart plots all materialized study years from the engine's data and the judgment trend line(s) appear at the user's **previously-set** value (or unset, awaiting input) — **never auto-placed or suggested** (FR33).
2. **Given** the chart, **when** I **drag** a judgment trend line, **then** the affected judgment input (estimated future Sales/EPS — see Dev Notes mapping) updates, the §4 forecast/zones recompute through `core`, and the **§4 zone bar recolours within ~100 ms** under my hand (NFR-P1, FR30, FR31).
3. **Given** the same judgment, **when** I instead **type its exact value** in the existing §1 `JudgmentField`, **then** the drag line and the exact-value field stay **in sync** (both read/write the same `Study.judgment` field — same `core` function) — gesture for intuition, value for rigor (FR31, NFR-U2).
4. **Given** a load-bearing input is **not validated-and-fresh** or the study is **low-confidence**, **when** the zones recolour on a drag, **then** the recolour respects **verdict integrity** — full saturated bands only for a `full` verdict; `provisional` desaturates + hatches; `withheld` spends no colour (FR12, the §4 `sat` gate from Story 2.6). The drag never paints full colour beside a non-green input.
5. **Given** a drag is in progress, **then** the chart holds **constant geometry — no re-layout / jank** (only colour/alpha + line-position tokens move, never metric/typo tokens), and the live zone recolour uses **a single smooth easing, never a flash**, **disabled when OS reduced-motion is requested** (UX-DR22, UX-DR27, `Studies.reduced-motion`).
6. **Given** the drag ends (pointer up / commit), **then** the new judgment value **persists** to `Study.judgment` via the existing mutation rail (`put_study`) and round-trips on reopen; **moving the line never destroys a saved input** (no silent blank/`0`; unknown → em-dash). *(Undo/redo of the move itself is Story 2.9 — not here.)*
7. **Given** Slint accessibility, **then** the §1 chart is **not mouse-only**: every draggable judgment line has the keyboard/exact-value path (AC 3) and Slint `accessible-*` properties; decision meaning is never colour-only (the §4 redundant encoding already satisfies this).
8. **Given** Epic 1's Spike B was **GO** (it was — measured 40–60 µs typical / 235 µs max, 400×–2500× under budget; `docs/spikes/spike-b-native-slint-chart.md`), **then** the production chart uses the **native-Slint `Path`+`TouchArea`** approach. The throwaway spike (`app/examples/spike_b_chart.rs` + its `justfile` recipe) is **removed** once the production chart lands. *(The agreed fallback — dedicated Slint canvas, or `plotters`→`SharedPixelBuffer` + `TouchArea` overlay — is **not** needed; never egui, never web.)*
9. **Given** the Definition of Done for a UI story, **then** the binary **launches and runs the event loop**, the headless-provable logic is unit-tested (coordinate↔value mapping round-trip, series→`Path`-command generation, live-recompute coherence, persist-on-commit round-trip), and the in-GUI drag click-through is recorded as a **documented partial** (human / AT-SPI deferral, identical to Stories 2.1–2.7 in this sandbox). 4 CI gates green `--locked`.

## Tasks / Subtasks

- [x] **Task 1 — Expose the per-year historical series for plotting (engine, app crate)** (AC: 1)
  - [x] In `app/src/viewmodel/engine.rs`, extend `StudyFrame` with the per-year series the chart needs (`year`, `sales`, `eps`, `high_price`, `low_price` per `CanonicalYear`). **Clone them off `canonical` BEFORE the `StudySnapshot::new(&canonical, …)` move** — exactly the pattern Story 2.7 used for `plausibility`. **Do NOT call `normalize` a second time** (a second pass drifts the series-frame from the verdict-frame — the coherence invariant).
  - [x] These series cross into `core` already as `CanonicalYear { year, sales, eps, high_price, low_price }` (`core/src/normalize/types.rs:154`) — all `Option<Decimal>`. No `core` change; `core/` stays a pinned surface.
- [x] **Task 2 — `growth_chart` adapter: series → Slint `Path` commands + axis/fan (app crate)** (AC: 1, 4, 5)
  - [x] Add `engine::growth_chart(frame: &StudyFrame, study: &Study, …) -> GrowthChartState` producing **pre-built `Path` `commands` strings** (`"M x y L x y …"`) for Sales, EPS, and Price, plus the **dashed projection** segments, the **5–30 % guide fan**, the **1→200 semi-log axis** ticks, and the judgment-line pixel position(s). Reuse the spike's proven helpers as the reference: `y_for(value)` (1→200 `log10` map), `value_for_y(y)` (inverse, used on drag), `path_commands(series)` (see `app/examples/spike_b_chart.rs:123–161` before you delete it).
  - [x] **Pixel mapping uses `f32`/`f64` — that is rendering, NOT the decision chain** (the spike's explicit, GO'd rule). The series *values* arrive as `Decimal` and are converted to `f64` **only** for the `log10` pixel map. No money/ratio decision math in the app.
  - [x] Plot **all materialized years** via `viewmodel::form::materialized_year_numbers(study)` — **no fixed row/year cap** (issue #20 removed `PE_TABLE_ROWS`; do not reintroduce a `[0,5)` window).
  - [x] Add the `GrowthChartState` struct to `app/ui/state.slint` and re-export it in `app/ui/app.slint`. Cross **only formatted strings / floats / bools** — never a `Decimal` or domain enum.
- [x] **Task 3 — `GrowthChart` Slint component (native Path + TouchArea)** (AC: 1, 5, 7)
  - [x] New `app/ui/components/growth_chart.slint` exporting `component GrowthChart inherits Rectangle`. Render Sales/EPS/Price as `Path` elements (commands bound to `GrowthChartState`), historical **solid** / projection **dashed**, the guide fan, the year + 1→200 axes, and the draggable judgment line(s) as a 2 px line + **visible grip handle** with a **generous hit target (~±8–10 px)** wider than the drawn line.
  - [x] **Colour budget:** §1 carries **NO zone hues** (UX-DR10). Series + axis + fan are **ink-scale only** (distinguish Sales/EPS/Price by stroke weight + dash style, not colour). **Do NOT copy the spike's `#6da3ff` blue EPS line** — that was a throwaway shortcut outside the palette. Add the needed neutral tokens to `app/ui/tokens.slint` (e.g. chart series inks from the existing `text-high/mid/low` scale, a guide-fan ink, a judgment-line/handle ink). No hard-coded hex/px — read `Tokens`.
  - [x] States: idle, hover-handle (brighten handle, grab cursor), dragging (grabbing cursor), low-confidence overlay. Add Slint `accessible-role`/`accessible-label` on the interactive line; keep an always-visible focus affordance (NFR-U2).
  - [x] Slint hygiene (recorded traps): components `PascalCase`, file `snake_case`, properties/callbacks **`kebab-case`** (`judgment-moved`, `series-commands`). **Do NOT name any input `z` or `row`** (reserved attached properties — 2.6 had to rename a `ZoneBar` input). `@children` inside a conditional is illegal; element-ids are unreachable from a component-root function inside a conditional.
- [x] **Task 4 — Live drag → core recompute → §4 recolour, under ~100 ms (app crate)** (AC: 2, 4, 5)
  - [x] Mount `GrowthChart` in `app/ui/screens/study_screen.slint` §1, **replacing the `PlaceholderRegion { caption: @tr("Graphique disponible prochainement"); }`** at the §1 chart slot. Leave the existing §1 `JudgmentField`s (sales_growth / eps_growth / est_high_eps / est_low_eps) and §4 `ZoneBar` in place — the chart drives them, it does not replace them.
  - [x] Add a `judgment-moved(string, length)` callback on the `Studies` global (field-key, pixel-y). In `main.rs`, wire it: clamp y → `value_for_y` → the new judgment value → **on a non-persisted in-memory `Study` clone**, call `engine::build_frame` ONCE, and push **only** the affected live properties (`zone-bar`, `verdict`, the moved judgment-field display, the chart line position). **Persist only on drag-end / commit.**
  - [x] **CRITICAL latency trap:** do **NOT** route each `moved` event through the full `push_form` + `put_study` path (Story 2.6's `main.rs::push_form`, lines 119–189, rebuilds every row and writes the journal — far too heavy per `moved`). The spike proved the *recompute itself* is sub-millisecond; the cost to avoid is per-event journal writes + full-form rebuilds. Keep the live frame lightweight; commit once.
  - [x] **Cardinal Rule:** the zone/forecast/verdict recompute happens in `core` via `build_frame` — **never** recompute a zone or P/E inside a Slint callback or the app crate. The drag handler only maps pixels↔value and pushes the resulting snapshot's pre-formatted outputs.
  - [x] **One coherence frame:** zone-bar + verdict on each live frame must derive from the **same** `StudySnapshot` (so `confidence`/`sat` and the bands never disagree mid-drag). `build_frame` already guarantees a single `normalize`.
- [x] **Task 5 — Gesture ↔ exact-value sync + persist-on-commit (app crate)** (AC: 3, 6)
  - [x] The draggable line and the matching §1 `JudgmentField` write the **same** `Study.judgment` field through the **same** `set_judgment` / `state::set_judgment_field` rail — sync is by construction, no second source of truth (FR31).
  - [x] On drag-end, persist via the existing rail (read-only / no-journal / save-failure guards + neutral `MSG_*`, re-read study, `Journal::put_study`). The `JudgmentField` re-seeds from the model on `changed value` **only `if (!input.has-focus)`** — so an in-progress typed edit is never clobbered by a drag and vice-versa (the 2.4/2.6 keep-input-on-refusal pattern).
  - [x] **Soft-lock symmetry (2.5 MEDIUM trap):** if a drag would write to a judgment input that is soft-locked (`✓`), honour the same `MSG_SOFT_LOCKED` backstop as `edit_cell`/`set_not_available` — never silently demote a `✓`. (Judgment-input gate today: `None`→`Missing`, `Some`→`ValidatedFresh`; confirm whether judgment fields participate in soft-lock before assuming they do — see Q1.)
  - [x] Unknown/cleared → `None` → em-dash, **never `0`** (the single most-repeated rail).
- [x] **Task 6 — Remove the throwaway spike + keep CI green** (AC: 8)
  - [x] Delete `app/examples/spike_b_chart.rs` and the `spike-b` recipe in `justfile` (lines 28–29). `clippy --all-targets` lints examples, so the example must be gone (not left rotting). Confirm `app`'s `rust_decimal` dependency is still needed by production code after removal (it is a runtime dep since Story 2.4) — do **not** drop a still-used dependency; **`Cargo.lock` + `deny.toml` must re-diff byte-identical** (no new dependency expected).
  - [x] Keep `docs/spikes/spike-b-native-slint-chart.md` (the GO findings record) — `docs/method/` is pinned but `docs/spikes/` is the spike's home; do not delete the evidence.
- [x] **Task 7 — Tests, posture floors & DoD honesty** (AC: 1, 4, 6, 9)
  - [x] Headless `#[test]`s (pure Rust, no `slint::test`, sentence-case messages): `y_for`/`value_for_y` round-trip + clamp at axis bounds (1 and 200); `path_commands` / `growth_chart` series→commands for a known study; live-recompute coherence (a simulated drag value yields a snapshot whose zone-bar `confidence` matches its verdict); persist-on-commit round-trip (drag value → `put_study` → reopen restores it); verdict-integrity gate (a non-fresh load-bearing input ⇒ `provisional`/`withheld`, never `full`, on the dragged frame).
  - [x] Bump the `posture.rs` floors to the **actual** post-2.8 counts (current floors: `.slint` files ≥ 18, `@tr()` literals ≥ 130, `USER_FACING_MESSAGES` = 14, engine labels = 22). New `@tr` strings (axis/fan captions, chart a11y labels) and any new component file move the counts up — set the floor to the new true minimum so a future broken scan still trips. Any new user-visible string passes the banned-verb scan (zone/method nouns are exempt; **the chart must never carry a "suggested"/"optimal"/buy-sell-hold imperative** — FR13/FR33).
  - [x] Update the §1 `CollapsibleSection` `fold-summary` if the chart changes what the collapsed summary should state (today `@tr("Croissance BPA est. — · ventes —")`).
  - [x] DoD: launch the binary (event loop runs); record the in-GUI drag click-through as a documented partial (human/AT-SPI) exactly as 2.1–2.7 did. **Do not mark a `[x]` for a test that does not exist** (2.6 MEDIUM review fix; File List ⇄ git must match exactly — issue #18).

### Review Findings (adversarial code review, 2026-06-14)

Blind Hunter + Edge Case Hunter + Acceptance Auditor (no layer failed). 0 decision-needed · 5 patch · 7 deferred · 4 dismissed.

**Patch (unchecked → to fix):**
- [x] [Review][Patch] Drag commit persists on a zero-movement click / rejected drag-start — a single click on the strip silently rewrites the forecast; guard commit on an actual `moved` [app/src/main.rs on_judgment_commit] *(High)*
- [x] [Review][Patch] Refused/failed commit (read-only / save error) leaves a phantom un-saved line — the `Err` branch never reconciles the preview to disk (contradicts its own doc-comment) [app/src/main.rs on_judgment_commit Err] *(High)*
- [x] [Review][Patch] Pointer `cancel` persists the release value instead of reverting the gesture [app/ui/components/growth_chart.slint + main.rs] *(Medium)*
- [x] [Review][Patch] §4/§5 judgment-dependent numbers (forecast high/low, U/D, projected return, §4 warning) are NOT refreshed on the live drag frame — the bar recolours while the numbers beside it stay frozen [app/src/main.rs push_live_preview] *(Medium)*
- [x] [Review][Patch] `judgment-dragging` flag can leak (form stuck unscrollable) if the study is closed mid-drag — defensively reset on study open/close [app/src/main.rs on_open_study + app/ui/app.slint] *(Medium)*

**Deferred (real, not now):**
- [x] [Review][Defer] Dragging converts a growth-%-derived forecast into a direct `est_high_eps` (spec Q2 chose this handle) — record as interpretation [chart.rs/main.rs] — deferred, by spec decision
- [x] [Review][Defer] 1→200 axis can't represent sub-$1 or >$200 EPS; a drag can snap/destroy such a typed forecast — strengthens deferred axis-scaling item #1 [chart.rs value_for_y] — deferred, axis-scaling refinement
- [x] [Review][Defer] Drag strip announces `accessible-role: slider` with no keyboard step handler (keyboard path exists via the exact-value field) [growth_chart.slint] — deferred, a11y refinement
- [x] [Review][Defer] All-`None`-EPS series + a set forecast → orphan grip/label with no anchored line (rare degenerate) [chart.rs/growth_chart.slint] — deferred, rare
- [x] [Review][Defer] Single-point (1-year) series renders nothing (no isolated-point marker) [chart.rs path_commands] — deferred, rare
- [x] [Review][Defer] Grip handle centered in the strip vs the trend-line endpoint at the plot edge — cosmetic offset; `judgment_x` exported but unused [growth_chart.slint/chart.rs] — deferred, GUI polish (post-MVP)
- [x] [Review][Defer] mouse-y→viewbox-y 1:1 mapping silently depends on the fixed plot height; add a guard/test when the chart becomes responsive [chart.rs/growth_chart.slint] — deferred, latent risk

**Dismissed (noise/handled):** commit re-snaps mouse-y (negligible, 2 dp); commit clears cache before guards (benign, restructured by the commit patch); switch-study-mid-drag (not mouse-triggerable; covered by the flag-reset patch); `span<=0` dead branch (unreachable, `FORECAST_HORIZON_YEARS > 0`).

## Dev Notes

### The two distinct surfaces — read this first (locked, stated twice in architecture/UX)

The story title conflates two **physically separate** components. The architecture keeps them distinct:

- **§1 growth chart** (`growth_chart.slint`, NEW this story): semi-log Sales/EPS/Price, solid historical / dashed projection, 5–30 % guide fan, 1→200 log axis, **draggable trend lines, NO zones** (UX-DR10).
- **§4 zone bar** (`zone_bar.slint`, **already built in Story 2.6**): the vertical Buy/Hold/Sell thirds + price axis + present-price marker + the saturation/verdict gate. **This is the recolour target.** You are *not* rebuilding it — the §1 drag feeds new judgment values through `core`, which produces a fresh `ZoneBarState`, which recolours the existing bar.

Data flow of the signature gesture (architecture, Journey 1 sub-flow):

```
Grab §1 trend line (or type exact value)
  → estimated future Sales/EPS update (Study.judgment field)
  → core::build_frame → normalize → StudySnapshot::new (ONCE)
  → §4 forecast high/low recompute (risk_reward.rs)
  → §4 ZoneBar recolours live  (<100 ms)  + U/D ratio · projected return · verdict badge update
  → (explore more, or) pointer-up → judgment persists (put_study); undo lives in Story 2.9
```

### The drag→judgment-input mapping (confirm against core before wiring — see Q2)

The §1 chart's draggable lines correspond to the **future-projection judgment inputs** that already exist as exact-value `JudgmentField`s in §1: `sales_growth`, `eps_growth`, `est_high_eps`, `est_low_eps`. These map to `core::ssg::JudgmentInputs` (`core/src/ssg/types.rs:31`):

| §1 draggable line / field | `JudgmentInputs` field | Feeds |
|---|---|---|
| EPS future / `est_high_eps` | `estimated_high_eps` | `forecast_high = judged_avg_high_pe × estimated_high_eps` → §4 **upper** zone bound (load-bearing for recolour) |
| EPS low / `est_low_eps` | `estimated_low_eps` | `forecast_low` (when option = avg_low_pe×eps) → §4 **lower** bound |
| Sales growth / `sales_growth` | `projected_sales_growth_pct` | §1 projected sales dashed line |
| EPS growth / `eps_growth` | `projected_eps_growth_pct` | §1 projected EPS dashed line |

The **load-bearing recolour driver** is `estimated_high_eps` (and `estimated_low_eps` for the lower bound), because `forecast_high`/`forecast_low` → `ZoneBounds` → the recolour. Make the EPS trend-line endpoint the primary draggable handle that moves `estimated_high_eps`; treat the growth-% lines as projection-shaping handles. **Confirm the exact field each visual line should write against `core::ssg::risk_reward.rs` and `growth.rs` — do not guess the geometry.**

`JudgmentInputs` full field list: `estimated_high_eps`, `estimated_low_eps`, `projected_sales_growth_pct`, `projected_eps_growth_pct`, `judged_avg_high_pe`, `judged_avg_low_pe`, `forecast_low_option`, `recent_severe_low`, `current_price`, `present_full_year_dividend` — all `Option<Decimal>`.

### Files to open / touch (all in the `app` crate — `core`/`contract`/`persistence` stay pinned)

- `app/examples/spike_b_chart.rs` — **the GO'd reference; read it, port `y_for`/`value_for_y`/`path_commands`/the `Path`+`TouchArea` structure, then DELETE it** (Task 6). The spike's `recompute()` did Decimal math inline — production routes that through `core` instead (Cardinal Rule).
- `app/src/viewmodel/engine.rs` — `StudyFrame` (line 201, extend it), `build_frame` (213), `zone_bar` (496), `verdict_badge` (540), `judgment_fields` (668), `to_judgment_inputs` (117). Add `growth_chart(...)`.
- `app/src/viewmodel/form.rs` — `materialized_year_numbers`, `EMPTY_SLOT`, `year_headers`.
- `app/src/viewmodel/format.rs` — `format_scaled(value, DisplayField, NumberFormat)` (the only rounding rail; `DisplayField::{Price, PerShare, Percent, …}`).
- `app/src/state.rs` — `set_judgment_field`, `snapshot_for`, `MSG_NORMALIZE_FAILED`, `MSG_SOFT_LOCKED`, `USER_FACING_MESSAGES` (persistence/mutation rail; add a live non-persisted recompute helper here or in viewmodel).
- `app/src/main.rs` — `push_form` (119–189, the per-frame full rebuild — your live drag must NOT use this per `moved`), `on_set_judgment` (~696). Wire the new `judgment-moved` callback here.
- `app/src/theme.rs` — `Palette` (dark+light ink scales; add chart series/axis/fan/handle inks here so a theme swap repaints the chart via the shared `arc_swap` token snapshot).
- `app/src/posture.rs` — bump banned-verb-scan floors (Task 7).
- `app/ui/screens/study_screen.slint` — §1 `PlaceholderRegion` at ~line 471 (replace), §1 `JudgmentField`s ~482–506 (keep), §4 `ZoneBar { bar: Studies.zone-bar; }` ~line 696 (keep, it recolours).
- `app/ui/components/zone_bar.slint` — **read, do not rebuild** (the recolour target; its `sat` gate enforces verdict integrity already).
- `app/ui/components/judgment_field.slint` — the commit + `!has-focus` re-seed discipline the drag must mirror.
- `app/ui/components/growth_chart.slint` — **NEW**.
- `app/ui/state.slint` — add `GrowthChartState`; `Studies` global (add `judgment-moved`); structs `JudgmentFields`/`ZoneBarState`/`VerdictState` already there.
- `app/ui/app.slint` — re-export the new struct.
- `app/ui/tokens.slint` — add neutral chart tokens (no zone hues on §1).
- `justfile` — remove the `spike-b` recipe.

### Established conventions (carry forward)

- **Cardinal Rule:** every calculation in `steadyinvest-core`. Forbidden anti-pattern: a P/E or zone recomputed inside a Slint callback / the app crate. Money crosses into `.slint` as **already-formatted, locale-aware strings** via `viewmodel/`; never an `f32`/`f64`/`Decimal`/domain enum (enums cross as stable strings: `"buy"`, `"full"`). **Exception that is allowed:** pixel coordinates for plotting are `f32`/`f64` — rendering ≠ decision chain (spike-proven).
- **No `.unwrap()`/`.expect()`** in non-test code (except a documented `// INVARIANT:`); **no silent `.ok()`** (the prior project shipped a blank chart that way — and DoD is literally "launch the app and look"). Propagate `NormalizeError` to a neutral `MSG_*`.
- Time/IDs only via the injected `Clock`/`IdGen` (`app/src/clock.rs`) — no scattered `Utc::now()`/`Uuid::new_v4`.
- **Colour budget:** saturated colour is spent ONLY on the three Okabe-Ito zone hues in the §4 bar (`zone-buy #009E73`, `zone-hold #E69F00`, `zone-sell #D55E00`, per-theme `zone-alpha` dark 0.36 / light 0.165). The lone geofenced exception is the `✓` `validated-ink #4A7C6F`, which never co-presents with zone bands. **The §1 chart is greyscale ink + line-style only.**
- **Reduced motion:** `Studies.reduced-motion` (default `false`; not yet wired to the OS flag — issue #22, out of scope here but your easing must gate on the token).
- **Dead-code-under-clippy:** `clippy --all-targets` builds the binary without `cfg(test)` — any new adapter fn must be reached from the production callback/`push_form` path, or it fails `-D warnings` as dead code. Wire it in.

### Verdict-integrity recolour rule (FR12 — do not break it)

The §4 `ZoneBar` already gates saturation by `confidence`: `full → sat 1.0`; `provisional → sat 0.45 + "╱╱╱╱" hatch`; `withheld/!available → calm empty state, no band`. Your live drag pushes a fresh `ZoneBarState` whose `confidence = verdict_state(snapshot.verdict())` every frame, so the recolour respects integrity for free **as long as you push the verdict from the same snapshot**. Never paint a full band during a Provisional/Withheld drag. The engine partition is structural: `Full ⟺ all load-bearing gates ValidatedFresh ∧ ¬low_confidence`; `Withheld ⟺ ≥1 gate Missing`; `Provisional` otherwise.

### Recorded dev traps this story must avoid (from 2.4–2.7 reviews / GitHub issues)

1. **Per-`moved` full rebuild** → misses NFR-P1. Live path is lightweight; persist on commit (Task 4). *The central trap of this story.*
2. **Second `normalize`** for the chart series → frame drift. Clone series off `canonical` before the `StudySnapshot::new` move, like 2.7 did for `plausibility` (Task 1).
3. **Full colour beside a non-green input** (FR12 disaster) → honour the `sat`/`confidence` gate (above).
4. **`Withheld` carrying a provenance caption** → `provenance_date` is emitted only for `Verdict::Provisional` (2.6 MEDIUM fix). Don't regress it via a new live-push path.
5. **Soft-lock backstop missing on a new write path** (2.5 MEDIUM) → a drag that writes a `✓` input needs the `MSG_SOFT_LOCKED` guard (Task 5 / Q1).
6. **`PE_TABLE_ROWS` / fixed 5-year cap** → removed (issue #20); plot all materialized years.
7. **Overclaimed `[x]` tests / File-List drift** (2.6 MEDIUM, issue #18) → claimed tests must exist; File List ⇄ git exact.
8. **Slint reserved `z`/`row` input names, `@children` in a conditional, ids unreachable in a conditional fn, cross-component callable not `public pure function`** — all real compile traps hit in 2.2–2.7.
9. **`editable_cell.slint` already owns Ctrl/Cmd chords** (Ctrl+Space, Ctrl+Enter, Ctrl+Backspace, Ctrl+V) — if you add a keyboard nudge for the line, don't collide.

### Project Structure Notes

- All work is in `steadyinvest-app`. **No `core`/`contract`/`ingestion`/`persistence`/`report` change is expected** — the chart series already exist as public `CanonicalYear` fields and the judgment inputs as public `JudgmentInputs` fields. Those crates, plus `docs/method/`, `.github/`, `rust-toolchain.toml`, `Cargo.lock`, `deny.toml`, and the frozen `persistence/tests/corpus/v1.db`, are **pinned surfaces — re-diff them byte-empty** before finishing. If you find you *must* touch `core`, stop and reconsider (it likely means decision math leaked into the app).
- **No new dependency.** `Cargo.lock` + `deny.toml` re-diff identical. `slint-build@1.16` already builds the UI; `rust_decimal` (with `maths`) is already an `app` runtime dep.
- Slint/Rust naming: components `PascalCase`; `.slint` files `snake_case`; properties/callbacks `kebab-case`; Rust↔Slint callbacks `verb-noun` (e.g. `judgment-moved`); types `PascalCase`, fns/modules `snake_case`, consts `SCREAMING_SNAKE_CASE`; one module = one file, organized by domain (no `utils.rs`).

### Tech stack (pinned, verified)

- **Rust** workspace MSRV **1.96** (`rust-toolchain.toml`); **Slint 1.16.1** (charts native via `Path`+`TouchArea`, `log10` in Rust; recolour trivial in Slint's dirty-driven retained mode); `rust_decimal 1.42` (+`maths`); `rusqlite 0.40` (`bundled`). Linux-only dev/CI for now (the `<100 ms` perceptual verdict was made by Guy on real hardware in Spike B; headless sandbox cannot screenshot/AT-SPI — plan the same documented-partial honesty).
- 4 CI gates, all `--locked`: `cargo fmt --all --check` · `cargo clippy --all-targets --all-features --locked -- -D warnings` · `cargo test --all --locked` · `cargo deny check`. Current app `#[test]` count: **102** (you add to it).

### Spike B evidence (de-risks AC 2 / AC 8)

Spike B (Story 1.5) prototyped *exactly this* and is **GO**. Measured on Guy's display 2026-06-09 across 660 drag events: recompute + property-set **~40–60 µs typical, 235 µs max** — 400×–2500× under the 100 ms NFR-P1 budget; Slint's retained-mode repaint made recolour visually instant ("suit mon geste instantanément, aucune perception de délai"). The latency risk is therefore the *per-event journal write + full-form rebuild*, not the recompute — which is why Task 4 keeps the live frame lightweight and commits once. The fallback rendering path is **not needed**.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 2.8] (lines 643–655: BDD ACs); #Epic 2 (271–277, 298–309, 534–536: ordering, dependency on Spike B & Story 2.6).
- [Source: _bmad-output/planning-artifacts/prd.md] FR30/FR31/FR32/FR33 (719–724), FR6/FR12/FR13/FR29; NFR-P1 ~100 ms (837–842), NFR-U1/U2/U3 (876–882), NFR-C1/C3, NFR-X1; Technical constraint (893–896: native Slint, egui removed, fallback defined).
- [Source: _bmad-output/planning-artifacts/architecture.md] Core Technical Decisions (charts native Slint, `arc_swap` token source, Cardinal Rule, viewmodel adapter, `judgment-moved` callback example); workspace crate layout; file map `app/ui/components/{growth_chart,zone_bar}.slint`, `app/src/state.rs`.
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md] UX-DR2/DR3 (Okabe-Ito hues, alpha, redundant encoding); UX-DR10 (§1 chart: draggable trend lines, NO zones, 5–30 % fan, 1→200 axis); UX-DR11 (§4 zone bar: live <100 ms recolour, full/muted/provisional); UX-DR22 (constant geometry, no jank); UX-DR23 (verdict-integrity colour rule); UX-DR27 (reduced-motion/font-scale); ERRATUM 2026-06-09 (egui removed → native Slint).
- [Source: app/examples/spike_b_chart.rs] `y_for`/`value_for_y`/`path_commands`/`recompute`/`TouchArea moved` — port then delete.
- [Source: docs/spikes/spike-b-native-slint-chart.md] GO findings + latency numbers.
- [Source: _bmad-output/implementation-artifacts/2-6-numeric-judgment-inputs-verdict-zone-bar.md] `build_frame`/`zone_bar`/`verdict_badge` wiring; the §4 `sat`/`confidence` gate; `push_form` per-frame shape.
- [Source: _bmad-output/implementation-artifacts/2-7-low-confidence-plausibility-surfacing.md] `StudyFrame` clone-before-move pattern; low-confidence on the verdict surface.
- [Source: core/src/ssg/types.rs:31] `JudgmentInputs`; :342 `ZoneBounds`; :351 `RiskRewardOutputs`; :258 `GrowthOutputs`. [core/src/normalize/types.rs:154] `CanonicalYear`.
- GitHub issues (repo `guycorbaz/steadyinvest`) — tracking source of truth: #18 (File-List⇄git), #20 (row cap removed), #22 (OS reduced-motion wiring), #24 (first-warning-per-cell).

## Open Questions (for Guy / dev — non-blocking, default chosen)

- **Q1 — Soft-lock on judgment inputs:** Today the judgment-input gate is `None→Missing`, `Some→ValidatedFresh` (a typed judgment is validated-by-entry, issue #23). It's unclear whether §1 judgment fields participate in the `✓` tri-state soft-lock at all (that was a §2/§3 *cell* concept in Story 2.5). **Default:** treat judgment fields as *not* soft-locked (drag freely writes them); add the `MSG_SOFT_LOCKED` backstop only if they do. Confirm in `state.rs`.
- **Q2 — Which line writes which field:** the exact visual-line → `JudgmentInputs`-field geometry (especially whether the EPS trend handle writes `estimated_high_eps` directly vs. `projected_eps_growth_pct`). **Default:** the load-bearing EPS handle writes `estimated_high_eps`/`estimated_low_eps` (the direct §4 forecast drivers); confirm against `core/src/ssg/risk_reward.rs`.
- **Q3 — Number of draggable lines in v1:** UX implies several (sales growth, EPS growth, forecast high/low P/E). **Default for 2.8:** ship the **EPS forecast line** (the one that recolours §4) as the primary draggable handle, with the existing exact-value fields covering the rest; defer extra draggable handles if they add risk. Confirm scope appetite.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8[1m] (Opus 4.8, 1M context)

### Debug Log References

- 4 CI gates run locally, all green `--locked`:
  - `cargo fmt --all --check` ✓ (one auto-format pass applied to `chart.rs`)
  - `cargo clippy --all-targets --all-features --locked -- -D warnings` ✓ (zero warnings; `--all-targets` confirms no orphaned reference to the deleted spike example)
  - `cargo test --all --locked` ✓ — app crate **110** `#[test]` (was 102; +8 chart), all other crates unchanged & green
  - `cargo deny check` ✓ (advisories, bans, licenses, sources ok)
- Posture floor failed once (over-bumped `@tr` floor to 146; actual extracted count is **140** = prior 138 + 2 new chart strings) → corrected to 140, re-green.
- Event-loop smoke test: `timeout 12 cargo run -p steadyinvest-app` against the real display (`DISPLAY=:0`, Wayland) → exit 124 (ran until the timeout kill), i.e. the window built and the event loop ran without panicking — the new §1 chart, the `GrowthChartState` struct, the `Path`/`TouchArea` rendering and the zone-bar `animate` all loaded and rendered.

### Completion Notes List

- **Open questions resolved from the code (all defaults confirmed):**
  - **Q2** — `core/src/ssg/risk_reward.rs` + `growth.rs` confirm `forecast_high = judged_avg_high_pe × estimated_high_eps`, where `estimated_high_eps` is the user's direct value *or* the value derived from `projected_eps_growth_pct`. So the §1 draggable line writes **`est_high_eps`** (the load-bearing §4-forecast driver) — exactly the spike's mapping.
  - **Q1** — judgment fields are NOT cells and do not participate in the §2/§3 `✓` soft-lock (gate is `None→Missing`, `Some→ValidatedFresh`); the drag writes them freely, no `MSG_SOFT_LOCKED` backstop needed.
  - **Q3** — shipped the **EPS forecast line** as the single draggable handle (the one that recolours §4); the other judgment inputs remain exact-value `JudgmentField`s. Extra draggable handles deferred.
- **Architecture honoured:** the live drag recomputes through ONE `engine::build_frame` on an in-memory (un-saved) `Study` clone cached at pointer-down (`drag_study`), pushing only the judgment fields + §1 chart + §4 zone bar + verdict — **never** the per-event `put_study` + full `push_form` (Task 4's central latency trap). Persistence happens **once** on pointer-up via the existing `set_judgment_field` rail, so gesture and exact-value share one source of truth (FR31). Cardinal Rule kept: all zone/forecast/verdict math in `core`; the only floats in `app` are pixel coordinates (rendering ≠ decision chain). One coherent frame per recompute (single `normalize`, series cloned before the `StudySnapshot::new` move — the 2.7 invariant extended to the chart series).
- **Verdict integrity by construction (AC4):** the live recolour reuses the §4 `ZoneBar` `sat`/`confidence` gate (full ⟺ all load-bearing inputs ✓ & fresh; provisional hatches; withheld empty). Headless test `dragging_never_paints_full_colour_when_a_load_bearing_input_is_missing` proves a drag on a withheld study never paints full colour.
- **FR33 (never auto-place):** the line is drawn only when the engine has a forecast est-high-EPS (`judgment_y >= 0`); with neither a direct value nor a growth-% set, `judgment_y = -1` and no line renders. Test `growth_chart_never_auto_places_the_judgment_line`.
- **AC5 easing:** added an `animate background` on the `ZoneSegment` bands, `duration: Studies.reduced-motion ? 0ms : 150ms` — smooth recolor, never a flash, collapses to instant under OS reduced-motion.
- **Colour budget:** the §1 chart spends NO judgment hue — series separate by ink-scale lightness (EPS text-high, Price text-mid, Sales text-low) + stroke weight + a Rust-built dash for the EPS projection (Slint `Path` has no dash array). The spike's `#6da3ff` blue EPS line was NOT carried over.
- **Spike removed:** `app/examples/spike_b_chart.rs` + the `justfile` `spike-b` recipe deleted; `docs/spikes/spike-b-native-slint-chart.md` (the GO findings) kept. No dependency change — `rust_decimal`/`arboard` are already runtime deps; `Cargo.lock` + `deny.toml` re-diff byte-identical; `core`/`contract`/`persistence`/`ingestion`/`report` + `rust-toolchain.toml` all untouched (verified via `git status`).
- **Honest DoD — now with real manual verification:** the binary launches + runs the event loop, the headless-provable logic is fully unit-tested (axis map round-trip + clamp, `path_commands`, series→commands, anchored trend line, gesture→value→chart round-trip, live-recompute integrity gate, persist round-trip via the same rail), **and Guy drove the live drag on his own display (Wayland)** — the recolour follows the gesture (matching Spike B's 40–60 µs / 400×–2500×-under-NFR-P1 measurement), AAPL renders full-colour and CSCO provisional. The manual pass surfaced (and this commit fixes) the Flickable-stolen-drag, the trend-line anchoring, the chart size/contrast, and a pre-existing nav gap.

#### Recorded interpretations / deferred items (→ file as GitHub issues, per the project's tracking rule)

1. **Shared-axis series scaling (v1):** all three series map onto the 1→200 semi-log axis with clamping (the spike's proven approach). Sales in millions therefore pins to the top edge; the signature interaction (EPS forecast line + §4 recolour) is unaffected since EPS/Price are per-share and in range. Per-series decade auto-scaling (so each series' *slope* reads comparably) is a deferred refinement.
2. **Fixed-height / flexible-width chart (v1):** the plot box renders at a fixed `420 px` HEIGHT (so the drag's `mouse-y` maps 1:1 to the viewbox-y) while the width stretches to the section. Full responsive layout (and a larger/zoomable chart) is deferred.
3. **Single draggable handle (v1):** only the EPS-forecast trend line is draggable (Q3 default), via the right-edge strip; the sales-growth / EPS-growth / forecast-P/E judgments stay exact-value-only for now.
4. **GUI polish deferred (post-MVP, Guy's call 2026-06-14):** the chart UI is good enough for the MVP; visual/interaction refinement (drag-strip affordance discoverability, series legend, axis-per-series scaling, sizing) is intentionally deferred to keep the MVP moving.
5. **Nav gap (pre-existing, fixed opportunistically):** selecting "Études" now closes the open study and returns to the list (previously only the in-form "‹ Retour" did). Not introduced by 2.8; fixed here because it blocked switching studies during verification.

### File List

**New**
- `app/src/viewmodel/chart.rs` — the §1 chart adapter: `y_for`/`value_for_y`/`judgment_value_for_y`, `path_commands`, `x_for`, `growth_chart()` (anchored trend line + endpoint), `unavailable()`, + 8 unit tests.
- `app/ui/components/growth_chart.slint` — the native-Slint `GrowthChart` component (`Path` + `TouchArea`, ink-only, anchored trend line, right-edge drag strip + grip handle + a11y).

**Modified**
- `app/src/viewmodel/engine.rs` — `StudyFrame` gains `series: Vec<CanonicalYear>` (cloned before the `StudySnapshot::new` move); `CanonicalYear` import added.
- `app/src/viewmodel/mod.rs` — register `pub mod chart;`.
- `app/src/main.rs` — push `growth-chart` in `push_form` (both branches); new `push_live_preview` helper; `drag_study` cache; `on_judgment_drag_start` / `on_judgment_moved` / `on_judgment_commit` callbacks.
- `app/src/state.rs` — `apply_judgment_field` made `pub(crate)` (reused for the live in-memory preview).
- `app/src/posture.rs` — `@tr` floor 130→140, `.slint` file floor 18→19 (the new chart component + its strings).
- `app/ui/state.slint` — new `AxisTick` + `GrowthChartState` structs (`judgment-commands`/`judgment-x`/`judgment-y`); `Studies.growth-chart` + `Studies.judgment-dragging` properties; `judgment-drag-start` / `judgment-moved` / `judgment-commit` callbacks.
- `app/ui/app.slint` — re-export `AxisTick`, `GrowthChartState`; **nav fix** — selecting "Études" closes any open study (returns to the list).
- `app/ui/tokens.slint` — neutral §1 chart strokes/grip/fan-opacity tokens (no judgment hue).
- `app/ui/screens/study_screen.slint` — mount `GrowthChart` in §1 (replacing the placeholder); removed the now-dead `PlaceholderRegion`; the form `Flickable` yields (`interactive: !Studies.judgment-dragging`) so the drag is not stolen by the scroll.
- `app/ui/components/zone_bar.slint` — `animate background` on `ZoneSegment` (AC5 eased recolour, reduced-motion-gated).
- `justfile` — removed the `spike-b` recipe.

**Deleted**
- `app/examples/spike_b_chart.rs` — the throwaway Spike B (productionized here; GO findings doc retained).

### Change Log

| Date | Change |
|------|--------|
| 2026-06-14 | Story 2.8 implemented: native-Slint §1 interactive growth chart with a draggable est-high-EPS judgment line driving live <100 ms §4 zone recolour; live recompute on an un-saved study clone (persist once on release), gesture⇄exact-value sync, verdict-integrity-gated recolour, Spike B removed. 4 gates green `--locked`; app tests 102→110. Status → review. |
| 2026-06-14 | Manual-verification rework (Guy on his display): (1) the form `Flickable` was stealing the vertical drag → gated with `Studies.judgment-dragging` (scroll yields while the handle is held); (2) the judgment line is now a TREND line — origin FIXED at the last historical EPS, only the future ENDPOINT drags via a right-edge strip + grip (was a full-width horizontal line); (3) chart enlarged (height 320→420, width stretches) and gridlines made more contrasted; (4) nav fix — selecting "Études" returns to the studies list (a pre-existing gap, not from 2.8, surfaced while switching studies). All 4 gates re-green; app tests still 110. |
| 2026-06-14 | Adversarial code review (3 layers, 0 failed): 0 decision · 5 patch · 7 defer · 4 dismiss. All 5 patches applied — (P2) a zero-movement click on the drag strip no longer rewrites the forecast (commit gated on an actual `moved`); (P3) a refused/failed write reconciles the preview to disk instead of leaving a phantom line; (P4) `cancel` reverts instead of persisting (new `judgment-cancel` callback); (P1) §4/§5 numbers (forecast high/low, U/D, projected return, §4 warning) now refresh on the live drag frame; (P5) `judgment-dragging` defensively reset on study open + both close paths so a mid-drag teardown can't leave the form unscrollable. 4 gates re-green `--locked`; app tests 110. Status → done. |
