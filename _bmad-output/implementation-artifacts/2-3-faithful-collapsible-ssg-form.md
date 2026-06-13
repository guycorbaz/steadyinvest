# Story 2.3: Faithful collapsible SSG form (§1–§5)

Status: done

<!-- Epic-2 story 3 — the FIRST story that renders the recognizable SSG study form. Story 2.1 built
     the shell; story 2.2 wired persistence (create → save → list → reopen) and deliberately shipped a
     "faithful-but-minimal restore view" (a title + a few text lines) with the explicit note: "full SSG
     form rendering is 2.3". THIS story replaces that placeholder restore view with the faithful,
     collapsible §1–§5 form: header + capitalization block, the A–H lettered columns with their
     formula captions, the §3 P/E table, §4/§5 calc rows, on a visible cell grid; each section
     individually collapsible with an information-scent summary when folded (fold + regime state
     persisted); two regimes (entry ↔ contemplation) expressed as fold presets + a colour/marker
     token delta on CONSTANT GEOMETRY. SCOPE GUARDRAIL — 2.3 builds the faithful STRUCTURE only:
     NO engine/compute call, NO verdict, NO zone bar (2.6); NO §1 interactive chart (2.8); NO data
     entry / paste-a-column / provenance display (2.4); NO tri-state validation markers / soft-lock
     (2.5). Derived columns, the §1 chart and the §4 zone bar are explicit faithful PLACEHOLDERS that
     2.4/2.5/2.6/2.8 fill. Headless CI cannot prove the form renders: the visual-verification DoD
     (AC 6) is load-bearing, exactly as it was for 2.1 and 2.2. -->

## Story

As Guy,
I want the recognizable high-fidelity SSG form with collapsible §1–§5 sections and the two reading
regimes,
so that I am never disoriented and can read the study at a glance.

## Acceptance Criteria

1. **The faithful form renders for an open study (FR2 display, high-fidelity SSG).** Opening a saved
   study (the `Journal::get_study(id)` path already wired in 2.2) renders the **recognizable SSG
   study form** in place of the 2.2 minimal restore view:
   - a **non-collapsible header** = study identity (security ticker · native currency · created-at ·
     `method_version`) **+ a capitalization block** (the faithful header table region; for v1 the
     capitalization fields the contract does not yet carry render as faithful empty/`—` cells — the
     *block exists* and is recognizable, it is not populated from non-existent data);
   - **five sections §1–§5** in the canonical SSG order with neutral French titles (no NAIC
     wordmark/logo/verbatim prose — neutral labels only, see Dev Notes § "Neutral voice");
   - **§3 renders the A–H lettered P/E table** with the **lettered column boxes (A…H) and their
     formula captions** (e.g. column D caption `A÷C`, G caption `F÷C×100`, H caption `F÷B×100`) on a
     **visible cell grid** (1 px borders per [[project_high_fidelity_ssg_forms]]);
   - **§2 renders the management grid** (PTP / ROE rows × year columns + average + trend columns);
   - **§4 and §5 render their calc rows** — the **row labels and the formula expressions** (e.g. §4
     `A · Prix haut = PER haut moy. × BPA est. haut`, §5 `Rendement annuel total = appr.% ÷ 5 + rdt
     moyen`) with a faithful **boxed result slot** per row.

   A familiar SSG user recognizes the form at a glance. The form renders correctly for the **empty
   study** (the only kind that exists until 2.4 adds data entry): present `YearData` cells show their
   raw value formatted as a string; absent cells render as faithful empty/`—` slots — **never `0`,
   never a crash, never a blank screen**.

2. **§1–§5 are individually collapsible with an information-scent summary when folded; fold state
   persists (FR56).** Each section has a fold header (chevron + number + title) and toggles
   open/closed independently. **When a section is collapsed it shows a one-line, fact-stating
   information-scent summary** beside its title (a folded section still tells the reader its key
   figures — e.g. §3 folded → `PER moy — · courant —` while empty, populated once data/engine land).
   **Fold state is persisted** in app-config (ADD7: app-config, **never** the journal/contract) and
   **restored on reopen**, per study (the active regime and per-section open/closed flags). The
   summary is a neutral fact line, not advice (posture-gated).

3. **Two regimes (entry ↔ contemplation) as fold presets + a colour/marker delta on CONSTANT
   GEOMETRY (FR56).** A regime toggle (clearly indicating the active regime) switches the study
   between:
   - **entry (Saisie):** fold preset = **all sections open** (filling/reading every cell);
   - **contemplation:** fold preset = **§1 + §4 open, §2/§3/§5 collapsed** (judgment-moment focus).

   Switching regime **applies the fold preset and swaps a regime-driven colour/marker token snapshot
   — and changes NO geometry**: row heights, font sizes and column widths are identical in both
   regimes (the switch is perceived as a lighting/fold change, never a re-layout). 2.3 establishes
   the **regime token-swap mechanism + the fold presets + the active-regime indicator**; the full
   marker attenuation (2.5) and full zone saturation (2.6) hang off this mechanism later (Dev Notes
   § "Regimes — exactly what 2.3 ships"). The active regime is persisted per study (AC 2) and
   restored on reopen.

4. **Crate-boundary & adapter discipline (architecture Cardinal Rule).** **No calculation in `app`,
   no `core::ssg::compute` call, no `contract::Judgment → core::JudgmentInputs` mapping** (that
   mapping + the engine call are Story 2.6 — building them here is dead code under `clippy -D
   warnings` and steals 2.6's scope). Every value crossing into Slint is an **already-formatted,
   locale-aware string** via the 2.1 `viewmodel::format` helper — **never an `f32`/`f64`/`Decimal`**.
   Formula captions (`A÷C`, …) are **static display text**, not computed. All colours/sizes come from
   the `Tokens` global — **no hard-coded hex/`px` in `ui/`** (the 2.1 governance rule). New `.slint`
   files snake_case, components PascalCase, properties/callbacks kebab-case, globals PascalCase.

5. **Quality gates, posture & pinned-surface discipline.** All four gates green `--locked`:
   `cargo fmt --all --check` · `cargo clippy --all-targets --all-features --locked -- -D warnings` ·
   `cargo test --all --locked` · `cargo deny check`. Specifically:
   - **every new user-visible string** (section titles, column/row labels, info-scent summaries,
     regime labels, the header field labels) passes the crate-local **banned-verb posture test** —
     register them in the scanned `USER_FACING_MESSAGES` slices and/or as `@tr()` literals; reuse
     `core::method::BANNED_VERBS_EN/FR`, never copy (watch French nouns vs imperatives — *« achat /
     vente »* as **zone-band nouns** pass, imperatives *« acheter / vendre »* do not);
   - **new keyboard-operable controls** (fold headers, regime toggle) follow the 2.1 a11y pattern:
     `FocusScope` + Enter/Space activation + **visible focus ring** (NFR-U1/U2), and **decision is
     never colour-only** — fold state shows a chevron glyph + the summary line, regime shows a text
     label, not colour alone;
   - **pinned surfaces untouched:** `core/`, `contract/`, `persistence/`, `ingestion/`, `report/`,
     `docs/method/**`, `.github/`, `deny.toml`, `rust-toolchain.toml`, and the frozen
     `persistence/tests/corpus/v1.db` are **not modified by this story** — 2.3 is **`app`-only**
     (UI + viewmodel + config field). `git diff` over those paths must be empty. **No new external
     crate** is expected (`Cargo.lock` delta should be empty; if a fold-state map needs nothing new,
     it adds nothing — record the delta precisely either way).

6. **Visual verification (Definition of Done — load-bearing, retro §3.4 / mirrors 2.1 & 2.2).**
   Launch the built app, create or open a study, and verify on display: the **faithful §1–§5 form
   renders** (header + capitalization block, §3 A–H table with formula captions on a visible grid,
   §2 grid, §4/§5 calc rows); **each section folds/unfolds** showing its info-scent summary when
   collapsed; the **regime toggle** switches fold presets **with no layout jank** (constant
   geometry); **close → relaunch → reopen the same study → the fold + regime state is restored**.
   Confirm the footer disclaimer (FR64) still shows, dark/light + label-set + locale swaps (2.1)
   still work, and launch-to-interactive stays ~within 3 s (NFR-P4). Record the run in the Dev Agent
   Record. Headless CI cannot stand in for this AC.

## Tasks / Subtasks

- [x] **Task 1 — Reusable collapsible section component + fold-state plumbing (AC: 2)**
  - [x] Add `app/ui/components/collapsible_section.slint`: a `CollapsibleSection` component with
        `in property <string> title`, `in property <string> number`, `in property <string>
        fold-summary`, `in property <bool> open`, `callback toggled(bool)`, and `@children` body.
        Header = chevron glyph (rotates/changes on open) + number + title; when **closed**, show the
        `fold-summary` line (right-aligned, `Tokens.text-low`, numeric font). `FocusScope` +
        Enter/Space toggles; visible focus ring (`Tokens.focus` / `Tokens.focus-border`). All
        colours/sizes from `Tokens`; no hard-coded hex/px. Constant geometry: header height fixed.
  - [x] Extend `AppConfig` (`app/src/config.rs`) with a `#[serde(default)]` per-study view-state
        field — recommended `study_view_state: BTreeMap<String, StudyViewState>` keyed by study-id
        string, where `StudyViewState { regime: Regime, folds: [bool; 5] }` (both `#[serde(default)]`,
        `Regime` defaults to `Entry`). Append-only forward-extensibility exactly as 2.2 added
        `journal_path`; add a unit test that an **old config without the field** still loads and the
        field defaults empty. (If per-study map bookkeeping is judged out of scope, a **documented
        partial** — a single global last-used `regime` + fold preset — is acceptable, but FR56/UX want
        per-study; prefer the map. Record the choice in the Dev Agent Record.)

- [x] **Task 2 — Faithful form layout: header + capitalization block + §1–§5 scaffold (AC: 1, 4)**
  - [x] Add `app/ui/screens/study_screen.slint` (or `components/ssg_form.slint` — follow the
        architecture tree, which names `app/ui/study_screen.slint`): the faithful form. Non-collapsible
        header (identity fields + capitalization block table region) + five `CollapsibleSection`
        instances §1–§5 in canonical order with neutral French titles.
  - [x] **§3 A–H P/E table:** a visible-grid table with lettered column boxes A…H and **formula
        captions** under the computed-column headers (D `A÷C`, E `B÷C`, G `F÷C×100`, H `F÷B×100`);
        5 year rows + the summary rows (Total / Moyenne / PER moyen / PER courant) as faithful row
        labels with empty/`—` value slots until 2.4/2.6 fill them. Right-aligned tabular figures
        (`Tokens.font-numeric`), 1 px cell borders.
  - [x] **§2 management grid:** PTP and ROE rows × year columns + "Moy. 5 ans" + "Tendance" columns,
        same visible-grid styling, formula captions in fine print.
  - [x] **§4 / §5 calc rows:** a `calc-row` layout (label · formula expression · boxed result slot)
        for the §4 rows (A forecast high, B(a)–(d) + selected forecast low, C zonage ÷ 3, D U/D ratio,
        E appréciation) and the §5 rows (rendement courant, rendement moyen 5 ans, appréciation
        annualisée, rendement annuel total). Result slots render empty/`—` (engine is 2.6).
  - [x] **§1 chart area + §4 zone bar = faithful PLACEHOLDERS** (a calm, fact-stating "graphique
        disponible prochainement"-style neutral placeholder region on constant geometry; the
        interactive chart is 2.8 and the zone bar is 2.6). Do NOT draw a fake chart or fake zones.

- [x] **Task 3 — Viewmodel form adapter: `Study` → Slint form structs (AC: 1, 4)**
  - [x] Add `app/src/viewmodel/form.rs` (or extend `viewmodel/studies.rs`): map a `contract::Study`
        into the Slint form structs — header fields, the §2/§3 grid cell strings, §4/§5 calc-row
        result strings, and the **per-section info-scent summary strings**. Present `Cell.value`
        renders via `format::format_amount(...)` (string, never float); absent/`None` →
        faithful empty/`—`; **`unknown` is never rendered as `0`** (architecture rule, retro rail).
  - [x] Define the Slint structs (e.g. `FormHeader`, `GridRow`, `CalcRow`, `SectionState`) in
        `app/ui/state.slint` and re-export via `app.slint` (the 2.1/2.2 globals pattern). Money/decimal
        fields are `string` on the Slint side — the adapter is the only place values become strings.
  - [x] Register all new viewmodel user-facing label constants in a `USER_FACING_MESSAGES` slice so
        the posture test (Task 6) scans them.

- [x] **Task 4 — Two regimes: toggle, fold presets, colour/marker token-swap, constant geometry (AC: 3)**
  - [x] Add a `Regime { Entry, Contemplation }` enum (`app/src/` — likely `theme.rs` or a small
        `regime.rs`; dev discretion, follow the tree) and a **regime-driven token snapshot swap**: a
        small set of regime tokens on `Tokens` (or a sibling global) that the form reads, so 2.5
        (marker attenuation) and 2.6 (zone saturation) can hang their deltas on it. In 2.3 the visible
        delta is the **fold preset + the regime indicator** (and any subtle surface/emphasis token); do
        NOT invent marker/zone visuals here (those are 2.5/2.6).
  - [x] Add the regime toggle control in the study-screen top region (reuse the `ChoiceChip` /
        segmented pattern from 2.1 settings; keyboard-operable, visible focus, active state shown by
        weight/ink step + label, **not colour alone**). Toggling applies the fold preset to the five
        `CollapsibleSection`s and swaps the regime token snapshot.
  - [x] **Constant-geometry guard:** assert (by construction + a documented manual check) that row
        height, font sizes and column widths are token-static across regimes — only colour/alpha and
        fold/open state change. Reuse the existing Tokens split (colour/alpha swappable vs metric/typo
        quasi-static).

- [x] **Task 5 — Wire into the study/dashboard screen; persist + restore fold/regime (AC: 1, 2, 3)**
  - [x] Replace the 2.2 minimal restore view in `app/ui/screens/dashboard.slint` (the
        `Studies.detail-title` / `detail-body` Rectangle) with the new faithful form, fed by the new
        form structs. Keep the dashboard list + create flow from 2.2 intact.
  - [x] In `app/src/main.rs`, on `open-study`: build the form structs via the Task-3 adapter, **read
        the persisted `StudyViewState`** (regime + folds) for that study id from `AppConfig` (default
        = Entry + entry preset for a never-opened study) and push it into the form. Wire callbacks:
        `toggle-fold(section-index, open)` and `set-regime(regime)` → update `AppConfig.study_view_state`
        → `persist()` (mirror the 2.2 `Prefs` callback persistence pattern exactly: validate, mutate,
        persist, no silent `.ok()`).
  - [x] Keep `main.rs` allow-scopes honest (no new genuinely-used dep should stay under
        `#![allow(unused_crate_dependencies)]`; `ingestion`/`report`/`tokio` remain unused until Epic 3).

- [x] **Task 6 — Posture, accessibility & gates (AC: 5)**
  - [x] Extend the `app` posture test (`app/src/posture.rs`) to scan the new `.slint` `@tr()`
        literals (section titles, column/row labels, regime labels, info-scent summary templates,
        header field labels) and the new viewmodel/`USER_FACING_MESSAGES` slices against
        `BANNED_VERBS_FR/EN`. Bump the asserted minimum counts to cover the new strings.
  - [x] Keyboard walkthrough by construction: every fold header and the regime toggle are
        `FocusScope`-wrapped, Enter/Space-operable, with a visible focus ring; tab order is logical
        top-to-bottom; the form is readable with colour stripped (chevron glyph + summary line +
        regime label carry meaning, never colour alone).
  - [x] All four gates green `--locked`. `git diff` over `core/ contract/ persistence/ ingestion/
        report/ docs/method/ .github/ deny.toml rust-toolchain.toml` and the frozen `v1.db` is
        **empty**. Record the `Cargo.lock` delta (expected: none).

- [x] **Task 7 — Visual verification, records & File List (AC: 6)**
  - [x] Launch, walk the AC-6 journey (open study → §1–§5 render faithfully → fold/unfold with
        info-scent summaries → regime toggle with no jank → **relaunch → fold/regime restored**),
        record the outcome (and any sandbox screenshot/AT-SPI limitation, as 2.1/2.2 did) in the Dev
        Agent Record.
  - [x] Update the **File List** (every new/modified file incl. any QA-generated test file and the
        automator log — issue #18 discipline) and refresh test counts in the Change Log.
  - [x] Record interpretations (fold-state granularity choice, placeholder copy, any §1/§4 placeholder
        decisions) in the Dev Agent Record; file a consolidated GitHub issue for any real deferred
        interpretation (the 1.11/2.1/2.2 pattern — issues, not inline TODOs).

## Dev Notes

### What this story is — and the disasters it must make impossible

2.3 is the **first faithful rendering of the SSG study form**. Story 2.2 explicitly shipped a
placeholder restore view and wrote "full SSG form rendering is 2.3" — **this story cashes that
cheque**. It is a **UI-structure** story: the faithful §1–§5 layout, collapsibility with
information-scent summaries, fold/regime persistence, and the two reading regimes on constant
geometry. It is **`app`-only**.

Disasters to prevent:
- **Scope bleed into 2.4/2.5/2.6/2.8.** The single biggest risk. 2.3 draws the **bones**, not the
  flesh:
  - **NO `core::ssg::compute` call, NO `contract::Judgment → core::JudgmentInputs` mapping** — those
    are **Story 2.6**. Building them here is dead code under `-D warnings` and steals 2.6's job.
    (2.2's Dev Notes say this verbatim: "No `contract::Judgment` → `core::JudgmentInputs` engine
    mapping, no `compute` call (2.6).")
  - **NO data entry / editable cells / paste-a-column / provenance (source × freshness × coverage)
    display** — that is **Story 2.4**. 2.3 cells are **read-only display slots** showing raw stored
    values where present.
  - **NO tri-state validation markers (✓/?) / soft-lock** — **Story 2.5**.
  - **NO verdict badge / zone bar / U-D ratio / projected return** — **Story 2.6** (the §4 zone bar is
    a faithful placeholder here).
  - **NO interactive §1 chart / draggable judgment line / live recolor** — **Story 2.8** (the §1 chart
    area is a faithful placeholder here).
- **Calculation in `app`.** Cardinal Rule: all SSG math lives in `core` and nowhere else. 2.3 shows
  **static formula captions** (`A÷C`) and **raw stored values formatted as strings** — it does not
  compute D/E/G/H, forecasts, zones, returns, or the verdict. Keep `app` calc-free.
- **Floats/Decimals crossing into Slint.** Slint has no `Decimal`; money/ratios cross as
  **already-formatted strings** via `viewmodel::format` only. Never pass an `f64` or `rust_decimal`.
- **`unknown` rendered as `0`.** A missing/absent cell is an empty/`—` slot, never `0` (the prior
  project's blank-chart class of bug; architecture + 2.2 retro rail).
- **Geometry change on regime/fold switch (layout jank).** Regime swaps **colour/fold state only**;
  row heights, fonts, column widths are token-static. The Tokens already split colour/alpha
  (swappable) from metric/typo (quasi-static) — use that split; do not size anything off the regime.
- **Fold/regime state written into the journal or contract.** UI view-state is **app-config**
  (ADD7: app-config strictly local & per-machine), **never** the journal SQLite or `contract::Study`
  (which is an immutable, versioned domain snapshot — polluting it would be a schema disaster). Add a
  `#[serde(default)]` field to `AppConfig`, exactly as 2.2 added `journal_path`.
- **NAIC marks / verbatim prose / banned verbs in the new labels.** See § "Neutral voice".

### Scope — the one-paragraph contract

> 2.3 renders the **faithful, collapsible §1–§5 SSG form structure** for an open study, on a visible
> cell grid, with the A–H lettered columns and their **formula captions**, the §2/§3 grids and §4/§5
> **calc-row labels**; sections fold/unfold with an **information-scent summary** and the fold +
> regime state **persists per study in app-config**; the **two regimes** are **fold presets + a
> colour/marker token-swap on constant geometry**. It calls **no engine**, enters **no data**, shows
> **no markers, no verdict, no zone bar, no interactive chart** — those are 2.4/2.5/2.6/2.8. Cells
> display **raw stored values as strings** where present, faithful empty/`—` slots elsewhere.

### The five sections — neutral titles, content scaffold, and which contract fields feed them

Render in this canonical order. Titles are illustrative neutral French (final copy = dev's, must pass
posture). Each section is a `CollapsibleSection` with an info-scent summary line shown when folded.

- **§1 — Analyse visuelle des ventes, bénéfices & prix.** Houses the **semi-log growth chart
  (PLACEHOLDER in 2.3 — chart is 2.8)** + the four growth-metric slots (sales CAGR, EPS CAGR,
  projected sales/EPS growth) as empty/`—` until 2.6/2.8. Folded summary template:
  `Croissance BPA est. — · ventes —`.
- **§2 — Évaluation de la gestion.** Management grid: rows **PTP** (`Bén. av. impôt ÷ ventes`) and
  **ROE** (`BPA ÷ valeur comptable/action`) × year columns + "Moy. 5 ans" + "Tendance". Feeds (when
  data lands in 2.4): `YearData.pre_tax_profit`, `YearData.sales`, `YearData.eps`,
  `YearData.book_value_per_share` (all `Option<Cell>`/`Cell`). Folded summary: `Marge — · ROE —`.
- **§3 — Historique cours / bénéfices.** The **A–H lettered P/E table** on a visible grid:
  | letter | column | formula caption | contract source |
  |---|---|---|---|
  | A | Haut | (direct) | `YearData.high_price: Cell` |
  | B | Bas | (direct) | `YearData.low_price: Cell` |
  | C | BPA | (direct) | `YearData.eps: Cell` |
  | D | PER haut | `A÷C` | computed (2.6) — caption only here |
  | E | PER bas | `B÷C` | computed (2.6) — caption only here |
  | F | Div./action | (direct) | `YearData.dividend_per_share: Option<Cell>` |
  | G | % Distribution | `F÷C×100` | computed (2.6) — caption only here |
  | H | % Rdt haut | `F÷B×100` | computed (2.6) — caption only here |
  5 year rows + summary rows (Total / Moyenne / PER moyen / PER courant). Folded summary:
  `PER moy — · courant —`.
- **§4 — Évaluation du risque & de la récompense — 5 ans.** Calc rows (label · formula · boxed result
  **slot**): `A · Prix haut = PER haut moy. × BPA est. haut`; `B(a) PER bas moy. × BPA est. bas`;
  `B(b) Prix bas moyen 5 ans`; `B(c) Plus bas sévère récent`; `B(d) Soutenu par dividende`;
  `B · Prix bas prévu (sélectionné)`; `C · Zonage — fourchette ÷ 3`; `D · Ratio hausse/baisse`;
  `E · Appréciation potentielle`. The **zone bar + price axis = PLACEHOLDER (2.6)**. Judgment inputs
  that feed §4 live in `contract::Judgment` (`estimated_high_eps`, `estimated_low_eps`,
  `judged_avg_high_pe`, `judged_avg_low_pe`, `forecast_low_option`, `recent_severe_low`,
  `current_price`, `present_full_year_dividend`) — **not entered or computed here**. Folded summary:
  `Zone — · H/B —`.
- **§5 — Potentiel à 5 ans.** Calc rows: `Rendement courant = div. ÷ prix × 100`;
  `Rendement moyen 5 ans`; `Appréciation annualisée = appr.% ÷ 5`;
  `Rendement annuel total = appréciation + rendement moyen`. Result slots empty/`—` (2.6). Folded
  summary: `Rdt annuel total —`.

> The `contract::Study` shape (verified in `contract/src/study.rs`): `Study { id, journal_id,
> security_ticker, native_currency, years: Vec<YearData>, judgment: Judgment, rationale:
> Option<String>, created_at, schema_version }`. `YearData { year: i32, sales/eps/high_price/low_price:
> Cell, dividend_per_share/pre_tax_profit/book_value_per_share: Option<Cell> }`. `Cell { value:
> Option<Money>, source, freshness, review, coverage, provenance }` — in 2.3 you read **only
> `Cell.value`** (format it as a string); `source`/`freshness`/`review`/`coverage`/`provenance` are
> **displayed in 2.4/2.5**, not here.

### Regimes — exactly what 2.3 ships (and what it defers)

FR56: "switch a study between an **entry regime** (dense editing) and a **contemplation regime**
(reading/judgment), with the active regime clearly indicated." The UX spec adds the fold presets and
a colour/marker delta on constant geometry.

2.3 ships:
- the **regime enum + toggle** (clearly-indicated active regime, keyboard-operable, label-not-colour);
- the **fold presets**: Entry = all open; Contemplation = §1+§4 open, §2/§3/§5 collapsed;
- the **regime-driven token-swap mechanism** (a token snapshot the form reads), so the later marker
  and zone deltas have a hook;
- **constant geometry** (only colour/fold change);
- **persistence** of the active regime per study (AC 2).

2.3 defers (do NOT build):
- the **✓ marker attenuation** in contemplation (markers are **2.5**);
- the **zone full-saturation** in contemplation (zone bar is **2.6**).

So in 2.3 the *visible* regime delta is primarily the fold preset + the indicator; the colour delta
mechanism is in place but has little to recolor yet. State this honestly in the Dev Agent Record —
do not fake marker/zone visuals to make the delta look richer.

### Fold + regime persistence (ADD7 — app-config, never the journal)

- Store in `AppConfig` (`app/src/config.rs`), the forward-extensible `#[serde(default)]` struct 2.1
  established and 2.2 extended (`journal_path`). Recommended:
  `study_view_state: BTreeMap<String, StudyViewState>` keyed by study-id string,
  `StudyViewState { #[serde(default)] regime: Regime, #[serde(default)] folds: [bool; 5] }`.
- App-config lives in the OS **config** dir (`directories::ProjectDirs`), already wired by 2.1/2.2 —
  reuse `config::load`/`save` (corrupt-safe: bad file renamed `.invalid`, never destroyed). Do **not**
  duplicate persistence machinery and do **not** write into the journal.
- Add the same kind of unit test 2.2 added: an old `config.json` **without** the new field still loads,
  field defaults empty.
- **Never `deny_unknown_fields` on app-config** (the 2.1 forward-compat rail).

### Existing code being modified / extended (read before writing)

- **`app/ui/screens/dashboard.slint`** — the 2.2 restore view (the `if Studies.detail-title != ""`
  Rectangle showing `detail-title`/`detail-body`) is what you **replace** with the faithful form. Keep
  the create form + studies list intact. Everything sourced from `Tokens`.
- **`app/ui/state.slint`** — the `Studies` global (`rows`, `notice`, `read-only`, `detail-title`,
  `detail-body`, `create-study`, `open-study`) and `StudyRow` struct. Add the form structs +
  `toggle-fold` / `set-regime` callbacks here; re-export via `app.slint` (the 2.2 pattern).
- **`app/ui/tokens.slint` + `app/src/theme.rs`** — the `Tokens` global is the single source of truth.
  Colour/alpha tokens (`bg`, `surface`, `surface-alt`, `separator`, `text-high/mid/low`, `zone-buy/
  hold/sell`, `zone-alpha`, `focus`) swap per theme; metric/typo tokens (`space-*`, `font-*`,
  `weight-*`, `border`, `focus-border`, `radius`, `row-height`, …) are quasi-static. Add any **new**
  tokens you need (e.g. a grid-line colour, a calc-result-box token) to `Tokens` + both palettes in
  `theme.rs` — never hard-code. If you add regime tokens, follow the same palette-push pattern.
- **`app/src/viewmodel/{mod.rs,format.rs,studies.rs}`** — `format::format_amount(canonical, format)`
  is the **only** money→string path (locale-aware, no arithmetic). `studies::detail(study, _format)`
  is the 2.2 restore view; its `_format` arg was reserved "for the Story 2.3 SSG form" (2.2 review LOW
  note) — this is where you use it. `studies.rs` already has a `USER_FACING_MESSAGES` slice the
  posture test scans; add the new form labels to a scanned slice.
- **`app/src/config.rs`** — add the view-state field (above). Reuse load/save.
- **`app/src/main.rs`** — the `open-study` callback (parse UUID → `get_study` → `detail()` → push
  title/body) is where you also build the form structs and push fold/regime; add the `toggle-fold` /
  `set-regime` callbacks next to the `Prefs` callbacks (same validate→mutate→persist shape).
- **`app/src/posture.rs`** — the banned-verb gate scanning `.slint` `@tr()` + `USER_FACING_MESSAGES`
  slices; extend its asserted counts for the new strings.
- **`app/ui/components/{action_button,text_field,nav_item,choice_chip}.slint`** — reuse the a11y
  pattern (`FocusScope`, Enter/Space, visible focus ring) for the fold header + regime toggle.

### Architecture compliance (guardrails)

- **Cardinal Rule:** no calculation in `app`; `core` stays free of I/O; the contract→core mapping
  (2.6) lives in `app` when it arrives, never in `core`. 2.3 adds **no** calc.
- **Adapter rule:** money/decimals cross to Slint as formatted strings only (`viewmodel::format`).
- **Errors:** any failure (e.g. persisting fold state) is a visible neutral notice, never a swallowed
  `.ok()`/`.unwrap()` in non-test app code (architecture error model + retro rail).
- **Naming (architecture tree):** the form's home is `app/ui/study_screen.slint` per the tree; the
  reusable section is a `components/` primitive. `.slint` snake_case files, PascalCase components,
  kebab-case properties/callbacks, PascalCase globals.
- **Performance (NFR-P4):** form open/layout within the ~1 s recompute window; **no re-layout** on
  fold/regime change (Slint dirty-driven; only fold/colour properties update); launch ~within 3 s.

### Neutral voice (FR13 / posture gate)

- **No NAIC logo/wordmark/verbatim NAIC prose.** The A–H column letters, the §1–§5 structure and the
  formulas are reproducible method, not trademarks — **keep them**; neutralize only marks/wordmarks/
  verbatim instructional prose (the [[project_open_source_naming_constraint]] + [[project_high_fidelity_ssg_forms]] memories).
- **Banned verbs:** run every new label through `core::method::BANNED_VERBS_FR/EN` **before wiring**
  (1.11 had to rename a `Hold` type; 2.1/2.2 posture-gated their strings). French nouns naming the
  price bands (*« achat / neutre / vente »* as **zone-band nouns**) pass; imperatives (*« acheter /
  vendre / conserver »*) do not. Info-scent summaries are **fact lines** ("PER moy —"), never advice.
- Two distinct gate families (architecture): **trust gates** (types/traceability/reproducibility) vs
  **posture gates** (neutral naming, swappable labels) — do not reduce neutrality to a string grep;
  register strings in the scanned slices.

### Previous-story intelligence (2.2 dev record + review; 2.1; epic-1 retro)

- **Gates always `--locked`;** clippy `--all-targets --all-features` compiles tests + the frozen spike
  examples (`examples/spike_*.rs` must keep compiling — untouched here). 2.2's review re-ran every
  gate and re-diffed pinned surfaces; expect the same scrutiny.
- **Visual-verification DoD is load-bearing and the sandbox blocks screenshots / may lack AT-SPI** —
  2.1 and 2.2 both recorded a partial AC: process launches + on-disk truth proven, in-GUI pixel
  click-through left for human/AT-SPI confirmation. Plan for the same honesty: prove fold/regime
  **persistence** against the real on-disk `config.json` (write state → relaunch → read back), and
  record the GUI interaction as needing human confirmation if AT-SPI is unavailable.
- **File List completeness is the epic's single most-repeated finding (issue #18):** list **every**
  new/modified file (incl. any QA test file + the `_bmad-output/story-automator/…` automator log) with
  refreshed test counts **before** review. Budget the bookkeeping (Task 7).
- **`viewmodel::studies::detail()` already carries an unused `_format: NumberFormat`** kept explicitly
  "for the Story 2.3 SSG form" (2.2 review). Use it — don't add a parallel path.
- **Validate-before-mutate + corrupt-safe config** (1.10/2.2): reuse `config::load`/`save`; the
  fold/regime write must validate and persist, never silently drop.
- **`unused_crate_dependencies` is crate-level allow** (2.2 debug log): 2.3 adds no new dep, so the
  allow comment stays as-is; don't churn it.
- **Slint gotcha (2.2):** `row` is a reserved layout-attached property — don't name a property `row`
  (2.2 hit `Cannot override property 'row'` and renamed to `entry`). Watch reserved names in the grid.

### Git intelligence

Recent commits: `feat(story-2.2): Create, save & reopen a study …`, `feat(story-2.1): Application
shell …`. Conventions: conventional commits `feat(story-2.3): …`; the story file + `sprint-status.yaml`
update land in the **same** commit; merge only with all four gates green `--locked`. `app/` has real
structure now (2.1+2.2): `config.rs`, `theme.rs`, `labels.rs`, `clock.rs`, `state.rs`, `posture.rs`,
`viewmodel/{format,studies}.rs`, `ui/{tokens,state,app}.slint`, `ui/screens/*`, `ui/components/*` —
follow those patterns, do not reinvent. `core/`, `contract/`, `persistence/` must **not** change in
this story.

### Project Structure Notes

- **New:** `app/ui/components/collapsible_section.slint`; `app/ui/screens/study_screen.slint` (or
  `components/ssg_form.slint` — follow the architecture tree's `study_screen.slint` name);
  `app/src/viewmodel/form.rs` (or extension of `studies.rs`); possibly `app/src/regime.rs` (or fold
  the `Regime` enum into `theme.rs`); new `app` unit tests (config default, viewmodel mapping, posture).
- **Modified:** `app/ui/state.slint` (form structs + `toggle-fold`/`set-regime` callbacks),
  `app/ui/app.slint` (re-export), `app/ui/screens/dashboard.slint` (mount the form, replace restore
  view), `app/ui/tokens.slint` + `app/src/theme.rs` (any new tokens / regime snapshot),
  `app/src/config.rs` (view-state field + test), `app/src/main.rs` (form push + fold/regime callbacks),
  `app/src/posture.rs` (scan new strings), `app/src/viewmodel/{mod,studies}.rs`,
  `sprint-status.yaml`, this story file.
- **Untouched (verify with `git diff` — must be empty):** `core/`, `contract/`, `persistence/`,
  `ingestion/`, `report/`, `docs/method/**`, `.github/workflows/ci.yml`, `rust-toolchain.toml`,
  `deny.toml`, the frozen `persistence/tests/corpus/v1.db`.
- **Variance note (architecture tree):** the tree names `app/src/state.rs` as the "immutable StudyState
  snapshot; undo stack; content-addressed verdict". 2.3 needs **none** of the undo/verdict slice — it
  reads the persisted `Study` for display only. Implement just the display read; the undo stack is 2.9
  and the verdict is 2.6. A documented partial is fine (same posture 2.2 took on `state.rs`).

### References

- Story & ACs: `_bmad-output/planning-artifacts/epics.md` § "Story 2.3" (lines 569–583) + Epic 2 intro
- FR2 (reopen/display), FR56 (entry↔contemplation regimes), FR57/FR58 (legend/empty — **later**
  stories 2.13), FR64 (disclaimer), FR65 (offline): `_bmad-output/planning-artifacts/prd.md`
  § "Functional Requirements"
- Faithful-form rules, fold presets, info-scent summary, two-regime delta, colour tokens, constant
  geometry, accessibility: `_bmad-output/planning-artifacts/ux-design-specification.md`
  (Design Direction Decision lines 650–699; Visual Foundation 474–641; Component Strategy 858–931;
  Responsive & Accessibility 1010–1068) and the mockup `ux-stock-study-screen.html`
- Crate boundaries / Cardinal Rule, adapter (money-as-strings) rule, naming, error model, app-config-
  vs-journal (ADD7), token single-source, study_screen.slint home: `architecture.md`
  § "Project Structure & Boundaries", § "Frontend Architecture", § "Enforcement Guidelines"
- Contract shapes consumed (read-only): `contract/src/study.rs` (`Study`/`YearData`/`Judgment`/
  `ForecastLowOption`), `contract/src/cell.rs` (`Cell` + `Source`/`Freshness`/`Review`/`Coverage`)
- A–H column / §1–§5 mapping & formulas: `_bmad-output/implementation-artifacts/1-2-ssg-method-specification.md`
  and `docs/method/**` (read for fidelity; **do not** call the engine)
- Prior story (the placeholder this replaces) + its review LOW notes: `2-2-create-save-reopen-study.md`
- Tokens/components/viewmodel patterns to reuse: `app/ui/tokens.slint`, `app/ui/state.slint`,
  `app/src/theme.rs`, `app/src/viewmodel/{format,studies}.rs`, `app/ui/components/*.slint`
- High-fidelity SSG fidelity & naming constraints: project memories
  [[project_high_fidelity_ssg_forms]], [[project_open_source_naming_constraint]]; GUI = Slint-only:
  [[project_gui_framework_slint_only]]
- Issue #18 (File List discipline): `gh issue view 18`; banned verbs: `core::method::BANNED_VERBS_EN/FR`

### Tech currency note (2026-06-13)

Slint pinned at 1.16; this story adds **no external dependency** (it is UI structure + a config
field). The §1 chart and §4 zone bar are **placeholders** here — the native-Slint `Path`/`TouchArea`
chart work (Spike B) lands in 2.8, the zone bar in 2.6. `cargo deny check` stays green; the
`Cargo.lock` delta is expected to be **empty** — verify and record.

## Dev Agent Record

### Agent Model Used

claude-opus-4-8 (1M context) — BMAD dev-story workflow.

### Debug Log References

- **Slint `@children` in a conditional is illegal.** First `CollapsibleSection` draft put the body
  under `if root.open:` → build error "The @children placeholder cannot appear in a conditional
  element". Fixed by folding via a clipped height-0 container (`height: open ? body.preferred-height
  : 0px; clip: true`) so `@children` stays unconditional.
- **Element ids unreachable from a component-root function.** Moving the dashboard list under
  `if !Studies.study-open:` put `ticker`/`currency` inside a conditional, so the root-level
  `submit-create()` could no longer see them ("Cannot access id 'ticker'"). Fixed by declaring the
  function on the `content` VerticalLayout inside the branch and calling `content.submit-create()`.
- **clippy `unnecessary_get_then_check`** on a test assertion (`get(k).is_none()`); switched to
  `!contains_key(k)`.
- All four gates green `--locked` after the fixes: `cargo fmt --all --check`,
  `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --all --locked`,
  `cargo deny check` (advisories/bans/licenses/sources ok).

### Completion Notes List

Implemented the faithful collapsible §1–§5 SSG form, `app`-only. Highlights:

- **Task 1 — CollapsibleSection + config.** New `app/ui/components/collapsible_section.slint`
  (chevron-glyph + number + title fold header; info-scent summary line shown only when folded;
  `FocusScope` + Enter/Space + visible focus ring; constant-geometry fixed header height). New
  `StudyViewState { regime, folds: [bool;5] }` on `AppConfig` (per-study `BTreeMap`, append-only
  `#[serde(default)]`); `StudyViewState::default()` = Entry + all-open so an absent/partial entry is
  never an all-collapsed accident. Old-config-loads + partial-entry + round-trip tests added.
- **Task 2 — faithful layout.** New `app/ui/screens/study_screen.slint`: non-collapsible identity
  header + capitalization block (faithful `—` cells), then five `CollapsibleSection`s in canonical
  order. §3 = the A–H lettered P/E table with formula captions (`A÷C`, `B÷C`, `F÷C×100`, `F÷B×100`)
  on a visible 1 px cell grid + Total/Moyenne/PER moyen/PER courant summary rows; §2 = PTP/ROE
  management grid; §4/§5 = label · formula · boxed result calc rows. §1 chart + §4 zone bar are
  calm fact-stating PLACEHOLDERS (no fake chart/zones).
- **Task 3 — adapter.** New `app/src/viewmodel/form.rs`: `Study → FormHeader`/`PeRow` structs. Money
  crosses as locale strings via `viewmodel::format` only; absent cell → `EMPTY_SLOT` (`—`), **never
  `0`**; D/E/G/H caption-only (computed in 2.6). `method_version` from `core::METHOD_VERSION` (a
  `&str` identity const — display, not the engine call). The dead 2.2 `studies::detail()` + its
  labels were removed (superseded).
- **Task 4 — regimes.** New `app/src/regime.rs`: `Regime { Entry, Contemplation }` + fold presets
  (Entry all-open; Contemplation §1+§4) + a single `regime-emphasis` token swap (`Tokens`), applied
  via `regime::apply`. Regime toggle = two `ChoiceChip`s (keyboard-operable, active state by label +
  weight, not colour alone). Constant geometry: only an alpha + fold state change, never a size.
- **Task 5 — wiring + persistence.** `main.rs` `open-study` builds the form structs, restores the
  persisted view-state (default Entry), pushes regime/folds, sets `study-open`. New `toggle-fold` /
  `set-regime` callbacks mutate `study_view_state` (validate index → mutate → `persist`, the 2.2
  Prefs shape, no silent `.ok()`), then re-push. Dashboard swaps list ↔ form; "‹ Retour" closes.
- **Task 6 — posture/a11y/gates.** Posture `@tr` floor bumped 15 → 60 and file floor 8 → 11 (the
  scan covers every new form label — no banned verbs; French zone-band/price nouns like *prix
  haut/bas* are safe, no imperatives). All four gates green `--locked`.
- **Pinned surfaces:** `git diff` over `core/ contract/ persistence/ ingestion/ report/ docs/method/
  .github/ deny.toml rust-toolchain.toml` + frozen `v1.db` is **empty**. **`Cargo.lock` delta is
  empty** — no new external crate (verified).
- **Interpretations** consolidated in **GitHub issue #20** (the 1.11/2.1 pattern): view-state
  granularity, the regime-emphasis hook for 2.5/2.6, fixed-5 §3 rows + faithful-static §2,
  `method_version` source, capitalization placeholders, the height-0 fold technique.

**Visual verification (AC 6) — partial, mirrors 2.1/2.2.** The built app **launches cleanly** (no
panic; ran to the kill timeout, empty error log). Screenshot/AT-SPI capture is unavailable under this
Wayland sandbox (`import -window root` fails; no Xvfb), so the in-GUI click-through (fold/unfold
info-scent summaries, regime toggle with no jank, relaunch-restore, dark/light + label-set + locale
swaps, footer disclaimer, ≤3 s launch) is left for **human/AT-SPI confirmation**. The load-bearing
**fold+regime persistence is proven headlessly against the real on-disk `config.json`**
(`config::tests::fold_and_regime_edits_survive_a_simulated_relaunch`), and the adapter rails
(money→string, absent→`—` never `0`, computed columns caption-only) are unit-tested.

**Tests:** app crate 53 passing (was 39): +regime (6), +config view-state (5), +form adapter (5),
posture thresholds tightened. Full workspace suite green.

### File List

**New (app-only):**
- `app/src/regime.rs` — `Regime` enum, fold presets, `regime-emphasis` token swap (`apply`).
- `app/src/viewmodel/form.rs` — `Study → FormHeader`/`PeRow` adapter (money→strings, `—` slots).
- `app/ui/components/collapsible_section.slint` — reusable fold section + info-scent summary.
- `app/ui/screens/study_screen.slint` — the faithful §1–§5 form + regime toggle + placeholders.

**Modified (app-only):**
- `app/src/main.rs` — `mod regime`; open-study builds/pushes form + view-state; `toggle-fold` /
  `set-regime` persistence callbacks; `push_view_state` helper; `unused_crate_dependencies` note
  updated (`core` now on the runtime path).
- `app/src/config.rs` — `StudyViewState` + `study_view_state` field (append-only); tests.
- `app/src/theme.rs` — `grid_line` palette token (dark/light) pushed into `Tokens`.
- `app/src/viewmodel/mod.rs` — `pub mod form`.
- `app/src/viewmodel/studies.rs` — removed superseded `detail()` + restore-view labels; lean to_row.
- `app/src/posture.rs` — dropped the removed studies label slice; bumped `@tr`/file floors.
- `app/ui/state.slint` — `FormHeader`/`PeRow` structs; `Studies` form/fold/regime props + callbacks
  (replacing `detail-title`/`detail-body`).
- `app/ui/app.slint` — re-export `FormHeader`/`PeRow`.
- `app/ui/tokens.slint` — `grid-line` + `regime-emphasis` tokens.
- `app/ui/screens/dashboard.slint` — mount `StudyScreen` on open; keep list/create; remove old view.
- `_bmad-output/implementation-artifacts/sprint-status.yaml` — 2-3 → review.
- `_bmad-output/implementation-artifacts/2-3-faithful-collapsible-ssg-form.md` — this record.

## Senior Developer Review (AI)

**Reviewer:** Guy (story-automator adversarial review) · **Date:** 2026-06-13 · **Outcome:** Approve
(status → done).

### What was verified (claims re-checked against reality, not taken on trust)

- **All four gates green `--locked`** on this machine: `cargo fmt --all --check`, `cargo clippy
  --all-targets --all-features --locked -- -D warnings`, `cargo test --all --locked`, `cargo deny
  check` (advisories/bans/licenses/sources ok). App crate **53/53** tests pass (matches the claimed
  39 → 53).
- **Pinned surfaces untouched:** `git diff` over `core/ contract/ persistence/ ingestion/ report/
  docs/method/ .github/ deny.toml rust-toolchain.toml` + the frozen `v1.db` is **empty**. **`Cargo.lock`
  delta empty** — no new external crate. Both verified, not assumed.
- **Cardinal Rule intact (AC4):** `viewmodel/form.rs` reads only `Cell.value` (→ locale string via
  `format_amount`) and `core::METHOD_VERSION` (a `&'static str` identity const, not the engine). No
  `compute` call, no `Judgment → JudgmentInputs` mapping. No hard-coded hex/`px` in the new `.slint`
  (scanned: only relative `%` widths + an alpha-driven `opacity`, both legitimate).
- **`unknown` never `0` (retro rail):** absent cells render `EMPTY_SLOT` (`—`); unit-tested for the
  empty study (5 faithful empty rows), absent dividend, and present-year direct vs caption-only columns.
- **Clean removal of the 2.2 restore view:** no dangling `detail()` / `detail-title` / `detail-body`
  references remain. No production `unwrap`/`expect`/`panic`/`TODO` in the new source (all `unwrap`s are
  test-only).
- **AC6 (visual) — partial, as recorded:** the binary builds and the Slint form compiles; a headless
  launch fails cleanly at winit backend init (no display in the sandbox), **not** a panic in the form.
  In-GUI click-through (fold/unfold, regime toggle, relaunch-restore, theme/label/locale swaps, footer
  disclaimer, ≤3 s launch) remains for human/AT-SPI confirmation. The load-bearing fold+regime
  **persistence** is proven headlessly against the real on-disk `config.json`.

### Findings

No CRITICAL / HIGH / MEDIUM issues. Three LOW items; one auto-fixed, two recorded.

- **LOW (auto-fixed).** Re-selecting the **already-active** regime chip re-applied that regime's fold
  preset, silently discarding the user's manual fold edits within the regime (AC3 says only an actual
  *switch* applies the preset). Fixed in `app/src/main.rs` `on_set_regime`: a guard early-returns when
  the selected regime equals the current persisted regime (no preset reset, no persist, no re-push).
  Re-ran fmt/clippy/test — all green (53/53).
- **LOW (recorded, no change).** `viewmodel/form.rs` hard-caps the §3 table at `PE_TABLE_ROWS = 5`; a
  study with >5 years would drop years 6+. Moot in 2.3 (only empty studies exist until 2.4 adds data
  entry) and already noted in issue #20 — revisit when 2.4 wires real year data.
- **LOW (recorded, no change).** §3 summary rows (`PeSummaryRow`, 4:5 stretch) don't column-align with
  the 9-column A–H grid above them — a minor visual seam under the high-fidelity mandate, best settled
  when 2.6 populates the summary values.

## Change Log

- 2026-06-13 — **Review (story-automator, adversarial):** Approve → status **done**. Four gates
  re-verified green `--locked`; pinned surfaces + `Cargo.lock` re-diffed empty; 53/53 app tests. One
  LOW auto-fixed (re-selecting the active regime no longer clobbers manual fold edits —
  `main.rs:on_set_regime` guard); two LOWs recorded (§3 fixed-5 rows, summary-row alignment) for
  2.4/2.6. No CRITICAL/HIGH/MEDIUM.
- 2026-06-13 — Story 2.3 implemented (faithful collapsible §1–§5 SSG form): `CollapsibleSection`
  primitive + per-study fold/regime view-state in app-config; the §1–§5 `StudyScreen` (A–H P/E grid
  with formula captions, §2 management grid, §4/§5 calc rows, §1/§4 placeholders); `Study→Slint`
  form adapter (money→strings, absent→`—`); two regimes (fold presets + `regime-emphasis` token swap
  on constant geometry) with persistence. `app`-only; pinned surfaces + `Cargo.lock` untouched. App
  tests 39 → 53; all four gates green `--locked`. Interpretations → issue #20. Status → review.
