# Story 2.6: Numeric judgment inputs, verdict & zone bar (integrity-gated)

Status: done

<!-- Epic-2 story 6 — THE story that finally wires Epic 1's proven engine into the live UI. Until now the
     app only PARSES, RENDERS and PERSISTS the per-cell data-state model (2.3 structure, 2.4 entry+provenance,
     2.5 review tag + soft-lock); every computed slot (§2 ratios, §3 averages, D/E/G/H, §4 zones, §5 return,
     the verdict) has been a faithful em-dash placeholder. 2.6 makes the app CALL `core` for the first time:
     map `contract::Study` (years of `Cell`s + the `Judgment` snapshot) → `core::RawFinancials` → `normalize`
     → `CanonicalFinancials`; map `contract::Judgment` → `core::JudgmentInputs`; build `core::verdict::InputGates`
     from each usable year's cell review×freshness (2.5) + the judgment inputs; then `StudySnapshot::new(...)`
     ONCE per frame → read `outputs()` (§2/§3/§4/§5 results, zone bounds, U/D, projected return) + `verdict()`
     (Full / Provisional / Withheld). It adds the **numeric judgment-input editing** (future growth %, forecast
     high/low P/E, the four-option low-price selector, current price, dividend), the **§4 zone bar** (the FIRST
     time saturated colour is spent), the **U/D ratio + projected return**, the **verdict badge** (full /
     provisional-hatched / degraded / withheld), the **sticky verdict bar**, and the **traceability view**
     (inputs → provenance → rule). SCOPE GUARDRAIL — 2.6 is exact-VALUE judgment entry + engine wiring + the
     §4 zone bar + verdict + traceability ONLY: NO §1 interactive draggable chart / live drag-recolour (Story
     2.8 — §1 stays the 2.3 placeholder; the zone bar recolours on VALUE change, not on a chart drag); NO
     plausibility / low-confidence WARNING surfacing as a distinct cell warning (Story 2.7 — but the engine's
     `low_confidence` flag DOES drive the verdict to Provisional, that is 2.6's verdict-integrity job); NO
     undo/redo / scenario-compare (2.9); NO decision-rationale capture (2.10); NO provider fetch / quarterly
     auto-fill (Epic 3). The engine, contract, and persistence are CONSUMED, never modified — every type 2.6
     needs already exists (Story 1.7 normalize, 1.8 ssg::compute, 1.11 verdict::StudySnapshot, 2.2 issue-#14
     Judgment schema). `Cargo.lock`/`deny.toml` expected UNCHANGED (no new dependency). Headless CI cannot
     prove zone-bar render / colour-budget / verdict-badge texture: the visual-verification DoD (AC 8) is
     load-bearing exactly as for 2.1–2.5 — but the judgment-entry → persist → reopen round-trip, the full
     engine-wiring path (Study→Raw→normalize→compute→snapshot), and the verdict-integrity derivation (Full
     vs Provisional vs Withheld for every gate combination) ARE proven headlessly. -->

## Story

As Guy,
I want to set judgment values numerically and read a trustworthy verdict,
so that I reach a defensible buy/hold/sell conclusion even before touching the chart.

## Acceptance Criteria

1. **Numeric judgment inputs, entered by exact value, persisted (FR6, FR31 exact-value path).** Every §1/§3/§4
   judgment input the SSG method needs is editable by **exact keyboard value** (locale-aware parse → `Money`,
   the 2.4 `parse_amount` rail), each change persisted to `contract::Study.judgment` via `Journal::put_study`:
   - **future sales growth %/yr** (`projected_sales_growth_pct`) and **future EPS growth %/yr**
     (`projected_eps_growth_pct`) — the §1 trend judgments (FR6);
   - **estimated high EPS** (`estimated_high_eps`) and **estimated low EPS** (`estimated_low_eps`) — direct
     judgment values (the engine derives high-EPS from EPS growth only when the direct value is absent;
     low-EPS is direct-only in v1);
   - **judged average high P/E** (`judged_avg_high_pe`) and **judged average low P/E** (`judged_avg_low_pe`)
     — the §3/§4 multiples;
   - **the four-option low-price method selector** (`forecast_low_option` ∈ `{AvgLowPeTimesEps,
     AvgLowPriceLast5y, RecentSevereLow, DividendSupported}`) plus the option-(c) input **recent severe low**
     (`recent_severe_low`);
   - **current price** (`current_price`) and **present full-year dividend** (`present_full_year_dividend`).
   `unknown/insufficient` is **never** shown or stored as `0`: a cleared input is `None`, rendered as the
   faithful em-dash (`EMPTY_SLOT`), never coerced.

2. **The engine (Epic 1) recomputes and every §2/§3/§4/§5 RESULT + the verdict update (FR6).** On any data
   edit (2.4) **or** judgment edit (AC 1), the app calls `core` ONCE per frame through the single construction
   path `core::verdict::StudySnapshot::new(&canonical, &judgment_inputs, &observations, gates)`:
   - **map** `contract::Study` → `core::RawFinancials` (per-year `RawAmount{value, currency}` from each present
     `Cell.value`, native currency from `Study.native_currency`) → `core::normalize()` → `CanonicalFinancials`
     (a `NormalizeError` surfaces as a **neutral notice**, never `unwrap`/`.ok()`);
   - **map** `contract::Judgment` → `core::ssg::JudgmentInputs` (each `Option<Money>` → `Option<Decimal>` via
     `.as_decimal()`; `contract::ForecastLowOption` → `core::ssg::ForecastLowOption` by-name);
   - the previously em-dash **§2 ratios/averages/trends, §3 P/E history A–H + averages, §4 forecast high/low +
     zones + U/D, §5 yield/return** now render the engine's `SsgOutputs` as **already-formatted, locale-aware
     strings** (the adapter rule — no `Decimal`/`f64`/domain struct crosses into `.slint`). Each `Option::None`
     output renders as `EMPTY_SLOT`, never `0`. **No calculation in `app`** (Cardinal Rule).

3. **The §4 zone bar — the FIRST sanctioned spend of saturated colour (FR6, FR31).** A single **vertical
   Buy/Neutral/Sell** bar (equal thirds of the forecast range, `core::ssg::ZoneBounds`) with a **price axis
   beside it** (the 4 boundary prices, top→bottom) and a **present-price marker** (`Zone` of the current price):
   - the three **Okabe-Ito zone hues already in `Tokens`** (`zone-buy #009E73`, `zone-hold #E69F00`,
     `zone-sell #D55E00`) — **fill at the per-theme alpha** (`zone-alpha`: dark 0.36, light 0.165) **plus a
     1.5–2 px full-saturation edge stroke** per boundary; redundant encoding = hue + value + vertical position
     (buy low → sell high) + the text label (the zone-label noun, exempt from the banned-verb gate);
   - the bar is **muted in the entry regime → full in contemplation** (the `regime-emphasis` token, the 2.3/2.5
     hook) — same constant geometry, no re-layout;
   - when `zones` is `None` (a forecast input missing or the range degenerate) the bar renders a **calm empty
     state**, never a fake band; the geofenced `✓`-green from 2.5 **never co-presents** with the zone bands.

4. **U/D ratio, projected return, and the verdict badge (FR6).** Derived from the same snapshot:
   - **U/D ratio** = `core::ssg::UpsideDownside` (the three-state `Ratio(d)` / `Undefined` / `Unknown`) —
     `Undefined`/`Unknown` render as a fact-stating em-dash/label, **never** a fabricated ratio;
   - **projected return** = `ReturnOutputs.projected_total_annualized_return_pct` and **appreciation** =
     `projected_appreciation_pct`;
   - the **verdict badge** renders the `core::verdict::Verdict` state: **Full** / **Provisional** / **Withheld**
     (the engine's exact tri-partition), with neutral wording (the surfaced facts, never a "buy"/"sell"
     command).

5. **The sticky verdict bar (FR6).** A bar **pinned at the top of the study scroll area** (`app.slint`'s named
   "sticky verdict bar") stays visible while scrolling/folding and shows: **verdict** + **present price** +
   **projected return** + **appreciation** (+ capital-at-risk em-dash slot, an Epic-4 forward — caption-only,
   never faked). Constant geometry; the bar derives from the same single snapshot as the §4 badge (one
   coherence frame, never two numbers from two computations).

6. **Verdict integrity holds — full saturated colour ONLY when every load-bearing input is `✓` & not stale
   (FR12).** This is the anti-silent-wrong-signal rule, and it is the engine's by construction — the app's job
   is to **build the gates correctly and render the three states honestly**:
   - the app builds `core::verdict::InputGates` = one `YearGates` per **usable** year (each year's four
     `LOAD_BEARING_YEAR_FIELDS` `["sales","eps","high_price","low_price"]` mapped from `Cell.review × freshness`)
     + the five `LOAD_BEARING_JUDGMENT_INPUTS` `["estimated_high_eps","estimated_low_eps","judged_avg_high_pe",
     "judged_avg_low_pe","current_price"]`;
   - **`Cell` → `GateState`:** `None`→`Missing`; `(Validated, Current)`→`ValidatedFresh`;
     `(Validated, Stale)`→`Stale`; else `NotValidated`. **Judgment-input → `GateState`** (interpretation to
     record — judgment values are bare `Option<Money>`, not review-tagged `Cell`s): `None`→`Missing`,
     `Some`→`ValidatedFresh` (a deliberately-typed personal judgment is validated-fresh by the act of entry;
     it is the user's own number, not provider data awaiting sign-off);
   - the engine then derives **Full** (all gates `ValidatedFresh` ∧ ¬`low_confidence`) → **full saturated zone
     colour + full verdict badge**; **Provisional** (≥1 gate not-validated/stale, or `low_confidence`) →
     **hatched/provisional texture + temporal-provenance caption** ("computed from data of DD/MM"), neutral ink,
     not full bands; **Withheld** (≥1 load-bearing input `Missing`) → the verdict is **withheld** (named open
     gates, no colour). `verdict.isFull ⟹ ∀ load-bearing input validated ∧ ¬stale` is structurally guaranteed
     — the app must not paint full colour beside a non-green input.

7. **Traceability view — any result → its inputs, their provenance, and the rule that produced it (FR11).** The
   user can open a traceability surface for a chosen result (e.g. the verdict, a zone bound, a §4 forecast)
   that names: the **inputs** it descends from (the cells / judgment values), each input's **provenance**
   (source × freshness × review, from the 2.4/2.5 model + the cell `Provenance` timestamp), and the **rule /
   formula** that produced it (the method identity — `core::METHOD_VERSION` + a fact-stating formula caption,
   reusing the §4/§5 formula-expression captions already in the screen). The view spends **no colour**; open
   gates (the `Verdict::open_gates()` for a degraded verdict) are surfaced here as the honest "why not full".

8. **Crate-boundary, quality gates, posture, dependency & pinned-surface discipline + visual verification
   (Definition of Done).**
   - **Cardinal Rule — no calculation in `app`:** every number comes from `core` (the snapshot); the adapter
     maps domain → Slint structs and formats `Money`/`Decimal` → strings (`format_amount`/named rounding from
     `core`). The verdict crosses as an **enum-derived string** (`"full"|"provisional"|"withheld"`) + already-
     formatted fact strings; **no `Decimal`/`f64`/domain struct leaks into `.slint`**.
   - **All four gates green `--locked`:** `cargo fmt --all --check` · `cargo clippy --all-targets
     --all-features --locked -- -D warnings` · `cargo test --all --locked` · `cargo deny check`.
   - **NO new external dependency** — `steadyinvest-core` is already an `app` dependency and already used
     (`METHOD_VERSION`, `BANNED_VERBS`); 2.6 only adds the first *engine* (`compute`/`StudySnapshot`) call
     against it. `Cargo.lock` and `deny.toml` expected **UNCHANGED**; if any lock change appears, stop and
     record why.
   - **Pinned surfaces untouched** (`git diff` empty): `core/`, `contract/`, `persistence/`, `ingestion/`,
     `report/`, `docs/method/**`, `.github/`, `rust-toolchain.toml`, the frozen
     `persistence/tests/corpus/v1.db`, and `deny.toml`. **`contract/` is NOT modified** — the `Judgment`
     schema (issue #14, Story 2.2) and the `Cell` review/freshness model already exist; 2.6 consumes them.
   - **Posture / banned verbs:** every new user-visible string (judgment-input labels, the zone-label nouns,
     the verdict-state captions, the temporal-provenance caption, the traceability labels, any
     normalize-failure notice) passes the crate-local banned-verb gate — register them in the scanned `@tr()` /
     `USER_FACING_MESSAGES` surfaces and **bump the asserted floors** (`>= 14` `.slint` files, `>= 100` `@tr`
     total, the message count) to the new actual counts. Verdict/zone wording is **fact-stating** (the zone
     nouns + the surfaced criterion facts), **never** advice. Reuse `core::method::BANNED_VERBS_FR/EN`.
   - **Accessibility:** the verdict and zone decision is **never colour-only** — carried by hue + value +
     vertical position + the text label; new judgment-input fields follow the 2.4 `TextInput` + visible
     focus-ring / keyboard pattern.
   - **Visual verification (load-bearing, mirrors 2.1–2.5):** launch the built app, open a study with data,
     enter judgment values, and verify on display: the **§2/§3/§4/§5 results now show real numbers** (not
     em-dashes) and **recompute on edit**; the **§4 zone bar** shows the three bands at the per-theme alpha +
     edge strokes, the **present-price marker** in the right zone, and the **price axis**; the bar is **muted
     in entry → full in contemplation**; the **verdict badge** reads **Full** with all-`✓`-fresh inputs, flips
     to **Provisional** (hatched + "computed from data of DD/MM") when a load-bearing cell is un-validated or
     stale, and to **Withheld** when one is missing; the **sticky verdict bar** stays pinned while scrolling;
     the **traceability view** opens and names inputs/provenance/rule; **close → relaunch → reopen** restores
     the judgment inputs and the recomputed verdict. Confirm the footer disclaimer (FR64), theme/label/locale
     swaps (2.1), fold/regime (2.3), entry/coverage (2.4), and review markers/soft-lock (2.5) still work, and
     launch-to-interactive stays ~within 3 s (NFR-P4) and a recompute-on-edit feels instant (NFR-P1 budget —
     the value-driven recompute, the chart-drag <100 ms is 2.8). Record the run in the Dev Agent Record.
     Headless CI cannot stand in for this AC — but the **judgment-entry → `put_study` → reopen** round-trip,
     the **full engine-wiring path** (`Study`→`Raw`→`normalize`→`compute`→`snapshot`), the **adapter
     formatting** (unknown → em-dash, never `0`), and the **verdict-integrity derivation** (Full/Provisional/
     Withheld for every gate combination) ARE proven headlessly.

## Tasks / Subtasks

- [x] **Task 1 — The engine-wiring adapter: `contract::Study` → `core` inputs → `StudySnapshot` (AC: 2, 6)**
  - [x] Add a **new** `app/src/viewmodel/engine.rs` (recommended; or extend `form.rs`) that owns the
        contract→core mapping. Functions (pure, unit-testable, no I/O):
    - `to_raw_financials(study: &Study) -> core::RawFinancials` — one `core::normalize::RawYear` per
      `Study.years` entry: each present `Cell.value` (`Money`) → `RawAmount{ value: money.as_decimal(),
      currency: study.native_currency.clone() }`; absent/`None` cells → `None`; `native_currency` from the
      study; `splits: vec![]` (no split events in v1 manual entry — record). Map `sales/eps/high_price/
      low_price` (non-optional `Cell`) and the optional `dividend_per_share/pre_tax_profit/
      book_value_per_share`.
    - `to_judgment_inputs(judgment: &contract::Judgment) -> core::ssg::JudgmentInputs` — each `Option<Money>`
      → `Option<Decimal>` via `.as_decimal()`; `contract::ForecastLowOption` → `core::ssg::ForecastLowOption`
      by-name (a `match`, NOT `as`-cast — record the by-name glue so a future variant can't silently
      mis-map).
    - `to_observations(study) -> core::ssg::QuarterlyObservations` — **v1: `QuarterlyObservations::empty()`**
      (the manual study carries no quarterly data yet → current P/E / relative value are honestly `unknown`;
      quarterly capture is a later story / Epic 3). Record this as an interpretation — `current_pe: None` is
      faithful, never faked.
    - `to_input_gates(study, canonical) -> core::verdict::InputGates` — one `YearGates` per **usable** year
      (filter `canonical.years` on `YearUsability::Usable`, read the matching `Study` year's cells), each
      year's `[GateState; 4]` from the four `LOAD_BEARING_YEAR_FIELDS` cells via `cell_to_gate_state`; the
      five judgment `[GateState; 5]` via `judgment_to_gate_state`.
    - `cell_to_gate_state(cell: Option<&Cell>) -> GateState`: `None`→`Missing`; `(Validated,Current)`→
      `ValidatedFresh`; `(Validated,Stale)`→`Stale`; else `NotValidated`.
    - `judgment_to_gate_state(value: Option<Money>) -> GateState`: `None`→`Missing`; `Some`→`ValidatedFresh`
      (recorded interpretation — AC 6).
  - [x] Add `app/src/state.rs::snapshot_for(study_id) -> Result<StudySnapshot, String>` (or compute on read):
        re-read the study, run the mapping, `normalize()` (a `NormalizeError` → a **neutral notice** via a
        new `MSG_*`, never `unwrap`), then `StudySnapshot::new(...)`. **One** call site — the single
        construction path, so outputs + verdict are always one coherent frame (architecture: "an incoherent
        frame is structurally impossible").
  - [x] **`steadyinvest-core` is already declared in `app/Cargo.toml` (`{ workspace = true }`) and already
        used** (`form.rs:89` `METHOD_VERSION`, `posture.rs` `BANNED_VERBS`) — so 2.6 adds only the first
        *engine* (`compute`/`StudySnapshot`) call, no new dep. The crate-wide `unused_crate_dependencies`
        allow covers only `ingestion`/`report`/`tokio` (not `core`); leave it. `Cargo.lock` stays unchanged.
  - [x] Headless tests: a golden-style study (mirror a `core/tests` fixture / a `core::golden` study) →
        `snapshot_for` → the outputs match the engine's direct `compute()` on the same inputs (the adapter
        introduces no drift); a study with a missing load-bearing cell → `Verdict::Withheld`; all-`✓`-fresh →
        `Verdict::Full`; one stale/un-validated cell → `Verdict::Provisional`; `low_confidence` (<5 usable
        years) → `Provisional`.

- [x] **Task 2 — Numeric judgment-input editing → persist (AC: 1)**
  - [x] Add `state::set_judgment_field(study_id, field, value: Option<Money>)` on the mutation rail
        (re-read study → set one `Judgment` field → `put_study`, reusing the read-only/no-journal/save-failure
        guards + neutral notices; **no silent `.ok()`**). Add `state::set_forecast_low_option(study_id,
        option)` for the selector. A cleared field is `None` — **never `0`** (the project's most-repeated rail).
  - [x] Reuse the 2.4 `parse_amount(input, format)` locale-aware path for every numeric judgment field. The
        growth/P/E/price fields are exact-value `TextInput`s (the 2.4 cell pattern); the low-price method is a
        4-option selector (radio/segmented control or a `ComboBox`-equivalent — record the chosen Slint
        control).
  - [x] Surface the judgment inputs in the faithful form: future-growth + estimated-EPS near §1 (the chart is
        still a 2.8 placeholder, but the VALUES live here), judged high/low P/E in §3, the low-price selector +
        recent-severe-low + current price + dividend in §4. Keyboard-reachable, posture-gated French labels.
  - [x] Headless round-trip: set each judgment field → `put_study` → re-`get_study` → the value survives;
        clearing a field stores `None` (verified not `0`); changing `forecast_low_option` round-trips.

- [x] **Task 3 — Wire engine outputs into the §2/§3/§4/§5 result slots (AC: 2)**
  - [x] In the adapter (`form.rs` / `engine.rs`), replace the em-dash placeholders for the **computed** slots
        with the snapshot outputs, **formatted as strings** (`format_amount` / a percent formatter / a ratio
        formatter — define the per-field display via `core::rounding` named scale; never hand-round in `app`):
    - **§2:** `ManagementOutputs` per-year PTP%/ROE%, the 5-yr averages, the trends (`Trend` → a fact-stating
      glyph/label, not colour).
    - **§3:** `ValuationOutputs` per-year high/low P/E, payout%, yield%, the averages (`avg_high_pe`,
      `avg_low_pe`, `avg_pe`), `current_pe` (em-dash in v1 — empty observations), `relative_value_pct`.
    - **§4:** `RiskRewardOutputs.forecast_high/forecast_low`, the `ZoneBounds`, U/D (Task 4), the §4 D/E rows
      (U/D ratio, appreciation).
    - **§5:** `ReturnOutputs` present yield, avg annual EPS/dividend, avg yield, appreciation, total
      annualized return.
  - [x] **Every `Option::None` output → `EMPTY_SLOT`** (the existing em-dash const), never `0`, never a
        fabricated number. The fold-summaries (§2 "Marge — · ROE —", §4 "Zone — · H/B —", §5 "Rdt annuel
        total —") now show the real summary values when known.
  - [x] Headless test: the formatted output strings for a known golden study match expected (a small
        snapshot-string test over the adapter); an unknown metric formats as `EMPTY_SLOT`.

- [x] **Task 4 — The §4 zone bar component (AC: 3, 4)**
  - [x] Add **new** `app/ui/components/zone_bar.slint` (`ZoneBar`, architecture-named): a vertical
        Buy/Neutral/Sell bar from `ZoneBounds` (equal thirds), each segment filled at `Tokens.zone-buy/hold/
        sell` × `Tokens.zone-alpha` + a **1.5–2 px full-saturation edge stroke** per boundary; the price axis
        (the 4 boundary prices) beside it; the **present-price marker** positioned by the current price within
        `[forecast_low, forecast_high]` and tagged with its `Zone`. Constant geometry. The bar reads
        `Tokens.regime-emphasis` (muted entry → full contemplation). A `None` zones input → a **calm empty
        state** (no fake band). **No hard-coded hex/px** — tokens only.
  - [x] Cross the zone data as an adapter struct of **strings + a zone enum-string + numeric positions as
        normalized floats for layout only** (positions are geometry, not money — but the displayed prices are
        formatted strings). The U/D ratio crosses as a formatted string (`"3.4:1"`) with the
        `Undefined`/`Unknown` states as fact-stating labels.
  - [x] Mount the `ZoneBar` into the §4 placeholder region of `study_screen.slint` (replacing the
        `PlaceholderRegion` for §4; the §1 chart placeholder stays — that is 2.8).

- [x] **Task 5 — The verdict badge + the sticky verdict bar (AC: 4, 5, 6)**
  - [x] Add **new** `app/ui/components/verdict_badge.slint` (`VerdictBadge`, architecture-named): renders the
        verdict-state string `"full"|"provisional"|"withheld"`:
    - **full** → full saturated colour (the zone hue of the present-price zone) + the surfaced facts;
    - **provisional** → a **hatched/outline texture** (neutral ink, NOT a full band) + the temporal-provenance
      caption ("Calculé à partir des données du DD/MM" — fact-stating);
    - **withheld** → no colour, the named open gates / a neutral "verdict withheld — input(s) missing" fact.
    Decision carried by texture + label, **never colour alone**.
  - [x] Add the **sticky verdict bar** to `app.slint` (the named surface) or `study_screen.slint`'s pinned
        header: verdict badge + present price + projected return + appreciation (+ a caption-only capital-at-
        risk em-dash, Epic-4 forward). `position: sticky`-equivalent in Slint (a pinned row outside the scroll
        viewport, the 2.1 footer/2.5 confirm-overlay pinning pattern). Derives from the SAME snapshot as §4.
  - [x] Cross the verdict as an adapter struct: `state: string`, the formatted fact strings, the temporal-
        provenance date string (from the cells' `Provenance` / the engine inputs date), and the open-gate
        names (for the badge + traceability). **No `Verdict` domain enum / `Decimal` into `.slint`.**
  - [x] Headless test: the adapter maps `Verdict::Full/Provisional/Withheld` → the right state string + the
        right fact set + (for degraded) the open-gate names; the temporal-provenance string is present for
        provisional, absent/structural for full.

- [x] **Task 6 — The traceability view (AC: 7)**
  - [x] Add a traceability surface (a `PopupWindow` / an inline expandable panel — record the pattern; reuse
        the 2.5 confirm-overlay primitive shape) opened from a result (the verdict badge and/or a §4 result
        row, keyboard-reachable). It names, for the chosen result: the **inputs** (the cells / judgment values
        it descends from), each input's **provenance** (source × freshness × review + the `Provenance`
        timestamp), and the **rule** (the `core::METHOD_VERSION` identity + the fact-stating formula caption).
  - [x] For a degraded verdict, surface the `Verdict::open_gates()` here as the honest "why not full" (which
        load-bearing input is missing / un-validated / stale). **No colour spent.** Fact-stating French copy.
  - [x] Headless test: the traceability adapter, given a result + the snapshot, returns the correct input
        list + provenance + rule identity + (for degraded) the open gates.

- [x] **Task 7 — Posture, accessibility, dependency & gate discipline (AC: 8)**
  - [x] Extend `app/src/posture.rs` scanned surfaces with the new strings (judgment-input labels, zone-label
        nouns, verdict-state captions, temporal-provenance caption, traceability labels, any normalize-failure
        `MSG_*`); **bump the floors** (`.slint` files `>= 16` if `zone_bar.slint` + `verdict_badge.slint` are
        added, `@tr` total to the new actual, `USER_FACING_MESSAGES` count). Verdict/zone copy fact-stating;
        reuse `core::method::BANNED_VERBS_FR/EN`. **Note:** the zone-label nouns ("Buy/Neutral/Sell" →
        "ACHAT/NEUTRE/VENTE") are method nouns exempt from the banned-verb gate (spec §6 / `Zone::label`
        comment) — keep them, they are topology not commands.
  - [x] A11y by construction: verdict + zone decisions carry hue + value + vertical position + text label
        (never colour alone); new judgment-input `TextInput`s have visible focus rings, logical tab order,
        keyboard entry; the low-price selector is keyboard-operable.
  - [x] All four gates green `--locked`. `git diff` over the pinned surfaces (`core/ contract/ persistence/
        ingestion/ report/ docs/method/ .github/ rust-toolchain.toml deny.toml` + the frozen `v1.db`) is
        **empty**. **`Cargo.lock` unchanged** (confirm `core` was already in the lock tree; no new crate).

- [x] **Task 8 — Visual verification, records & File List (AC: 8)**
  - [x] Launch, walk the AC-8 journey (enter judgment values → §2–§5 results compute → §4 zone bar with bands
        + edge strokes + present-price marker + price axis → entry-muted vs contemplation-full → verdict Full
        vs Provisional-hatched vs Withheld for the three gate cases → sticky verdict bar pinned while
        scrolling → traceability view names inputs/provenance/rule → **relaunch → judgment + verdict
        restored**), record the outcome (and any sandbox AT-SPI / headed-render limitation, as 2.1–2.5 did) in
        the Dev Agent Record.
  - [x] Prove headlessly what the sandbox blocks visually: the **judgment-entry → `put_study` → reopen**
        round-trip, the **full engine-wiring path** (`Study`→`Raw`→`normalize`→`compute`→`snapshot`, outputs
        matching a direct `compute()`), the **adapter formatting** (unknown → em-dash, never `0`), and the
        **verdict-integrity derivation** (Full/Provisional/Withheld across the gate matrix). Refresh test
        counts in the Change Log.
  - [x] Update the **File List** (every new/modified file incl. any QA test file + the story-automator log —
        issue #18 discipline) and file a consolidated GitHub issue for the genuine 2.6 interpretations (the
        judgment-input gate-state mapping, the empty-`QuarterlyObservations` v1 choice, the per-field display-
        scale source, the low-price selector control chosen, the traceability surface pattern, the
        sticky-bar pinning technique, the forwarded perceptual confusability/zone-colour CI gate) — issues,
        not inline TODOs (the 1.11/2.1–2.5 pattern).

## Dev Notes

### What this story is — and the disasters it must make impossible

2.6 is **the engine-wiring story**: the first time the `app` crate calls `core`. Every story before it built the
trustworthy *input* surface (2.3 structure, 2.4 entry+provenance, 2.5 review+soft-lock) and left every computed
slot a faithful em-dash. 2.6 makes the app compute and display the **verdict** — numeric judgment inputs in,
zone bar + U/D + projected return + verdict badge + sticky bar + traceability out — through the single
construction path `core::verdict::StudySnapshot::new(...)`, so the verdict and its inputs are born in one
coherent frame. The contract `Judgment` schema (issue #14, Story 2.2) and the verdict machinery (Story 1.11)
were built *for this story*; 2.6 consumes them, it does not modify `core`/`contract`/`persistence`.

Disasters to prevent:
- **Calculation in `app` (the Cardinal Rule, the #1 risk for THIS story).** Every number — every ratio,
  average, P/E, forecast, zone bound, U/D, return, the verdict tri-partition — comes from `core`. The app
  **maps** types and **formats** `Money`/`Decimal` → strings (via `core::rounding` named scale +
  `format_amount`); it does **not** add, divide, compare-to-threshold, or derive a verdict. If you find
  yourself writing arithmetic on a price or a P/E in `app`, stop — it belongs in `core` (which already has it).
- **Scope bleed into 2.7 / 2.8 / 2.9.** 2.6 is exact-VALUE judgment entry + engine wiring + the §4 zone bar +
  verdict + traceability:
  - **NO §1 interactive draggable chart / live drag-recolour** — **Story 2.8**. §1 stays the 2.3 placeholder.
    The zone bar recolours on a **value change** (the user types a new growth/P/E), not on a chart drag; the
    <100 ms drag budget (NFR-P1) is 2.8's. (The value-driven recompute should still feel instant.)
  - **NO plausibility / unit-split / low-confidence WARNING as a distinct cell warning** — **Story 2.7**. BUT
    the engine's `SsgOutputs.low_confidence` (<5 usable years, FR8) and `findings` DO exist and DO drive the
    verdict to **Provisional** — surfacing that *in the verdict* is 2.6's verdict-integrity job; surfacing the
    *cell-level warning glyph* is 2.7.
  - **NO undo/redo / scenario-compare** — **Story 2.9** (the full `state.rs` snapshot stack). 2.6 computes the
    snapshot on read; the undo stack is a documented partial.
  - **NO decision-rationale capture** — **Story 2.10** (`Study.rationale` exists in contract but 2.6 doesn't
    edit it).
  - **NO provider fetch / quarterly auto-fill / reconciliation** — **Epic 3**. `QuarterlyObservations` is
    `empty()` in v1 → current P/E unknown (honest, not faked).
- **`unknown` rendered or stored as `0`** — the project's single most-repeated rail. Every engine output is an
  `Option`; `None` → `EMPTY_SLOT` (em-dash), never `0`. A cleared judgment input is `None`, never `0`. The §9
  "unknown is a state, not a number" discipline is already in `core`; the app must not undo it at the display
  boundary (e.g. don't `unwrap_or(0)` a `Decimal`).
- **A verdict in full colour beside a non-green input (the FR12 disaster).** Full saturated colour is spent
  **only** when the engine returns `Verdict::Full` — which it does only when every gate is `ValidatedFresh` ∧
  ¬`low_confidence`. The app must build the gates correctly (the `cell_to_gate_state` / `judgment_to_gate_state`
  mapping) and render Provisional as **hatched/neutral + temporal provenance**, Withheld as **no colour +
  named open gates**. Never paint a full band from a Provisional/Withheld verdict.
- **Spending the colour budget wrongly.** 2.6 is the FIRST sanctioned spend of saturated colour — the three
  zone hues (`zone-buy/hold/sell`, already in `Tokens`). Everything else stays greyscale ink. The geofenced
  `✓`-green (2.5, `validated-ink ≈ #4A7C6F`) **never co-presents** with the zone bands (2.5 already attenuates
  it in contemplation, where the zones light up). Do NOT introduce a new hue for the verdict states — Provisional
  is a *texture* (hatch), not a colour.
- **An incoherent frame (two numbers from two computations).** The §4 badge and the sticky bar must derive from
  the **same** `StudySnapshot`. Call `StudySnapshot::new` once per frame; never compute the zone bar from one
  call and the verdict from another. The architecture guarantees this by construction *if* you use the single
  construction path — so use it.
- **Mutating `contract` / pinned surfaces.** The `Judgment` schema (10 fields, issue #14) and the verdict
  machinery already exist. **Do not change `contract/` or `core/`.** No new dependency → `Cargo.lock` unchanged.
- **A scattered wall-clock / `Uuid::new_v4`.** Any provenance stamp on a judgment edit comes only from the
  injected `Clock` (ADD15) — the 2.4/2.5 `manual_provenance` rail.

### Scope — the one-paragraph contract

> 2.6 wires Epic 1's engine into the live UI for the first time: it adds **exact-value numeric judgment entry**
> (future sales/EPS growth %, estimated high/low EPS, judged high/low P/E, the four-option low-price selector +
> recent-severe-low, current price, dividend), each persisted to `contract::Study.judgment`; on any data
> (2.4) or judgment edit it maps `contract::Study` → `core::RawFinancials` → `normalize` → `CanonicalFinancials`,
> maps `Judgment` → `core::ssg::JudgmentInputs`, builds `core::verdict::InputGates` from the usable years'
> cell review×freshness + the judgment inputs, and calls **`StudySnapshot::new(...)` once per frame**; it then
> renders the engine's `SsgOutputs` into the previously-em-dash §2/§3/§4/§5 result slots (formatted strings,
> unknown → em-dash never `0`), the **§4 zone bar** (the first saturated-colour spend — Buy/Neutral/Sell at the
> per-theme alpha + edge strokes + present-price marker + price axis, muted in entry → full in contemplation),
> the **U/D ratio + projected return**, the **verdict badge** (Full / Provisional-hatched / Withheld, the
> engine's exact tri-partition), the **sticky verdict bar**, and a **traceability view** (inputs → provenance →
> rule). **Verdict integrity** holds by construction (full colour ⟺ all load-bearing inputs `✓` & not stale).
> It builds **no §1 chart (2.8), no cell-level plausibility warning (2.7), no undo/redo (2.9), no rationale
> (2.10), no provider fetch (Epic 3)**, adds **no new dependency**, and does **not** modify `core`/`contract`/
> `persistence`.

### The engine API surface (verified — call the REAL API, do not invent)

**Single construction path** (`core/src/verdict.rs:371`):
```rust
core::verdict::StudySnapshot::new(
    financials: &core::CanonicalFinancials,   // from core::normalize(raw)
    judgment:   &core::ssg::JudgmentInputs,
    observations: &core::ssg::QuarterlyObservations,
    gates:       core::verdict::InputGates,
) -> StudySnapshot
// .outputs() -> &SsgOutputs   .verdict() -> &Verdict   .inputs_hash() -> &str   .method_version() -> &'static str
```
`StudySnapshot::new` runs `ssg::compute` itself and derives the verdict — `SsgOutputs` is never caller-supplied,
so a mismatched outputs/inputs frame is unrepresentable. **Use this, not `ssg::compute` directly** (you need the
verdict).

**Re-exports** (`core/src/lib.rs:24-28`): `core::{normalize, CanonicalFinancials, RawFinancials, compute,
JudgmentInputs, QuarterlyObservations, SsgOutputs}` and the `verdict::{...}` group.

**`normalize`** (`core/src/normalize/mod.rs:32`): `pub fn normalize(raw: RawFinancials) -> Result<CanonicalFinancials,
NormalizeError>` — **handle the `Err`** (neutral notice). `RawFinancials { native_currency: String, years:
Vec<RawYear>, splits: Vec<SplitEvent> }`; `RawYear { year, period_months, fiscal_year_end_month, sales, eps,
high_price, low_price, dividend_per_share, pre_tax_profit, net_profit, tax_rate, book_value_per_share }` (each
amount `Option<RawAmount>`, `RawAmount { value: Decimal, currency: String }`). `RawYear::empty(year)` is the
construction base.

**`JudgmentInputs`** (`core/src/ssg/types.rs:31`) — fields mirror `contract::Judgment` **by name** (alignment,
not import): `estimated_high_eps`, `estimated_low_eps`, `projected_sales_growth_pct`,
`projected_eps_growth_pct`, `judged_avg_high_pe`, `judged_avg_low_pe`, `forecast_low_option:
ForecastLowOption`, `recent_severe_low`, `current_price`, `present_full_year_dividend` (all `Option<Decimal>`
except the option enum). `JudgmentInputs::empty()` available.

**`ForecastLowOption`** (`core/src/ssg/types.rs:17`) and `contract::ForecastLowOption`
(`contract/src/study.rs:31`) have identical variants: `AvgLowPeTimesEps | AvgLowPriceLast5y | RecentSevereLow |
DividendSupported`. Glue by `match` (by-name), never `as`-cast.

**`QuarterlyObservations`** (`core/src/ssg/types.rs:93`): `ttm_quarterly_eps: Option<[Decimal;4]>` (Σ = TTM EPS,
the current-P/E denominator) + 4 quarter sales/EPS fields. **v1: `QuarterlyObservations::empty()`** → current
P/E / relative value / quarterly change are `unknown` (faithful — no quarterly data is captured yet).

**Verdict gates** (`core/src/verdict.rs`): `InputGates::new(year_gates: Vec<YearGates>, judgment_gates:
[GateState; 5])`; `YearGates::new(year: i32, states: [GateState; 4])`. `GateState ∈ {Missing, NotValidated,
Stale, ValidatedFresh}`. The catalogs (`core/src/method/mod.rs:25-35`): `LOAD_BEARING_YEAR_FIELDS =
["sales","eps","high_price","low_price"]`; `LOAD_BEARING_JUDGMENT_INPUTS = ["estimated_high_eps",
"estimated_low_eps","judged_avg_high_pe","judged_avg_low_pe","current_price"]`. **Caller's duty: one
`YearGates` per USABLE year** (`canonical.years` filtered on `YearUsability::Usable`).

**Verdict derivation** (`core/src/verdict.rs:418`, `derive_verdict`): `Full` ⟺ all gates `ValidatedFresh` ∧
¬`low_confidence`; `Withheld` ⟺ ≥1 gate `Missing`; `Provisional` ⟺ every other degraded case. `Verdict ∈
{Full(FullVerdict), Provisional(DegradedVerdict), Withheld(DegradedVerdict)}`; `.facts() -> &VerdictFacts`,
`.open_gates() -> &[OpenGate]`, `.low_confidence() -> bool`, `.method_version()`, `.inputs_hash()`.

**Zone-bar / verdict outputs** (`core/src/ssg/types.rs`): `RiskRewardOutputs { forecast_high: Option<Decimal>,
forecast_low: Option<Decimal>, zones: Option<ZoneBounds>, present_price_zone: Option<Zone>, upside_downside:
UpsideDownside }`; `ZoneBounds { forecast_low, buy_top, neutral_top, forecast_high }` (all `Decimal`); `Zone ∈
{Buy, Neutral, Sell}` with `Zone::label() -> &'static str` (the **English** noun "Buy"/"Neutral"/"Sell",
banned-verb-exempt — the French ACHAT/NEUTRE/VENTE come from the app's `@tr()` layer, never `core`);
`UpsideDownside ∈
{Ratio(Decimal), Undefined, Unknown}`. `ReturnOutputs { present_yield_pct, avg_annual_eps, avg_annual_dividend,
avg_yield_pct, projected_appreciation_pct, projected_total_annualized_return_pct }` (all `Option<Decimal>`).
`VerdictFacts { present_price_zone, ud_at_or_above_target, relative_value_below_ceiling,
present_price_in_buy_zone, appreciation_at_or_above_double, quality_value_candidate: bool }` — **neutral facts,
never a recommendation**.

### The contract types 2.6 consumes (verified, never modified)

- **`contract::Judgment`** (`contract/src/study.rs:52`): the 10-field persisted judgment snapshot (issue #14,
  Story 2.2) — all `Option<Money>` except `forecast_low_option: ForecastLowOption`. 2.6 *edits* these.
- **`contract::Cell`** (`contract/src/cell.rs`): `{ value: Option<Money>, source, freshness, review, coverage,
  provenance }`; `Cell::edited(...)` is the manual-mutation rail (2.4/2.5). 2.6 reads `review × freshness` for
  the gates; it does NOT route judgment edits through `Cell` (judgment inputs are bare `Money` fields, not
  cells).
- **`contract::Study`** (`contract/src/study.rs:82`): `{ id, journal_id, security_ticker, native_currency,
  years: Vec<YearData>, judgment, rationale, created_at, schema_version }`. `YearData { year, sales, eps,
  high_price, low_price (Cell), dividend_per_share, pre_tax_profit, book_value_per_share (Option<Cell>) }`.
- **`contract::Money`** (`contract/src/money.rs`): `Money(Decimal)`; `.as_decimal() -> Decimal`,
  `Money::from_decimal(...)`. Value-based equality. JSON string, never float.

### The judgment-input gate-state question (record the interpretation)

The five `LOAD_BEARING_JUDGMENT_INPUTS` are stored as bare `Option<Money>` on `Judgment` — they have **no
review tag** (unlike the §2/§3 data cells, which got the tri-state in 2.5). So `judgment_to_gate_state` must
decide: a present judgment value is `ValidatedFresh` (the recommended reading — a deliberately-typed personal
judgment is the user's own validated number, not provider data awaiting sign-off; `None` → `Missing`). This
means the verdict's "all load-bearing inputs validated & fresh" gate is, in practice, **gated by the §2/§3 data
cells** (2.5's review markers) once the judgment values are entered. Record this as the chosen interpretation;
a future story could give judgment inputs their own review tag if the product wants the user to sign off on his
own judgments — out of scope here. (This is the cleanest mapping that makes the engine's existing
`derive_verdict` correct without a contract change.)

### Architecture compliance (guardrails)

- **Cardinal Rule:** no calculation in `app`; all math in `core` (`architecture.md:495,549-551,715-717`). 2.6
  maps + formats only.
- **Adapter rule:** `core`/`contract` domain types never enter `.slint`; money/ratios cross as already-formatted
  locale-aware strings; the verdict crosses as an enum-derived string; collections via `ModelRc`/`VecModel`
  (`architecture.md:517-520,580,683`).
- **Single construction path / coherent frame:** `StudySnapshot::new` once per frame; outputs + verdict from one
  snapshot — "an incoherent frame is structurally impossible" (`architecture.md:135-138,400-414`).
- **Verdict integrity:** `verdict.isFull ⟹ ∀ load-bearing input validated ∧ ¬stale` (`architecture.md:412-414,
  581`; UX `525-531`). Provisional = hatched + temporal provenance; Withheld = named open gates.
- **Provenance/clock:** judgment-edit provenance only from the injected `Clock` (ADD15,
  `architecture.md:528-531`).
- **Rounding:** the single named rounding mode + per-field display scale live in `core::rounding`, applied only
  at display — the app reads them, never hand-rounds (`architecture.md:356-359`).
- **Errors:** any failure (normalize error, save failure) is visible + neutral — never a swallowed
  `.ok()`/`.unwrap()` in non-test app code.
- **Performance:** NFR-P4 launch ~3 s; the value-driven recompute should feel instant (the snapshot is cheap).
  NFR-P1's <100 ms drag-recolour budget is **2.8** (chart drag), not 2.6.

### The §4 zone bar + verdict rendering spec (UX)

- **Zone bar** (UX `673-677,882-884`, mockup `ux-stock-study-screen.html:119-130,262-268`): one **vertical**
  Buy/Neutral/Sell bar (equal thirds of the forecast range), present-price marker, **price axis beside it** (the
  4 boundary prices top→bottom). NOT duplicated as text rows (the §4C range÷3 stays in the calc column for
  fidelity).
- **Zone colours** (UX `485-501`): Okabe-Ito hues already in `Tokens` — Buy `#009E73`, Hold `#E69F00`, Sell
  `#D55E00`. **Dark theme:** fill 32–40 % alpha (`zone-alpha` = 0.36) + 1.5–2 px full-saturation edge stroke.
  **Light theme:** 15–18 % alpha (0.165) + the same stroke. Redundant encoding = hue + value + vertical position
  (buy low → sell high) + the BUY/HOLD/SELL label. **Hold & Sell pushed apart on the value axis** (Hold lighter,
  Sell deeper) because their hues are close in luminance.
- **Verdict badge** (UX `888-889`): states `full colour / provisional (hatched + temporal provenance) /
  degraded / withheld`. **Full colour = full confidence** (UX `524-531`).
- **Sticky verdict bar** (UX `658-660,890-891`): pinned at the top of the scroll area — verdict + present price +
  projected return + appreciation (+ capital-at-risk, Epic-4). Always visible while scrolling/folding.
- **Two regimes** (UX `599-609,661-664`): entry = zone muted, grid-dominant; contemplation = zone **full**,
  chart+zones dominant, `✓`-green attenuated. Carried by the **colour/alpha token family only** (`regime-emphasis`),
  constant geometry, no re-layout.
- **Colour budget** (UX `476-481,243-250`): saturated colour ONLY on the three zones; everything else greyscale
  ink. The geofenced `✓`-green (`#4A7C6F`, 2.5) is the one exception and **never co-presents** with the zone
  bands.
- **Neutral voice** (UX `944-946,1005-1008`): factual nouns/facts, never imperative advice. The zone labels
  (ACHAT/NEUTRE/VENTE) are topology nouns, not commands; the verdict shows the surfaced criterion facts, never
  "buy this stock".

### Existing app code being modified / extended (read before writing)

- **`app/src/viewmodel/form.rs`** — currently maps `Study` → form structs with em-dash placeholders for every
  computed slot (`EMPTY_SLOT`, header method-identity string, `pe_rows`/`mgmt_rows`). 2.6 replaces the computed
  em-dashes with snapshot outputs (formatted strings); the editable §2/§3 cells (2.4) and review markers (2.5)
  are unchanged in shape. **Read the whole file** — it documents the "presentation only, nothing calculates"
  contract that 2.6 carefully keeps (the calculation is in `core`, the app only *formats* the result).
- **`app/src/viewmodel/engine.rs`** (recommended NEW) — the contract→core mapping (Task 1).
- **`app/src/viewmodel/format.rs`** — `format_amount` / `NumberFormat` / `parse_amount`; add a percent formatter
  and a ratio (`"3.4:1"`) formatter if not derivable, using `core::rounding` scales.
- **`app/src/viewmodel/entry.rs`** — the cell addressing / `coverage_str` / `review_str` helpers; add judgment-
  field addressing if needed.
- **`app/src/state.rs`** — the `JournalState` mutation rail (`mutate_cell`, `edit_cell`, `set_review`,
  `unlock_all`, `set_not_available`, `paste_column`, the `MSG_*` notices, `empty_judgment`). Add
  `set_judgment_field`, `set_forecast_low_option`, and `snapshot_for` (the engine-call site) on the same rail;
  add a `MSG_*` for a normalize failure. The doc-comment already says "content-addressed verdict is Story 2.6"
  — this is that story.
- **`app/src/main.rs`** — wire the judgment-edit + selector callbacks (validate → persist → re-read → recompute
  → re-push, the 2.3/2.4/2.5 one-source-of-truth shape) and the traceability open/close. Keep the injected
  `Clock`/`IdGen` the single time/identity source.
- **`app/src/theme.rs` + `app/ui/tokens.slint`** — the zone tokens (`zone_buy/hold/sell/alpha`) **already exist**
  (added in 2.1, awaiting 2.6 — a `zone_hues_are_identical_across_themes` test guards them). Add only a
  zone-edge-stroke width / a provisional-hatch metric if not derivable. **Never hard-code hex/px in `.slint`.**
- **`app/ui/state.slint`** — add the judgment-input fields + the zone-bar struct + the verdict struct +
  the traceability struct to the adapter; add the `set-judgment` / `set-forecast-low-option` / open-traceability
  callbacks on the `Studies` global; re-export new structs via `app.slint`.
- **`app/ui/components/zone_bar.slint`** (NEW) + **`app/ui/components/verdict_badge.slint`** (NEW) — the
  architecture-named components.
- **`app/ui/screens/study_screen.slint`** — mount the `ZoneBar` into the §4 placeholder region (the §1 chart
  placeholder stays — 2.8); add the judgment-input fields; wire the §2–§5 result slots to the new outputs; add
  the traceability entry point.
- **`app/ui/app.slint`** — add the sticky verdict bar (the named surface).
- **`app/src/posture.rs`** — scan the new strings; bump the floors.

### Previous-story intelligence (2.5 dev record + review; 2.4; 2.3; 1.8/1.11)

- **2.5 added** `trust_markers.slint`, the `validated-ink` token (both palettes, geofenced ≠ Buy green, theme
  test enforced), `set_review`/`unlock_all` on the mutation rail, the app's **first confirm-before-act overlay**
  (inline banner, Confirmer/Annuler — reuse its shape for the traceability popup), and the asymmetric
  attenuation (only `✓` dims in contemplation). App tests **82** after review.
- **Gates always `--locked`;** clippy `--all-targets --all-features` lints tests + the frozen `examples/spike_*.rs`
  (must keep compiling). Every story re-runs all four gates and re-diffs the pinned surfaces — expect the same.
- **`unused_crate_dependencies` is a crate-level allow** (2.2–2.5): `core`/`ingestion`/`report`/`tokio` have been
  *unused* in `app` so far. **2.6 finally uses `core`** — confirm the `app/Cargo.toml` dep on
  `steadyinvest-core` exists (the workspace declares it; verify `app` actually lists it) and the
  `unused_crate_dependencies` comment-of-record is updated for `core` becoming used. `ingestion`/`report`/`tokio`
  stay unused until Epic 3.
- **`unknown` never `0`** — the single most-repeated rail. Every engine output is `Option`; format `None` as
  `EMPTY_SLOT`. Never `unwrap_or(Decimal::ZERO)`.
- **One-source-of-truth render shape (2.3/2.4/2.5):** validate → mutate → `put_study` → re-read → recompute →
  re-push to Slint. The form rebuilds its rows on every persisted change — keep that; the recompute slots in at
  the re-read step (one `snapshot_for`).
- **Slint gotchas (2.2/2.3/2.4):** `row` is a reserved layout-attached property; `@children` in a conditional is
  illegal (fold via clipped height-0); element ids unreachable from a component-root function inside a
  conditional (declare functions on the in-branch layout); the 2.4 `editable_cell.slint` intercepts Ctrl/Cmd
  chords **first** in `key-pressed` — the new judgment `TextInput`s should follow the same commit-on-focus-out
  discipline and not collide with Ctrl+V / Ctrl+Space / Ctrl+Enter / Ctrl+Backspace (2.4/2.5 chords).
- **Visual-verification DoD is load-bearing; the sandbox blocks screenshots / may lack AT-SPI / headed render**
  — 2.1–2.5 all recorded a partial AC (process launches + on-disk + headless logic proven; in-GUI click-through
  left for human/AT-SPI). Plan the same honesty: prove the engine-wiring + verdict derivation + round-trip
  headlessly; record the zone-bar render / colour / badge texture as needing human confirmation.
- **File List completeness is the epic's single most-repeated finding (issue #18):** list **every** new/modified
  file (incl. any QA test file + the `_bmad-output/story-automator/…` automator log) with refreshed test counts
  **before** review.
- **The §4/§5 calc-row captions already exist** in `study_screen.slint` (the formula expressions) — reuse them
  for the traceability "rule" line; do not re-author the formulas.

### Git intelligence

Recent commits: `feat(story-2.5): Tri-state validation with soft-lock`, `feat(story-2.4): Manual data entry
with provenance & coverage`, `feat(story-2.3): Faithful collapsible SSG form …`, `feat(story-2.2): Create, save
& reopen a study …`. Conventions: conventional commits `feat(story-2.6): …`; the story file +
`sprint-status.yaml` update land in the **same** commit; merge only with all four gates green `--locked`. `app/`
structure (2.1–2.5): `clock.rs`, `config.rs`, `labels.rs`, `state.rs`, `theme.rs`, `regime.rs`, `posture.rs`,
`viewmodel/{format,studies,form,entry}.rs`, `ui/{tokens,state,app}.slint`, `ui/screens/*`, `ui/components/*`.
`core/`, `contract/`, `persistence/` must **not** change.

### Project Structure Notes

- **New (app-only):** `app/ui/components/zone_bar.slint`, `app/ui/components/verdict_badge.slint`;
  recommended `app/src/viewmodel/engine.rs` (the contract→core mapping); new `app` unit tests (the engine-wiring
  path, verdict-gate matrix, judgment round-trip, adapter formatting unknown→em-dash, snapshot-string, posture).
- **Modified:** `app/src/viewmodel/form.rs` (computed slots → outputs), `app/src/viewmodel/format.rs` (percent/
  ratio formatters), `app/src/viewmodel/entry.rs` (judgment addressing if needed), `app/src/state.rs`
  (`set_judgment_field`, `set_forecast_low_option`, `snapshot_for`, normalize `MSG_*`), `app/src/main.rs`
  (callbacks + traceability), `app/src/theme.rs` (edge-stroke/hatch metric if needed), `app/src/posture.rs`
  (floors), `app/ui/state.slint` (judgment fields + zone/verdict/traceability structs + callbacks),
  `app/ui/app.slint` (sticky verdict bar + re-exports), `app/ui/screens/study_screen.slint` (ZoneBar mount,
  judgment inputs, result slots, traceability entry), `app/ui/tokens.slint` (edge/hatch metric if needed),
  `sprint-status.yaml`, this story file.
- **Untouched (verify with `git diff` — must be empty):** `core/`, `contract/`, `persistence/`, `ingestion/`,
  `report/`, `docs/method/**`, `.github/workflows/ci.yml`, `rust-toolchain.toml`, the frozen
  `persistence/tests/corpus/v1.db`, `deny.toml`, and **`Cargo.lock`** (no new dependency — `core` already in the
  lock tree). **`contract/`/`core` are consumed, never modified.**
- **Variance note:** if the zone bar / verdict badge are rendered inline rather than as new component files, the
  `.slint` file floor differs — record the choice and set the floor to the actual count. `state.rs`'s full
  undo-stack slice remains 2.9 (a documented partial); the §1 chart remains 2.8; cell-level plausibility remains
  2.7.

### References

- Story & ACs: `_bmad-output/planning-artifacts/epics.md` § "Story 2.6: Numeric judgment inputs, verdict & zone
  bar (integrity-gated)" + Epic 2 intro (lines 532-536, 613-626)
- FR6 (judgment inputs), FR11 (traceability view), FR12 (verdict degraded/withheld), FR31 (exact-value path),
  FR8 (low-confidence), FR13 (neutral signals), FR64 (disclaimer), FR65 (offline):
  `_bmad-output/planning-artifacts/prd.md`
- §4 zone bar + verdict badge + sticky bar + verdict integrity + colour budget + two regimes + neutral voice:
  `_bmad-output/planning-artifacts/ux-design-specification.md` lines 243-250, 421-428, 476-501, 524-531,
  599-609, 658-664, 673-677, 882-891, 944-946, 1005-1008; mockup `ux-stock-study-screen.html` (zone bar +
  verdict bar markup, lines 37-42, 119-130, 153-159, 248-281)
- Cardinal Rule, adapter rule, verdict-integrity invariants, single construction path, clock injection,
  component file tree, performance budget, rounding policy: `_bmad-output/planning-artifacts/architecture.md`
  lines 67-86, 135-138, 356-359, 400-414, 495, 506-520, 528-531, 549-551, 577-586, 611-624, 682-701, 715-717
- The engine to call (consume, never modify): `core/src/verdict.rs` (`StudySnapshot::new`, `InputGates`,
  `YearGates`, `GateState`, `Verdict`, `derive_verdict`), `core/src/ssg/mod.rs` (`compute`), `core/src/ssg/
  types.rs` (`JudgmentInputs`, `ForecastLowOption`, `QuarterlyObservations`, `SsgOutputs`, `ZoneBounds`, `Zone`,
  `UpsideDownside`, `RiskRewardOutputs`, `ReturnOutputs`, `VerdictFacts`, the §1/§2/§3 output structs),
  `core/src/normalize/mod.rs` (`normalize`), `core/src/normalize/types.rs` (`RawFinancials`, `RawYear`,
  `RawAmount`, `YearUsability`, `CanonicalFinancials`, `CanonicalYear`), `core/src/method/mod.rs`
  (`LOAD_BEARING_YEAR_FIELDS`, `LOAD_BEARING_JUDGMENT_INPUTS`, the thresholds, `BANNED_VERBS_FR/EN`),
  `core/src/rounding.rs` (named rounding + display scale), `core/src/lib.rs` (`METHOD_VERSION`, the re-exports)
- The contract to consume (never modify): `contract/src/study.rs` (`Study`, `YearData`, `Judgment`,
  `ForecastLowOption`), `contract/src/cell.rs` (`Cell`, `Review`, `Freshness`, `Source`, `Coverage`,
  `Cell::edited`), `contract/src/money.rs` (`Money`/`as_decimal`), `contract/src/provenance.rs` (`Provenance`)
- The app rails to extend (2.4/2.5): `app/src/state.rs` (`mutate_cell`, `edit_cell`, `set_review`, `unlock_all`,
  `manual_provenance`, `empty_judgment`, the `MSG_*`/`USER_FACING_MESSAGES`), `app/src/viewmodel/form.rs`
  (`EMPTY_SLOT`, `header`, `pe_rows`, `mgmt_rows`, `editable_cell`/`GridCellState`), `app/src/viewmodel/
  format.rs` (`format_amount`, `parse_amount`, `NumberFormat`), `app/src/viewmodel/entry.rs`; the screen to
  extend: `app/ui/screens/study_screen.slint` (the §4 `PlaceholderRegion`, the §2/§3/§5 result slots, the calc-
  row formula captions); the globals/structs: `app/ui/state.slint`, `app/ui/app.slint`; tokens/theme:
  `app/ui/tokens.slint`, `app/src/theme.rs` (`zone_buy/hold/sell/alpha` already present); the confirm-overlay
  primitive to reuse for traceability: `app/ui/screens/study_screen.slint` (2.5 overlay)
- Prior stories: `2-5-tri-state-validation-soft-lock.md` (the review×freshness the gates read; the confirm
  overlay; attenuation), `2-4-manual-data-entry-provenance-coverage.md` (entry + provenance + `parse_amount`),
  `2-3-faithful-collapsible-ssg-form.md` (the §1–§5 structure + the em-dash placeholders 2.6 fills),
  `2-2-create-save-reopen-study.md` (issue #14 `Judgment` schema), the Epic 1 engine stories
  `1-7`/`1-8`/`1-11` (normalize / compute / verdict)
- File List discipline (issue #18); prior interpretation issues #19/#20/#21/#22; banned verbs:
  `core::method::BANNED_VERBS_EN/FR`; memories [[project_high_fidelity_ssg_forms]],
  [[project_open_source_naming_constraint]], [[project_gui_framework_slint_only]], [[project_linux_only_for_now]]

### Tech currency note (2026-06-13)

Slint pinned at 1.16; `rust_decimal` 1.42 (+`maths`) already in the tree. **No new external dependency** in 2.6
— it wires `core` (already a workspace member, already in `Cargo.lock`) into `app`. **`Cargo.lock` and
`deny.toml` are expected unchanged.** The zone bar is drawn with Slint primitives (`Rectangle` segments +
border strokes; a hatch via a repeating pattern or a clipped overlay for the provisional texture); no charting
crate (the §1 chart with `Path`/`TouchArea` is 2.8). Verify Slint's border-stroke + sticky-pinned-row patterns
in a quick check before wiring (reuse the 2.1 footer / 2.5 overlay pinning). The engine call is synchronous and
cheap (pure `Decimal` math) — the value-driven recompute is well within NFR-P4; the <100 ms drag budget is 2.8.

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8 (1M context) — dev-story workflow (2026-06-14).

### Debug Log References

- All four gates green `--locked`: `cargo fmt --all --check` (clean) · `cargo clippy --all-targets
  --all-features --locked -- -D warnings` (clean) · `cargo test --all --locked` (all pass) · `cargo deny check`
  (advisories/bans/licenses/sources ok).
- App test count: **82 → 95** (+13: engine wiring matrix, judgment round-trip + clear-to-None, `snapshot_for`
  no-drift, adapter formatting unknown→em-dash, `format_scaled`, posture engine-label gate).
- Slint `z` reserved-property clash on `ZoneBar`'s input property — renamed to `bar`.
- `clippy --all-targets` builds the binary without `cfg(test)`, so `state::snapshot_for` was dead until routed
  through the production traceability-open callback (the single engine-call site).
- Pinned-surface `git status` empty for `core/ contract/ persistence/ ingestion/ report/ docs/method/ .github/
  rust-toolchain.toml deny.toml Cargo.lock` — **`Cargo.lock` unchanged, no new dependency** (`core` was already
  an `app` dep; 2.6 only adds the first *engine* call).

### Completion Notes List

- **Engine-wiring adapter** (`app/src/viewmodel/engine.rs`, NEW): `to_raw_financials` / `to_judgment_inputs`
  (by-name option glue) / `to_observations` (empty v1) / `to_input_gates` (one `YearGates` per *usable* year +
  the 5 judgment gates) / `cell_to_gate_state` / `judgment_to_gate_state`; `build_snapshot` runs the SINGLE
  construction path (`normalize` → `StudySnapshot::new` once). Output adapters format `SsgOutputs` → already-
  grouped locale strings (`None` → em-dash, never `0`), the §4 `ZoneBarState` (+ a layout-only normalized marker
  float), the `VerdictState` (badge + sticky bar), and the `TraceState`. **No calculation in `app`** — every
  number comes from the snapshot; the app maps + formats.
- **`state.rs`**: `set_judgment_field` / `set_forecast_low_option` on the mutation rail (re-read → set one
  `Judgment` field → `put_study`, reusing the read-only/no-journal/save-failure guards; a cleared field is
  `None`, never `0`), and `snapshot_for(study_id)` — the engine-call site (`NormalizeError` → the neutral
  `MSG_NORMALIZE_FAILED`, never `unwrap`/`.ok()`).
- **Wired the previously-em-dash slots**: §1 CAGR, §2 PTP/ROE per-year + averages + trends, §3 D/E/G/H + summary,
  §4 forecast high/low + U/D + appreciation + the **zone bar**, §5 yields + total return — all formatted strings,
  unknown → em-dash. The §2 per-year cells align to the materialized years so the grid never misaligns.
- **New Slint components**: `zone_bar.slint` (vertical Buy/Neutral/Sell thirds at `zone-alpha` + edge strokes +
  price axis + present-price marker; muted-in-entry → full-in-contemplation via `regime-emphasis`; saturation
  gated by verdict confidence — full bands only for `Full`, hatched/desaturated for `Provisional`, calm empty
  state for `None`-zones/`Withheld`), `verdict_badge.slint` (`VerdictBadge` texture+label, never colour alone;
  `VerdictBar` the sticky bar pinned at the top of the study scroll area), `judgment_field.slint` (exact-value
  entry, commit-on-Enter/focus-out, re-seed on re-push).
- **Traceability** (AC 7): a centred overlay opened from the verdict bar naming inputs → provenance → rule
  (`METHOD_VERSION` + formula caption) + the open gates ("why not full"). No colour spent.
- **Posture/a11y**: registered the Rust-side labels in `engine::USER_FACING_LABELS` + the new `MSG_NORMALIZE_FAILED`
  in `state::USER_FACING_MESSAGES`; bumped the floors (`.slint` files ≥ 18, `@tr` literals ≥ 130, message count
  14, engine-label count 21). Verdict/zone wording is fact-stating; the zone nouns come from the `Labels` table
  (banned-verb-exempt). Decisions carried by hue + value + vertical position + text label (never colour alone);
  judgment fields have visible focus rings + keyboard entry; the forecast-low selector is keyboard-operable.
- **Visual verification (AC 8 — load-bearing, mirrors 2.1–2.5)**: the built app **launches successfully against
  the session display** (`DISPLAY=:0`/Wayland) and runs the event loop with no crash. The sandbox cannot capture
  screenshots or drive AT-SPI clicks, so the in-GUI click-through (entering judgment values → observing the zone-
  bar bands/marker/axis, the entry-muted vs contemplation-full swap, the verdict badge flipping Full ↔
  Provisional-hatched ↔ Withheld, the sticky bar staying pinned, the traceability overlay) is left for human
  confirmation. Proven HEADLESSLY (the 95 tests): the judgment-entry → `put_study` → reopen round-trip, the full
  engine-wiring path (`Study`→`Raw`→`normalize`→`compute`→`snapshot`, outputs matching a direct `compute()`), the
  adapter formatting (unknown → em-dash, never `0`), and the verdict-integrity derivation (Full/Provisional/
  Withheld across the gate matrix incl. `low_confidence`).
- **Interpretations** filed as GitHub issue **#23** (judgment gate-state mapping, empty `QuarterlyObservations`,
  per-field display scale, the §4 selector control, traceability surface pattern, sticky-bar pinning, the
  forwarded zone-colour CI gate, …).

### File List

**New (app-only):**
- `app/src/viewmodel/engine.rs` — the contract→core mapping + the core→Slint output/zone/verdict/trace adapters + tests
- `app/ui/components/zone_bar.slint` — the §4 Buy/Neutral/Sell zone bar
- `app/ui/components/verdict_badge.slint` — `VerdictBadge` + the sticky `VerdictBar`
- `app/ui/components/judgment_field.slint` — the labelled exact-value judgment-input field

**Modified:**
- `app/src/viewmodel/mod.rs` — register the `engine` module
- `app/src/viewmodel/format.rs` — `format_scaled` (reads `core::rounding` scale) + test
- `app/src/viewmodel/form.rs` — `pe_rows` fills D/E/G/H from outputs; `materialized_year_numbers`
- `app/src/state.rs` — `set_judgment_field`, `set_forecast_low_option`, `snapshot_for`, `MSG_NORMALIZE_FAILED`, `apply_judgment_field` + tests
- `app/src/main.rs` — `push_form` computes the snapshot + pushes the engine outputs/zone/verdict; the set-judgment / set-forecast-low-option / open-/close-traceability callbacks
- `app/src/posture.rs` — scan `engine::USER_FACING_LABELS`; bump the floors (`.slint` ≥ 18, `@tr` ≥ 130, messages 14, labels 21)
- `app/ui/state.slint` — the judgment/computed/zone/verdict/trace structs + the new `Studies` properties & callbacks
- `app/ui/app.slint` — re-export the new structs
- `app/ui/screens/study_screen.slint` — judgment inputs (§1/§3/§4), wired §1–§5 result slots, mounted `ZoneBar`, the sticky `VerdictBar`, the traceability overlay
- `_bmad-output/implementation-artifacts/sprint-status.yaml` — 2-6 ready-for-dev → in-progress → review
- `_bmad-output/implementation-artifacts/2-6-numeric-judgment-inputs-verdict-zone-bar.md` — this story file
- `_bmad-output/story-automator/orchestration-2-20260612-123914.md` — automator log (issue #18 discipline)

## Senior Developer Review (AI)

**Reviewer:** Guy · **Date:** 2026-06-14 · **Outcome:** Approved (changes requested were auto-fixed)

Adversarial review of the full File List against the implementation. The engine-wiring is sound: the
single construction path (`engine::build_snapshot` → one `StudySnapshot::new`) is the only compute site,
no calculation leaks into `app`, every engine output is an `Option` rendered as `EMPTY_SLOT` (never `0`),
the `contract → core` glue is by-name (no `as`-cast), and the verdict gate matrix
(Full/Provisional/Withheld + `low_confidence`) is exercised headlessly. All four gates green `--locked`
(fmt · clippy · test · deny); pinned surfaces (`core/ contract/ persistence/ … Cargo.lock deny.toml`)
diff-empty; **no new dependency**. The File List matches `git` exactly (issue #18 discipline held).

Two verified findings — both auto-fixed during review:

- **[MEDIUM · AC 6 fidelity]** `engine::verdict_badge` set the temporal-provenance caption
  ("Calculé à partir des données du DD/MM") for **every non-Full** verdict, so a **Withheld** verdict
  rendered "computed from data of DD/MM" beside a suspended conclusion. AC 6 reserves that caption for
  **Provisional** (Withheld's honest surface is its named open gates). **Fix:** the provenance date is
  now emitted only for `Verdict::Provisional` (`engine.rs`); Full and Withheld carry no caption.
- **[MEDIUM · overclaimed completion]** Task 5 and Task 6 each marked `[x]` a headless adapter test
  (the `verdict_badge` state/fact/provenance mapping; the `verdict_trace` inputs/rule/open-gates
  mapping) that did **not** exist — the features were implemented but untested. **Fix:** added
  `verdict_badge_maps_state_and_restricts_provenance_to_provisional` and
  `verdict_trace_names_inputs_rule_and_open_gates_for_a_degraded_verdict` (`engine.rs`), which also pin
  the first fix. App tests 95 → **97**.

No CRITICAL issues. The visual-verification DoD (AC 8) remains load-bearing and sandbox-blocked exactly
as recorded for 2.1–2.5 (in-GUI click-through left for human/AT-SPI confirmation); the engine-wiring,
round-trip and verdict-integrity paths are proven headlessly.

### Change Log

- **2026-06-14** — Senior Developer Review (AI): approved with two MEDIUM findings auto-fixed —
  (1) the temporal-provenance caption is now Provisional-only (Withheld no longer shows "computed from
  data of …", AC 6); (2) added the two missing Task 5/Task 6 verdict-adapter tests. App tests 95 → 97;
  all four gates green `--locked`, pinned surfaces unchanged. Status → done; sprint-status 2-6 → done.
- **2026-06-14** — Story 2.6 implemented (status → review): engine-wiring adapter (`viewmodel/engine.rs`) + the
  single `StudySnapshot::new` path (`state::snapshot_for`); numeric judgment-input editing → persist → recompute;
  the §2/§3/§4/§5 result slots wired to the engine's `SsgOutputs` (unknown → em-dash, never `0`); the §4 zone bar
  (first saturated-colour spend), U/D + projected return, the verdict badge (Full/Provisional-hatched/Withheld),
  the sticky verdict bar, and the traceability view. All four gates green `--locked`; `Cargo.lock`/`deny.toml`/
  pinned surfaces unchanged (no new dependency). App tests 82 → 95. Interpretations filed as GitHub issue #23.
- **2026-06-13** — Story 2.6 drafted (ready-for-dev) by create-story: the engine-wiring story (first `app`→`core`
  call); numeric judgment entry + `StudySnapshot` wiring + §4 zone bar (first saturated-colour spend) + U/D +
  projected return + verdict badge (Full/Provisional/Withheld) + sticky verdict bar + traceability view; all
  required `core`/`contract` API verified present (no schema change); `Cargo.lock`/`deny.toml`/pinned surfaces
  expected unchanged.
