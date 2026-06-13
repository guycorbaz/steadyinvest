# Story 2.4: Manual data entry with provenance & coverage

Status: done

<!-- Epic-2 story 4 — the FIRST story that makes the SSG grid EDITABLE and puts real data into a
     study. Story 2.3 rendered the faithful §1–§5 form as a READ-ONLY display of stored values (every
     cell a formatted string or an em-dash; the underlying `Study` for a freshly-created study has an
     EMPTY `years` vec). THIS story turns the §2 management grid and the §3 A–H table into a
     spreadsheet-grade ENTRY surface: type-to-edit + paste-a-column with locale-aware decimal parsing,
     cell-cursor keyboard navigation, each edited cell stamped `source = manual` through the existing
     `contract::Cell::edited` rail and PERSISTED via the 2.2 journal path; and it renders each cell's
     `source × freshness × coverage` state under the attention hierarchy (missing shouts, stale
     murmurs, source on demand) with NO colour spent. SCOPE GUARDRAIL — 2.4 is data entry +
     provenance/coverage DISPLAY only: NO tri-state review markers (✓/?) / soft-lock (2.5); NO
     engine/compute, verdict, zone bar, U/D, projected return (2.6); NO §1 interactive chart (2.8); NO
     plausibility/low-confidence warnings (2.7); NO provider fetch / reconciliation / stale-on-refresh
     (Epic 3). The computed columns (D/E/G/H) and §4/§5 result slots stay caption-only em-dashes.
     `unknown/insufficient` is NEVER shown or stored as `0` (the prior project's blank-coercion class
     of bug). Headless CI cannot prove paste/keyboard render: the visual-verification DoD (AC 6) is
     load-bearing, exactly as it was for 2.1/2.2/2.3 — but the entry→`Cell::edited`→persist round-trip
     and the locale parse are proven headlessly. -->

## Story

As Guy,
I want spreadsheet-grade manual entry showing each cell's source and coverage,
so that I can complete a study by hand and see its data honesty at a glance.

## Acceptance Criteria

1. **Spreadsheet-grade entry: type or paste-a-column, locale-aware, cell-cursor nav, each edit stamped
   `source = manual` (FR16, FR63 locale).** In an open study the **editable input cells** — the §3 A–H
   table's direct columns (**A** `high_price`, **B** `low_price`, **C** `eps`, **F**
   `dividend_per_share`) and the §2 management grid's direct rows (**sales**, **pre-tax profit**,
   **book value/share** — plus `eps` shared with §3) — accept manual entry:
   - **type-to-edit inline** with a visible active-cell cursor (brighter surface + 1 px ink ring, **no
     colour spent**, reusing the 2.1/Spike-A pattern);
   - **cell-cursor keyboard navigation**: arrows move the active cell, **Enter commits + advances down**,
     **Tab commits + advances right** (Shift+Tab left), typing replaces, Backspace/Delete clears to a
     gap (→ `Coverage::ToFill`, **not** `0`);
   - **paste a column of years** (Ctrl+V): a clipboard column of newline-separated values fills
     consecutive cells **downward from the active cell**, each value parsed exactly;
   - **all parsing is locale-aware** per the active `NumberFormat` (decimal **comma**/point + thousands
     separator incl. the narrow-NBSP group) → `rust_decimal::Decimal::from_str_exact` → `contract::Money`
     — the production reverse of `viewmodel::format::format_amount`, closing the Spike-A locale defer;
   - **every committed edit goes through `contract::Cell::edited(new_value, provenance)`** with a
     **manual** `Provenance` built from the injected `Clock` (`source = Manual`, `freshness = Current`),
     and the updated `Study` is **persisted** via the 2.2 journal path (`Journal::put_study`, an upsert
     that bumps `logical_version` and appends the FR51 time-series). A read-only journal refuses the
     write with the existing neutral notice; a save failure surfaces a banner, **never** a silent `.ok()`.

2. **Each cell displays its `source × freshness` and `coverage` state under the attention hierarchy
   (FR17–FR19 display).** Every data cell renders its independently-queryable state with **strong
   colour reserved for judgment zones (none exist yet) — provenance is texture, never colour**:
   - **coverage** is one of three **visually distinct** states: **present** (a value), **to-fill** (a
     marked, unfilled gap that **shouts** — a bold neutral glyph / diagonal hatch, "a hole in a regular
     grid"), **not-available-accepted** (a deliberate, *quiet* accepted gap — distinct from to-fill, it
     does **not** shout);
   - **freshness**: **stale** *murmurs* (~60 % opacity + a hollow dot / slight italic); **current** is
     the silent default;
   - **source** (provider / manual / derived) is **revealed on demand** (hover/focus), **not**
     always-on — no per-cell badge burns the colour/ink budget;
   - a per-cell gesture sets **not-available-accepted** (and clears it back to to-fill), so the user can
     deliberately mark a permanent gap (FR19);
   - **N/A, 0, and empty/to-fill are three distinct states** (a deliberate accepted gap, a real zero
     value, an unfilled cell) — never conflated.

3. **`unknown/insufficient` is never shown or stored as `0`.** A blank, cleared, or unparseable entry
   maps to **no value** (`Cell.value = None`, `Coverage::ToFill`) — rendered as the shouting gap glyph,
   **never** `0`, never a crash. This holds on type, on paste (a blank/non-numeric line leaves its cell
   an empty gap), and on round-trip through persistence.

4. **Crate-boundary & adapter discipline (architecture Cardinal Rule).** **No calculation in `app`**:
   2.4 enters and displays *raw input cells*; it does **not** compute D/E/G/H, the §2 averages/trends,
   the §4/§5 results, the verdict, or anything the engine owns (Story 2.6) — those stay caption-only
   em-dash slots. The only number-shaped work is **input normalization** (locale string →
   `contract::Money` via `Decimal::from_str_exact`), which is parsing, not arithmetic, and reuses the
   contract's exact-decimal type — **never `f64`** (only pixel/layout math may use floats). Money/decimals
   cross into Slint **only as already-formatted locale strings** via `viewmodel::format`; the parse lives
   in the viewmodel, the `Cell::edited` rail lives in `contract`, the `Provenance` is stamped in `app`
   from the injected `Clock` (ADD15 — no scattered wall-clock/`Uuid::new_v4`). All colours/sizes from
   `Tokens`; new `.slint` files snake_case, components PascalCase, properties/callbacks kebab-case.

5. **Quality gates, posture, dependency & pinned-surface discipline.** All four gates green `--locked`:
   `cargo fmt --all --check` · `cargo clippy --all-targets --all-features --locked -- -D warnings` ·
   `cargo test --all --locked` · `cargo deny check`. Specifically:
   - **`arboard` moves from a dev-dependency to a real `[dependencies]` entry** (production paste now
     uses it) — or the Slint-native `Platform::clipboard_text()` path is used instead; record the
     choice. The `Cargo.lock` delta and any new transitive licence are **expected and recorded**;
     `cargo deny check` must stay green (the Spike-A run already allow-listed `clipboard-win` BSL-1.0 in
     `deny.toml` — **touching `deny.toml` for the clipboard dep is in-scope for THIS story only** if a
     not-yet-allowed transitive licence appears; keep the tree GPL-3.0-compatible);
   - **every new user-visible string** (any not-available-accepted action label, status/coverage
     tooltips, entry hints) passes the crate-local **banned-verb posture test** — register them in the
     scanned `@tr()`/`USER_FACING_MESSAGES` surfaces; bump the asserted floors. Reuse
     `core::method::BANNED_VERBS_FR/EN`, never copy;
   - **new keyboard-operable controls** follow the 2.1 a11y pattern (`FocusScope` + visible focus ring,
     `decision never colour-only` — the gap/stale/source states are carried by **glyph/texture/opacity**,
     not colour);
   - **pinned surfaces untouched** (`git diff` empty): `core/`, `persistence/`, `ingestion/`, `report/`,
     `docs/method/**`, `.github/`, `rust-toolchain.toml`, and the frozen
     `persistence/tests/corpus/v1.db`. **`contract/` is NOT modified** — the `Cell::edited` rail,
     `Provenance`, `YearData`, and the coverage/source/freshness enums **already exist** (Story 1.11);
     2.4 consumes them, it does not change them. **`deny.toml`** may change **only** for the clipboard
     dependency licence, if needed.

6. **Visual verification (Definition of Done — load-bearing, mirrors 2.1/2.2/2.3).** Launch the built
   app, open a study, and verify on display: **type a value** into a §2/§3 cell (cursor visible, value
   accepted, locale format applied); **paste a column** of year-values that lands in consecutive cells;
   a **blank/non-numeric** entry stays an **empty shouting gap, never `0`**; **keyboard cell-cursor**
   nav (arrows/Enter/Tab) works; a cell marked **not-available-accepted** reads as a *quiet* gap,
   distinct from a to-fill gap; **source is revealed on hover/focus**, not always-on; **close →
   relaunch → reopen the same study → every entered value, its `source = manual`, and its coverage
   state are restored** intact. Confirm the footer disclaimer (FR64), dark/light + label-set + locale
   swaps (2.1), and fold/regime restore (2.3) still work, and launch-to-interactive stays ~within 3 s
   (NFR-P4). Record the run in the Dev Agent Record. Headless CI cannot stand in for this AC — but the
   **entry→`Cell::edited`→`put_study`→reopen round-trip** and the **locale parse** ARE proven headlessly.

## Tasks / Subtasks

- [x] **Task 1 — Locale-aware entry parser (production reverse of `format_amount`) (AC: 1, 3, 4)**
  - [x] Add a parse path in the viewmodel (recommended: `viewmodel::format::parse_amount(input, format)
        -> Option<Money>`, or a new `viewmodel/entry.rs`): strip the active `NumberFormat`'s thousands
        separator (comma **or** narrow-NBSP `\u{202F}` — and tolerate a plain space the user might
        type), map the decimal separator (comma/point) to a canonical `.`, accept the display minus
        `\u{2212}` as well as ASCII `-`, then `Decimal::from_str_exact` → `Money`. Blank / multi-dot /
        non-numeric → `None` (**never `0`**). Pure string→Decimal, no arithmetic (Cardinal Rule).
  - [x] Unit-test both presets incl. the CH/EU cases the spike deferred: `"1 234,56"` (NBSP + comma) and
        `"1,234.56"` parse exactly; `"1,5"` parses under `Comma`; `"−12,5"`/`"-12.5"` parse; `""`,
        `"1.2.3"`, `"12a"`, `"--2"` → `None`. Assert the **round-trip** `parse_amount(format_amount(x))`
        is value-stable for canonical inputs.

- [x] **Task 2 — Editable grid cell + cell-cursor model + paste-a-column (AC: 1, 3)**
  - [x] Add an **editable cell** variant (extend `study_screen.slint`'s `GridCell`, or a new
        `app/ui/components/editable_cell.slint`): an inline `TextInput` (the `text_field.slint` a11y
        pattern — visible focus ring, no std-widgets) shown for the editable input columns, a read-only
        `Text` for computed/derived slots. Right-aligned tabular figures, constant geometry (no size
        change between display and edit), 1 px grid borders from `Tokens.grid-line`.
  - [x] Track the **active cell** (row, column) in the form's state; a `FocusScope` `key-pressed`
        handler moves it (arrows / Enter↓ / Tab→ / Shift+Tab← with edge handling), edits inline on type,
        clears on Backspace/Delete. Guard modifier chords (`!control && !meta`) so Ctrl/Cmd combos do not
        leak a character into the cell (the Spike-A review fix).
  - [x] **Paste-a-column** (Ctrl+V): read the clipboard (promote **`arboard` to a runtime dependency**,
        `default-features = false`; or the Slint-native `Platform::clipboard_text()` — record which),
        split on `\n`/`\r\n`, parse each line via Task 1, fill consecutive cells of the **active column**
        downward from the cursor. Lines past the grid bottom are dropped with a neutral count notice
        (Spike-A review fix). A blank/unparseable line → an empty gap cell, **never `0`**.

- [x] **Task 3 — Wire edits → `Cell::edited` → `Study` → persist (AC: 1, 3, 4)**
  - [x] Add a `JournalState::edit_cell(...)` (and/or `save_study`) method in `app/src/state.rs`: load
        the open `Study`, locate/create the target `YearData`, replace the target `Cell` via
        `contract::Cell::edited(new_value, manual_provenance())`, and `put_study` it (the upsert; bumps
        `logical_version`, appends FR51 history). Reuse the read-only / no-journal / save-failure guards
        and neutral notices already in `state.rs` (`MSG_READ_ONLY_WRITE`, `MSG_NO_JOURNAL`,
        `MSG_SAVE_FAILED`). **Validate→mutate→persist; no silent `.ok()`** (the 2.2/2.3 rail).
  - [x] **Manual `Provenance`** helper (in `app`, from the injected `Clock`): `source = Source::Manual`,
        `timestamp = clock.now()`. Record the chosen `logical_version` and `hash_of_dependencies` for a
        **manually-entered leaf** input (no upstream dependencies) — see Dev Notes § "Manual provenance:
        the two underspecified fields" and file the interpretation (issue, per the 2.1/2.2/2.3 pattern).
  - [x] **Year-grid materialization:** a freshly-created study (2.2) has an **empty `years` vec**. Decide
        and implement how the editable year rows come into being (Dev Notes § "Where do the year rows
        come from?"): recommended = materialize a deterministic window of N most-recent historical years
        (all cells `ToFill`) the first time the study is opened for entry, derived from the study's
        created-at year; a cell becomes `Present` only when actually entered. Record the rule as an
        interpretation. **Fix the 2.3 `PE_TABLE_ROWS = 5` hard cap** so a study with the real year set
        renders all its years (the 2.3 review LOW + issue #20).

- [x] **Task 4 — `source × freshness × coverage` display under the attention hierarchy (AC: 2, 3)**
  - [x] Extend the form adapter (`viewmodel/form.rs`) so each cell crosses to Slint with its **state**,
        not just its value: add per-cell fields (e.g. `coverage: "present"|"to-fill"|"not-available"`,
        `stale: bool`, `source: "manual"|"provider"|"derived"`) on the `PeRow`/grid structs (or a new
        `GridCellState` struct). Money still crosses as a **formatted string**; the state is enum-derived
        strings/bools — no float.
  - [x] Render the **attention hierarchy** in `.slint`, **no colour spent** (colour budget = zones only,
        which don't exist yet): **to-fill** = a bold neutral glyph / diagonal hatch that *shouts*;
        **not-available-accepted** = a quiet distinct marker (e.g. a faint "n/a" ink, calm); **stale** =
        ~60 % opacity + a hollow dot / slight italic (*murmur*); **present-current** = the silent
        default. Add any needed neutral tokens (hatch/gap ink, stale opacity) to `Tokens` + both
        palettes in `theme.rs` — never hard-code.
  - [x] **Source on demand:** a hover/focus affordance (tooltip or a focus-revealed caption) shows the
        cell's source; **not** an always-on badge. Keyboard-reachable (focus reveals it too — a11y).
  - [x] **not-available-accepted gesture:** a per-cell action (define the gesture — e.g. a key on an
        empty/active cell, or a small affordance) that flips `ToFill → NotAvailableAccepted` and back,
        persisted through the Task-3 path. Its label passes the posture gate.

- [x] **Task 5 — Wire into the study screen; persist + restore entered data (AC: 1, 2, 6)**
  - [x] In `study_screen.slint` / `dashboard.slint`, mount the editable §2/§3 grids in place of the 2.3
        read-only cells (keep §1 chart + §4 zone bar PLACEHOLDERS, keep §4/§5 calc-row result slots
        caption-only em-dash — those are 2.6). Keep fold/regime (2.3), the dashboard list + create flow
        (2.2) intact. Editing is the entry regime's job; do not change the regime mechanism.
  - [x] In `main.rs`, add the edit callbacks on the `Studies` global (`commit-cell(row, col, text)`,
        `paste-column(row, col)`, `set-not-available(row, col, bool)`) → call the Task-3 `state.rs`
        methods → on success **rebuild the form structs from the re-read `Study`** and re-push (one
        source of truth, the 2.3 `push_view_state` shape); on refusal set the neutral notice. Keep the
        injected `Clock`/`IdGen` the single time/identity source.
  - [x] Keep `main.rs` allow-scopes honest: `arboard` is now a genuinely-used **runtime** dep (no longer
        dev-only); `ingestion`/`report`/`tokio` remain unused until Epic 3 — update the
        `#![allow(unused_crate_dependencies)]` comment-of-record accordingly.

- [x] **Task 6 — Posture, accessibility & gates (AC: 5)**
  - [x] Extend the `app` posture test to scan the new `.slint` `@tr()` literals + any new
        `USER_FACING_MESSAGES` (not-available label, paste/clear notices, source tooltips) against
        `BANNED_VERBS_FR/EN`; bump the `>= 11` file floor and `>= 60` `@tr` total floor to cover the new
        strings. (French data nouns are safe; watch imperatives.)
  - [x] Keyboard walkthrough by construction: the active-cell cursor, paste, and the not-available
        gesture are `FocusScope`-operable with a visible focus ring; tab order logical; the grid is
        readable with **colour stripped** (gap glyph + stale opacity/dot + source-on-focus carry meaning,
        never colour alone). Reduced-motion respected on any edit micro-feedback.
  - [x] All four gates green `--locked`. `git diff` over `core/ contract/ persistence/ ingestion/
        report/ docs/method/ .github/ rust-toolchain.toml` + the frozen `v1.db` is **empty**. Record the
        `Cargo.lock` delta (arboard + transitives) and confirm `cargo deny check` green (note any
        `deny.toml` clipboard-licence line touched).

- [x] **Task 7 — Visual verification, records & File List (AC: 6)**
  - [x] Launch, walk the AC-6 journey (open study → type a cell → paste a column → blank stays a gap,
        never `0` → keyboard nav → mark not-available → source on hover/focus → **relaunch → all entered
        data + source + coverage restored**), record the outcome (and any sandbox clipboard/AT-SPI
        limitation, as 2.1/2.2/2.3 did) in the Dev Agent Record.
  - [x] Prove headlessly what the sandbox blocks visually: a test that **enters values → `Cell::edited`
        stamps `source = manual` + `Coverage::Present` → `put_study` → re-`get_study` → values + state
        survive**, and the locale parse tests (Task 1). Refresh test counts in the Change Log.
  - [x] Update the **File List** (every new/modified file incl. any QA-generated test file + the
        automator log — issue #18 discipline) and file a consolidated GitHub issue for the genuine 2.4
        interpretations (year-grid derivation rule, manual-provenance `logical_version`/`hash` choice,
        not-available gesture, clipboard path) — issues, not inline TODOs (the 1.11/2.1/2.2/2.3 pattern).

## Dev Notes

### What this story is — and the disasters it must make impossible

2.4 is the **first story that puts real data into a study**. Story 2.3 built the faithful §1–§5 form as
a **read-only display** (every cell a formatted string or an em-dash; a freshly-created study's `years`
vec is **empty**). This story makes the §2 management grid and the §3 A–H table **editable**:
spreadsheet-grade entry (type + paste-a-column, locale-aware), each edit stamped `source = manual` and
**persisted**, with each cell's `source × freshness × coverage` shown under the attention hierarchy. It
is **`app`-only** (the contract primitives already exist).

Disasters to prevent:
- **Scope bleed into 2.5/2.6/2.7/2.8 and Epic 3.** The biggest risk. 2.4 enters **raw input data** and
  displays **provenance/coverage** — nothing more:
  - **NO tri-state review markers (`✓`/`?`) and NO soft-lock** — that is **Story 2.5**. The `Cell.review`
    field exists and `Cell::edited` already has the "divergent edit demotes `✓`→`?`" semantics, but 2.4
    **does not render or let the user set review tags**, and does not build the soft-lock edit-guard.
    (A manual edit may still pass through `Cell::edited`, whose review semantics are a no-op while the
    user has set no `✓` — leave the field, render nothing for it.)
  - **NO engine / `core::ssg::compute` / `Judgment → JudgmentInputs` mapping / verdict / zone bar / U-D /
    projected return** — **Story 2.6**. The computed columns D/E/G/H and every §4/§5 result stay
    caption-only em-dash slots. **No calculation in `app`.**
  - **NO §1 interactive chart / draggable line** — **Story 2.8** (the §1 area stays a placeholder).
  - **NO plausibility / unit-split / low-confidence warnings** — **Story 2.7**.
  - **NO provider fetch, reconciliation, or stale-on-refresh** — **Epic 3**. 2.4 stamps every manual
    edit `Freshness::Current` (the rail does this); **no cell becomes `Stale` in 2.4 except via test
    fixtures**. The stale-*murmur* rendering is built but barely exercised until Epic 3 — **state that
    honestly in the Dev Agent Record**, do not fake stale cells to make the texture look used (the 2.3
    "regime colour delta has little to recolor yet" honesty rail).
- **Calculation in `app`.** Cardinal Rule: all SSG math lives in `core`. 2.4's only number work is
  **input normalization** — a locale string parsed to `contract::Money` via `Decimal::from_str_exact`.
  That is parsing, not arithmetic; reuse the contract type, never `f64`. Do not compute averages,
  trends, P/Es, forecasts, or the verdict.
- **`unknown` rendered or stored as `0`.** The single most-repeated project rail. A blank/cleared/
  unparseable entry is `value: None` → `Coverage::ToFill` → the **shouting gap glyph**, never `0`. The
  contract's `Cell::edited` already enforces `Some ⇒ Present`, `None ⇒ ToFill`; honour it on display too.
  **N/A (not-available-accepted), 0 (a real zero), and to-fill (empty)** are three distinct states.
- **Floats/Decimals crossing into Slint.** Money crosses as **formatted strings** via
  `viewmodel::format` only; the parse returns a `Money`, never an `f64` to the UI.
- **A scattered wall-clock / `Uuid::new_v4`.** The manual `Provenance` timestamp comes **only** from the
  injected `Clock` (ADD15). No `Utc::now()` / `Uuid::new_v4` outside `clock.rs`.
- **Mutating the contract or pinned surfaces.** Everything 2.4 needs in `contract` already exists
  (`Cell::edited`, `Provenance`, the four enums, `YearData`) — **do not change `contract/`**. `core/`,
  `persistence/`, `ingestion/`, `report/`, `docs/method/**` are untouched. Only `deny.toml` may change,
  and only for the clipboard dependency's licence.
- **Colour spent on provenance.** UX colour budget = **the three judgment zones, full stop** (which do
  not exist until 2.6). Provenance/coverage/freshness are carried by **texture, glyph, opacity,
  position** — *zero* colour. Multicolour provenance badges are an explicit anti-pattern.

### Scope — the one-paragraph contract

> 2.4 makes the §2 management grid and §3 A–H table **editable**: type-to-edit + **paste-a-column**,
> **locale-aware** decimal parsing (comma/point + thousands separators) → `Decimal::from_str_exact` →
> `contract::Money`; **cell-cursor keyboard nav** (arrows/Enter/Tab); every commit goes through
> `contract::Cell::edited` with a **manual `Provenance`** from the injected `Clock` and is **persisted**
> via `Journal::put_study`. Each cell shows its **`source × freshness × coverage`** under the
> **attention hierarchy** — **missing shouts** (bold gap glyph/hatch), **stale murmurs** (~60 % opacity
> + hollow dot), **source on demand** (hover/focus), **not-available-accepted** a quiet distinct gap —
> with **no colour spent**. `unknown` is **never** shown or stored as `0`. It builds **no review markers
> / soft-lock (2.5), no engine / verdict / zone bar (2.6), no chart (2.8), no plausibility (2.7), no
> provider/reconciliation (Epic 3)**. Computed columns + §4/§5 results stay caption-only em-dash.

### The data-state model 2.4 renders (and the slice it defers to 2.5)

`contract::Cell` (verified in `contract/src/cell.rs`) is `{ value: Option<Money>, source, freshness,
review, coverage, provenance }`. The independently-queryable axes (FR17–FR20):
- **`source`** = `Provider | Manual | Derived` (FR17) — **2.4 renders on demand**; all 2.4 edits are
  `Manual`.
- **`freshness`** = `Current | Stale` (FR23) — **2.4 renders the stale murmur**; all 2.4 edits are
  `Current` (no provider yet).
- **`coverage`** = `Present | ToFill | NotAvailableAccepted` (FR19) — **2.4 renders all three** and lets
  the user set `NotAvailableAccepted`.
- **`review`** = `None | ToReview | Validated` (FR20) — **NOT 2.4. Story 2.5** renders/sets these + the
  soft-lock. 2.4 leaves the field on the cell (the `Cell::edited` rail manages it) and renders nothing.

The **attention hierarchy** (UX spec lines 247–249, 440–471, 536–551): *"missing shouts, stale murmurs,
review tags speak softly; strong colour reserved for the judgment zones."* Concretely (UX §State & Trust
Markers): **Missing** = a bold neutral glyph / diagonal hatch (a hole in a regular grid shouts on its
own); **Stale** = ~60 % opacity + a hollow dot / slight italic (a discreet murmur); **Source** =
revealed on demand (hover/focus), not always-on. **Asymmetric attenuation is a 2.5/2.6 concern** (it
governs `✓` vs the negative signals across regimes) — 2.4 need only render the gap/stale/source
textures; do not implement regime-driven attenuation of these here beyond what the 2.3 regime token
snapshot already provides.

### Where do the year rows come from? (the key 2.4 interpretation)

A study created by 2.2 has `years: Vec::new()` — **empty**. 2.3 faithfully rendered 5 empty placeholder
rows, but the underlying data is absent. 2.4 must decide how editable year rows exist before the engine
(2.6) or a provider (Epic 3):
- **Recommended:** on first open-for-entry, **materialize a deterministic window of N most-recent
  historical fiscal years** (all cells `Coverage::ToFill`, `value: None`), derived from the study's
  created-at year (e.g. the 5 most recent complete years). A cell flips to `Present` only when entered;
  an untouched cell stays a to-fill gap. Persist the materialized skeleton on first edit (not before —
  avoid writing an all-empty study the user never touched). **Record the exact derivation rule** (which
  year is "year 1", how many years, fiscal-year handling) as an interpretation/issue — it is genuinely
  underspecified and intersects FR with §3 of the method spec.
- **Fix the 2.3 hard cap:** `viewmodel::form::PE_TABLE_ROWS = 5` drops years 6+ (2.3 review LOW + issue
  #20). Render the study's **actual** year set (the materialized window), not a fixed 5; keep the
  faithful look for a short series (pad with to-fill rows only up to the canonical SSG window).
- **Out of scope:** extending the *forward projection* horizon is **Story 2.11**; full year-column
  add/remove management beyond the initial window can be a documented partial — record it.

### Manual provenance: the two underspecified fields

`Cell::edited(new_value, provenance)` takes a `Provenance { source, logical_version, timestamp,
hash_of_dependencies }` verbatim. For a **manually-entered leaf input**:
- `source` = `Source::Manual`; `timestamp` = `clock.now()` (injected — RFC3339 UTC). Settled.
- `logical_version` — there is no obvious app-side counter for a single cell. **Recommended:** use a
  simple, monotonic, defensible value for v1 (e.g. `1`, or the journal's current logical version) and
  **record the choice**; the field earns real meaning in Epic 3 reconciliation. Do not invent a complex
  per-cell version scheme now.
- `hash_of_dependencies` — a **manual leaf has no upstream dependencies**, so the digest is meaningless.
  **Recommended:** a fixed sentinel (e.g. `""` or `"manual"`) and record it; the recompute path (2.6+)
  is where real dependency digests appear. (Provenance does **not** validate these strings — see
  `contract/src/provenance.rs` module doc — so a sentinel is contract-legal.)

File both choices in the 2.4 interpretations issue.

### Persistence: edits are an upsert (FR51 time-series is free)

`Journal::put_study` (verified `persistence/src/studies.rs:35`) is an **upsert** keyed by `study.id`:
`ON CONFLICT(id) DO UPDATE` the payload + bumps `logical_version`, appending the FR51 longitudinal
history on every re-save. So a cell edit is: re-read the `Study`, swap the one `Cell` via
`Cell::edited`, `put_study`. `status`/`method_version` columns are untouched on update (Epic-2 concern).
A study from another journal is rejected (`JournalIdentityMismatch`) — reuse the existing guard. **Reuse
`state.rs`'s read-only / no-journal / save-failure notices** (`MSG_READ_ONLY_WRITE`, `MSG_NO_JOURNAL`,
`MSG_SAVE_FAILED`) — do not invent new error copy unless a genuinely new case appears (then posture-gate
it).

### Locale-aware parsing — the production reverse of `format_amount`

`viewmodel::format::format_amount(canonical, format)` (DISPLAY) already encodes the two presets:
`Comma` → narrow-NBSP `\u{202F}` thousands + `,` decimal; `Point` → `,` thousands + `.` decimal; minus =
`\u{2212}`. 2.4 adds the **inverse** (ENTRY): strip the active preset's thousands separator (and tolerate
a plain ASCII space, which a user may type for NBSP), normalize the decimal separator to `.`, accept
`\u{2212}` and `-`, then `Decimal::from_str_exact`. The Spike-A note (`1-4` review) is explicit: *"Locale/
thousands-separator paste rejected … Story 2.4 locale-aware parsing"* — this task **closes that defer**.
Keep it a pure string transform; `Money`/`Decimal` enforce exactness (no float, no silent rounding,
rejects scientific notation). Blank/ambiguous (`"1.2.3"`, `"--2"`, `""`) → `None`, never `0`.

### The clipboard dependency (`arboard`): dev-dep → runtime dep

`arboard` is currently an `app` **dev-dependency** (example-only, used by `spike_a_grid.rs`;
`app/Cargo.toml:38`, `default-features = false`). Production paste-a-column promotes it to a real
`[dependencies]` entry (still `default-features = false`, text-only). `deny.toml` **already allows**
`BSL-1.0` (clipboard-win on Windows; the spike added it) — so `cargo deny check` should stay green with
no `deny.toml` change; **verify and record**. The Slint-native `Platform::clipboard_text()` is the
alternative (no new runtime dep) but is awkward to reach from app code (Spike-A findings) — pick one,
record which and why. Linux runtime needs `libfontconfig1-dev` (already in CI). This is the **only**
Cargo.lock-affecting change expected in 2.4.

### Existing code being modified / extended (read before writing)

- **`app/src/viewmodel/form.rs`** — the 2.3 `Study → FormHeader`/`PeRow` adapter. Today it reads **only**
  `Cell.value`. 2.4 extends it to also surface each cell's `coverage`/`freshness`/`source` state (new
  struct fields, enum-derived strings/bools — money still a string). **Remove/replace the
  `PE_TABLE_ROWS = 5` cap** to render the real year set. The em-dash `EMPTY_SLOT` stays the present-cell
  "no value" render; the **to-fill gap** gets the shouting glyph (a distinct render from a derived
  caption-only em-dash).
- **`app/src/state.rs`** — the `JournalState` open/list/create/get slice (2.2). Add the **edit/save**
  methods (`edit_cell`/`save_study`) using `Cell::edited` + `put_study`, with the existing guards. The
  injected `Clock`/`IdGen` are already held here — build the manual `Provenance` from `self.clock`.
- **`app/src/viewmodel/format.rs`** — add the `parse_amount` inverse next to `format_amount` (same
  module, same preset table — single source of truth for the two separators).
- **`app/ui/screens/study_screen.slint`** — the 2.3 faithful form. Make the §2/§3 input cells editable
  (new editable-cell component or an editable mode on `GridCell`); render the coverage/stale/source
  textures; keep §1/§4 placeholders and §4/§5 caption-only results. Constant geometry — display↔edit
  must not resize.
- **`app/ui/state.slint`** — the `Studies` global + `PeRow` struct. Add per-cell state fields and the new
  edit callbacks (`commit-cell`, `paste-column`, `set-not-available`); re-export any new struct via
  `app.slint` (the 2.2/2.3 pattern).
- **`app/ui/components/text_field.slint`** — the `TextInput` a11y wrapper (focus ring) to reuse for the
  editable cell. **`app/ui/components/collapsible_section.slint`**, `choice_chip.slint`,
  `action_button.slint` — reuse a11y patterns.
- **`app/ui/tokens.slint` + `app/src/theme.rs`** — add any new **neutral** tokens (gap/hatch ink, stale
  opacity, active-cell cursor ring if not already present) to `Tokens` + both palettes. Never hard-code
  hex/px. The 2.3 `grid-line` + `regime-emphasis` tokens already exist.
- **`app/src/main.rs`** — add the edit callbacks (validate→mutate→persist→re-read→re-push, the 2.3
  shape); update the `unused_crate_dependencies` comment (arboard now runtime-used).
- **`app/src/posture.rs`** — bump the `>= 11` file floor and `>= 60` `@tr` floor; scan the new strings.

### Architecture compliance (guardrails)

- **Cardinal Rule:** no calculation in `app`; the contract→core mapping + compute are **2.6**. 2.4 adds
  **no** calc — only input parsing (string→`Money`) and display.
- **Adapter rule:** money/decimals cross to Slint as formatted strings only (`viewmodel::format`); the
  parse returns a `Money` to `app`, never an `f64` to the UI.
- **Provenance/clock:** the manual `Provenance` timestamp comes **only** from the injected `Clock`
  (ADD15); `data_grid.slint` is the architecture's name for this surface (`architecture.md:695` — "dense
  editable grid, paste-a-column, cell cursor (FR16,56)"). Follow the tree's intent; the 2.3 form lives
  in `study_screen.slint`, so extend there or factor the grid into a `components/` primitive.
- **Errors:** any failure (parse refusal surfaced as a left gap; save failure as a banner) is visible
  and neutral — never a swallowed `.ok()`/`.unwrap()` in non-test app code.
- **Performance (NFR-P4):** inline edit + paste are Slint dirty-driven; a paste of a full column updates
  only touched cells; launch ~within 3 s. No verdict recompute (no engine) so no 100 ms budget applies
  yet (that is 2.6/2.8).

### Neutral voice (FR13 / posture gate)

- The A–H column letters, §1–§5 structure, and formulas are reproducible method, not trademarks —
  **keep them** ([[project_high_fidelity_ssg_forms]], [[project_open_source_naming_constraint]]).
  Neutralize only marks/wordmarks/verbatim prose.
- **Banned verbs:** run every new label (the not-available-accepted action, paste/clear notices, source
  tooltips, any entry hint) through `core::method::BANNED_VERBS_FR/EN` **before wiring**. Coverage/state
  copy is **fact-stating** ("source : manuel", "non disponible — accepté"), never advice. Register
  strings in the scanned slices; do not reduce neutrality to a grep.

### Previous-story intelligence (2.3 dev record + review; Spike A 1.4; 2.2; epic-1 retro)

- **Gates always `--locked`;** clippy `--all-targets --all-features` lints tests + the frozen
  `examples/spike_*.rs` (must keep compiling — `spike_a_grid.rs` still uses `arboard` as a dev-dep path;
  promoting arboard to `[dependencies]` keeps the example compiling — verify). 2.3's review re-ran every
  gate and re-diffed pinned surfaces; expect the same scrutiny.
- **Spike A (1.4) GO** settled the entry-regime feasibility: custom Slint grid (Rust model + virtualized
  `ListView`) + cell-cursor + paste-a-column via `arboard`; **blank/non-numeric → empty, never `0`**.
  Its explicit defers are **2.4 production scope**: locale-aware parsing, typed-edit `Decimal`
  validation (validate on commit, not just paste), spreadsheet-grade nav (wrap, Shift+Tab). Build the
  production grid; reuse the spike's proven parse/fill shape but make it locale-aware and validate the
  typed path too (the spike validated only paste).
- **Visual-verification DoD is load-bearing; the sandbox blocks screenshots/clipboard / may lack AT-SPI**
  — 2.1/2.2/2.3 all recorded a partial AC (process launches + on-disk truth proven, in-GUI click-through
  + clipboard left for human/AT-SPI). Plan the same honesty: prove the **entry→`Cell::edited`→`put_study`
  →reopen** round-trip and the locale parse **headlessly**; record paste/keyboard GUI as needing human
  confirmation if AT-SPI/clipboard is unavailable.
- **File List completeness is the epic's single most-repeated finding (issue #18):** list **every**
  new/modified file (incl. any QA test file + the `_bmad-output/story-automator/…` automator log) with
  refreshed test counts **before** review.
- **`Cell::edited` is the one manual-mutation primitive** (Story 1.11, invariant 2b): returns a NEW cell
  (snapshot semantics), `Some ⇒ Present`, `None ⇒ ToFill`, sets `Freshness::Current`, and the
  divergent-edit `✓→?` demotion (inert until 2.5 sets any `✓`). **Use it — do not hand-roll cell mutation.**
- **Validate-before-mutate + corrupt-safe persistence** (1.10/2.2/2.3): reuse `state.rs` guards +
  `config::load`/`save`; the edit write must validate and persist, never silently drop.
- **Slint gotcha (2.2):** `row` is a reserved layout-attached property — don't name a property `row` in
  the grid (2.2 hit `Cannot override property 'row'`; use `entry`/`r`). **2.3 gotchas:** `@children` in a
  conditional is illegal (fold via clipped height-0); element ids are unreachable from a component-root
  function inside a conditional — declare functions on the in-branch layout.
- **`unused_crate_dependencies` is crate-level allow** (2.2/2.3): arboard becomes runtime-used, so it
  leaves the "unused" set; update the comment-of-record. `ingestion`/`report`/`tokio` stay unused (Epic 3).

### Git intelligence

Recent commits: `feat(story-2.3): Faithful collapsible SSG form …`, `feat(story-2.2): Create, save &
reopen a study …`, `feat(story-2.1): Application shell …`. Conventions: conventional commits
`feat(story-2.4): …`; the story file + `sprint-status.yaml` update land in the **same** commit; merge
only with all four gates green `--locked`. `app/` has real structure (2.1–2.3): `clock.rs`, `config.rs`,
`labels.rs`, `state.rs`, `theme.rs`, `regime.rs`, `posture.rs`, `viewmodel/{format,studies,form}.rs`,
`ui/{tokens,state,app}.slint`, `ui/screens/*`, `ui/components/*` — follow those patterns. `core/`,
`contract/`, `persistence/` must **not** change in this story.

### Project Structure Notes

- **New (app-only):** possibly `app/ui/components/editable_cell.slint`; possibly `app/src/viewmodel/
  entry.rs` (or `parse_amount` inside `format.rs`); new `app` unit tests (locale parse, edit→persist
  round-trip, coverage-state mapping, posture).
- **Modified:** `app/src/viewmodel/form.rs` (per-cell state + remove `PE_TABLE_ROWS` cap),
  `app/src/viewmodel/format.rs` (parse inverse), `app/src/state.rs` (edit/save methods + manual
  provenance), `app/src/main.rs` (edit callbacks + allow comment), `app/src/posture.rs` (floors),
  `app/src/theme.rs` (any new neutral tokens), `app/ui/state.slint` (per-cell state + edit callbacks),
  `app/ui/app.slint` (re-export), `app/ui/screens/study_screen.slint` (editable grids + textures),
  `app/ui/screens/dashboard.slint` (if mounting changes), `app/ui/tokens.slint` (gap/stale/cursor
  tokens), `app/Cargo.toml` (arboard dev-dep → runtime dep), `Cargo.lock` (arboard + transitives),
  `deny.toml` (**only if** a clipboard transitive licence not already allowed appears),
  `sprint-status.yaml`, this story file.
- **Untouched (verify with `git diff` — must be empty):** `core/`, `contract/`, `persistence/`,
  `ingestion/`, `report/`, `docs/method/**`, `.github/workflows/ci.yml`, `rust-toolchain.toml`, the
  frozen `persistence/tests/corpus/v1.db`. **`contract/` is consumed, never modified** — the rail,
  enums, and types already exist.
- **Variance note:** the architecture tree names `app/.../ui/.../data_grid.slint`; 2.3 put the form in
  `study_screen.slint`. Either extend `study_screen.slint` or factor the editable grid into a
  `components/` primitive — document the choice. `state.rs`'s full undo-stack/verdict slice remains 2.9/
  2.6 (a documented partial, as 2.2/2.3 took).

### References

- Story & ACs: `_bmad-output/planning-artifacts/epics.md` § "Story 2.4: Manual data entry with
  provenance & coverage" + Epic 2 intro
- FR16 (enter/override/correct by hand), FR17 (source), FR18 (provenance+timestamp), FR19 (coverage
  present/to-fill/not-available-accepted), FR63 (locale number format), FR65 (offline): `_bmad-output/
  planning-artifacts/prd.md` § "Functional Requirements" + Journey 2 (CH/EU partial coverage) lines
  299–313
- Attention hierarchy (missing shouts / stale murmurs / source on demand), provenance-as-texture-never-
  colour, two regimes, N/A vs 0 vs empty distinct, State & Trust Markers: `_bmad-output/planning-
  artifacts/ux-design-specification.md` lines 62–66, 247–249, 280–291, 436–471, 505–551; mockup
  `ux-stock-study-screen.html`
- Crate boundaries / Cardinal Rule, adapter (money-as-strings), `data_grid.slint`, clock injection
  (ADD15), Decimal-as-string-in-JSON, app-config-vs-journal: `architecture.md` § "Project Structure &
  Boundaries", § "Frontend Architecture", § "Core Technical Decisions" (lines 504–519, 695, 719)
- The manual-mutation rail (consume, don't modify): `contract/src/cell.rs` (`Cell::edited`, `Source`/
  `Freshness`/`Review`/`Coverage`), `contract/src/provenance.rs` (`Provenance`/`Timestamp` — no
  validation, sentinel-legal), `contract/src/study.rs` (`Study`/`YearData` shapes)
- Persistence upsert (FR51 time-series on every put): `persistence/src/studies.rs:35` (`put_study`)
- Injected clock/identity (the manual-provenance time source): `app/src/clock.rs`
- Locale formatter to invert + entry parse reuse: `app/src/viewmodel/format.rs` (`format_amount`,
  `NumberFormat`); the read-only adapter to extend: `app/src/viewmodel/form.rs`
- Spike A GO + its 2.4 defers (locale parse, typed-edit validation, spreadsheet nav) + clipboard via
  `arboard`: `_bmad-output/implementation-artifacts/1-4-spike-a-dense-editable-grid-paste-a-column.md`;
  findings `docs/spikes/spike-a-dense-grid.md`
- Prior story (the read-only form this makes editable) + its review LOWs (§3 fixed-5 cap, summary-row
  alignment) → issue #20: `2-2-create-save-reopen-study.md`, `2-3-faithful-collapsible-ssg-form.md`
- Tokens/components/viewmodel/a11y patterns to reuse: `app/ui/tokens.slint`, `app/ui/state.slint`,
  `app/src/theme.rs`, `app/src/viewmodel/{format,form,studies}.rs`, `app/ui/components/*.slint`
- File List discipline (issue #18), 2.3 interpretations (issue #20), banned verbs:
  `core::method::BANNED_VERBS_EN/FR`; memories [[project_high_fidelity_ssg_forms]],
  [[project_open_source_naming_constraint]], [[project_gui_framework_slint_only]]

### Tech currency note (2026-06-13)

Slint pinned at 1.16. The **one** dependency change is `arboard` dev-dep → runtime dep (text-only,
`default-features = false`) for production paste-a-column — `deny.toml` already allow-lists its BSL-1.0
Windows transitive (Spike A); `cargo deny check` should stay green with no `deny.toml` change (verify,
record). No other new external crate is expected. Parsing reuses `rust_decimal`/`contract::Money`
(already in the tree). The §1 chart (2.8) and §4 zone bar / engine (2.6) remain out of scope.

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8 (1M context) — `claude-opus-4-8[1m]`, via the BMAD `dev-story` workflow.

### Debug Log References

- **Slint reserved-property gotcha (carried from 2.2):** `MgmtInputRow` first declared `in property
  <MgmtRow> row;` → `Cannot override property 'row'`. Renamed to `mgmt` (the 2.2 lesson: `row` is a
  reserved layout-attached property).
- **clippy `type_complexity`:** the §2 `mgmt_rows` first used an array of `(&str, fn(&YearData) ->
  Option<&Cell>)` accessors → `-D warnings` failed. Refactored to iterate `entry::MGMT_FIELDS` and read
  each cell via `entry::get_cell(year, field)` (no fn pointers) — also removes duplicated field knowledge.
- **`cargo fmt`** reflowed two long `assert_eq!` calls in `form.rs` tests; re-ran `cargo fmt --all`.
- **`TextInput` capability check:** confirmed (in `i-slint-compiler-1.16.1/builtins.slint`) that the
  builtin `TextInput` exposes `key-pressed(KeyEvent) -> EventResult` and `select-all()` — the basis for
  the per-cell editor (intercept arrows / Ctrl+V before the default insert; select-all-on-focus so
  typing replaces).

### Completion Notes List

Implemented Story 2.4 **`app`-only** (the `contract` rail — `Cell::edited`, `Provenance`, the four
enums, `YearData` — already existed and was consumed, not modified). All seven tasks complete; all four
gates green `--locked`.

- **Task 1 — locale entry parser.** `viewmodel::format::parse_amount(input, format) -> Option<Money>`,
  the production inverse of `format_amount`: strips the preset's thousands separator (NBSP / comma / a
  plain space), maps the decimal separator to `.`, accepts `\u{2212}` and `-`, then
  `Decimal::from_str_exact`. Blank/ambiguous/non-numeric → `None`, **never `0`**. Round-trip
  `parse_amount(format_amount(x))` proven value-stable for both presets.
- **Task 2 — editable cell + cursor + paste.** New `app/ui/components/editable_cell.slint`: an inline
  `TextInput` on the faithful grid with constant geometry; the active cell shows a brighter neutral
  surface + 1px ink ring (the cell cursor — **no colour spent**). Keyboard: arrows + Enter move the
  cursor (Rust `cell-move` index math → `active-year`/`active-field` → the target cell focuses itself),
  Tab/Shift+Tab move horizontally (native), Ctrl+V pastes a column, Ctrl+Space toggles not-available,
  Backspace/Delete clears to a gap. Ctrl/Cmd chords are intercepted first so they never leak a character
  (the Spike-A fix). Paste reads the clipboard via **`arboard`** (promoted dev→runtime dep) and fills
  consecutive years of the same field; surplus lines past the grid bottom are dropped with a neutral notice.
- **Task 3 — edits → `Cell::edited` → persist.** `JournalState::edit_cell` / `set_not_available` /
  `paste_column` route every commit through `contract::Cell::edited` with a manual `Provenance` from the
  injected `Clock` (`source = Manual`, `freshness = Current`), then `put_study` (the upsert; bumps
  `logical_version`, appends the FR51 series). Read-only / no-journal / save-failure reuse the existing
  neutral notices — **no silent `.ok()`**. The year-grid is materialized **on first edit** (the 5
  complete years before the created-at year); `PE_TABLE_ROWS = 5` cap removed (issue #20).
- **Task 4 — `source × freshness × coverage` display.** The form adapter now crosses each cell's state
  (coverage/stale/source) as enum-derived strings/bools (money still a formatted string). The screen
  renders the attention hierarchy with **zero colour**: a to-fill gap **shouts** (bold neutral glyph),
  not-available is a **quiet** "n/a", stale **murmurs** (~60% opacity + hollow dot), source is **revealed
  on demand** (a section caption bound to the focused cell). New neutral tokens (`cell-active`, `gap-ink`,
  `gap-glyph`, `stale-dot`, `stale-opacity`) in `Tokens` + both palettes.
- **Task 5 — wire into the screen.** `study_screen.slint` mounts editable §3 A/B/C/F cells and §2
  sales/pre-tax/book rows (with dynamic year headers); computed D/E/G/H + PTP/ROE + §4/§5 results stay
  caption-only em-dash. `main.rs` adds `commit-cell`/`paste-column`/`set-not-available`/`cell-move`
  callbacks (validate→mutate→persist→re-read→re-push, the 2.3 one-source-of-truth shape) and updates the
  `unused_crate_dependencies` comment (arboard + rust_decimal now runtime-used).
- **Task 6 — posture, a11y & gates.** Posture floors bumped (≥13 `.slint` files, ≥90 `@tr` literals,
  message inventory 8→10 incl. the two new entry notices); every new label is fact-stating French and
  passes `BANNED_VERBS_FR/EN`. Active cursor + paste + NA gesture are `FocusScope`/`TextInput`-operable
  with a visible focus ring; the grid reads with colour stripped (glyph/opacity/dot carry meaning).
- **Task 7 — verification & records.** Headless proof (the load-bearing slice): `parse_amount` +
  `parse_pasted_column` locale tests, and the **entry → `Cell::edited` (source=manual, Present) →
  `put_study` → reopen** round-trip, plus clear→to-fill and not-available→reopen round-trips. App tests
  **53 → 71** (+18). `cargo run` launched the app under the live display and ran its event loop for the
  full timeout without panic (window mapped). **AC6 honesty:** the in-GUI click-through — type/paste/
  keyboard-nav render, source-on-focus reveal, NA gesture, dark/light + label/locale swaps, fold/regime
  restore, ~3 s launch — is **load-bearing and left for human confirmation** under the headless sandbox
  (no AT-SPI/clipboard driving), exactly as 2.1/2.2/2.3 recorded; the on-disk round-trip + locale parse
  ARE proven headlessly. No cell becomes `Stale` in 2.4 except via test fixtures (Epic 3 sets stale) — the
  stale-murmur texture is built but barely exercised, stated honestly, not faked.

**Gates (all green `--locked`):** `cargo fmt --all --check` · `cargo clippy --all-targets --all-features
-- -D warnings` · `cargo test --all` (app 71/71; workspace all green) · `cargo deny check` (advisories /
bans / licenses / sources ok). **Pinned surfaces** (`core/ contract/ persistence/ ingestion/ report/
docs/method/ .github/ rust-toolchain.toml`, frozen `v1.db`, `deny.toml`) `git diff` **empty**.
**`Cargo.lock` unchanged** — `arboard` and `rust_decimal` were already dev-deps (already in the lock);
promoting them to runtime deps changes only the dependency *kind*, not the resolved set, so `deny.toml`
needed no edit (the BSL-1.0 clipboard-win transitive was already allow-listed in Spike A). Interpretations
& documented partials filed as **GitHub issue #21** (year-grid rule, provenance sentinels, NA gesture,
clipboard path, paste-column transposed semantics, cursor-nav/stale partials).

### File List

**New (`app`-only):**
- `app/src/viewmodel/entry.rs` — field addressing, year-grid materialization, cell-cursor move math,
  `source/coverage` string mapping, `parse_pasted_column` (locale-aware) + tests.
- `app/ui/components/editable_cell.slint` — the editable cell (active-cursor, textures, keyboard, paste,
  NA gesture, source-on-focus).

**Modified (`app`-only):**
- `app/src/viewmodel/format.rs` — `parse_amount` (locale entry inverse) + tests.
- `app/src/viewmodel/form.rs` — per-cell `GridCellState` + `mgmt_rows` + `year_headers`; removed the
  `PE_TABLE_ROWS = 5` cap; tests.
- `app/src/viewmodel/mod.rs` — register `pub mod entry;`.
- `app/src/state.rs` — `edit_cell` / `set_not_available` / `paste_column` / `manual_provenance` /
  `mutate_cell`; two new neutral notices; round-trip tests.
- `app/src/theme.rs` — `cell_active` / `gap_ink` palette fields (both themes) + `apply`.
- `app/src/main.rs` — `push_form` helper; `commit-cell`/`paste-column`/`set-not-available`/`cell-move`
  callbacks; open-study resets the cursor; `unused_crate_dependencies` comment-of-record updated.
- `app/src/posture.rs` — bumped `.slint` file floor (≥13), `@tr` floor (≥90), message count (8→10).
- `app/ui/state.slint` — `GridCellState` + `MgmtRow` structs; `PeRow` (A/B/C/F now `GridCellState`);
  `year-headers`/`mgmt-rows`/`active-year`/`active-field`/`active-source` props; entry callbacks.
- `app/ui/app.slint` — re-export `GridCellState`, `MgmtRow`.
- `app/ui/screens/study_screen.slint` — editable §3 A/B/C/F + §2 raw-input rows (dynamic year headers);
  source caption + entry hint; `MgmtInputRow` / `PeTableRow` editable cells.
- `app/ui/tokens.slint` — `cell-active`, `gap-ink`, `gap-glyph`, `stale-dot`, `stale-opacity` tokens.
- `app/Cargo.toml` — `arboard` and `rust_decimal` promoted from `[dev-dependencies]` to `[dependencies]`.

**Story tracking:** `_bmad-output/implementation-artifacts/2-4-manual-data-entry-provenance-coverage.md`
(this file), `_bmad-output/implementation-artifacts/sprint-status.yaml`,
`_bmad-output/story-automator/orchestration-2-20260612-123914.md` (automator run log — issue #18 discipline).

**Unchanged (verified `git diff` empty):** `core/`, `contract/`, `persistence/`, `ingestion/`, `report/`,
`docs/method/**`, `.github/`, `rust-toolchain.toml`, frozen `persistence/tests/corpus/v1.db`, `deny.toml`,
`Cargo.lock`.

## Change Log

| Date       | Version | Description                                                                 |
|------------|---------|-----------------------------------------------------------------------------|
| 2026-06-13 | 0.1     | Story 2.4 implemented: spreadsheet-grade manual entry (type + paste-a-column, locale-aware parse → `contract::Money`), each edit via `Cell::edited` stamped `source=manual` + persisted (`put_study` upsert); `source × freshness × coverage` display under the attention hierarchy (no colour spent); `unknown` never `0`. `arboard` + `rust_decimal` dev→runtime deps (Cargo.lock unchanged). App tests 53 → 71. Interpretations → issue #21; `PE_TABLE_ROWS=5` cap removed (issue #20). Status → review. |
| 2026-06-13 | 0.2     | Adversarial code review (story-automator, auto-fix). All four gates re-run green `--locked` (fmt · clippy `-D warnings` · `cargo test --all` app 71/71 · `cargo deny check`); pinned surfaces + `Cargo.lock` re-diffed empty; entry→`Cell::edited`→`put_study`→reopen round-trip + locale parse re-confirmed headlessly. No CRITICAL/HIGH/MEDIUM defects. Auto-fixed 2 LOW items: removed an empty-`if` smell in `editable_cell.slint` `commit()`; added the automator run log to the File List (issue #18 discipline). Status → done. |

## Senior Developer Review (AI)

**Reviewer:** Guy · **Date:** 2026-06-13 · **Outcome:** Approve (status → done)

Adversarial review against the 6 ACs and the 7-task File List. Read every changed `app` source + `.slint`
file; re-ran all four quality gates `--locked` and re-diffed the pinned surfaces.

**Verified green:**
- **AC1/AC3** — `parse_amount` (locale inverse of `format_amount`) + `parse_pasted_column` are pure
  string→`Decimal::from_str_exact`→`Money`; blank/ambiguous/non-numeric → `None`, **never `0`**;
  round-trip `parse(format(x))` value-stable for both presets. Every commit routes through
  `contract::Cell::edited` with a clock-stamped manual `Provenance`, then `put_study` (upsert). Read-only /
  no-journal / save-failure reuse the neutral notices — no silent `.ok()`. Headless round-trip
  (edit → reopen) proves `source=Manual`, `Freshness::Current`, `Coverage::Present` survive persistence.
- **AC2** — per-cell `coverage`/`stale`/`source` cross as enum-derived strings/bools; the attention
  hierarchy is glyph/opacity/dot/focus-caption only — **zero colour spent**; N/A vs 0 vs to-fill kept
  distinct in both the contract round-trip and the render.
- **AC4** — no calculation in `app`; D/E/G/H + §2 ratios + §4/§5 stay caption-only em-dash; money crosses
  to Slint only as formatted strings; `Provenance` stamped from the injected `Clock`.
- **AC5** — all four gates green `--locked`; `arboard`+`rust_decimal` dev→runtime (resolved set unchanged,
  `Cargo.lock` empty diff, `deny.toml` untouched); posture floors bumped (≥13 `.slint`, ≥90 `@tr`, 10
  messages) and pass; new labels fact-stating French.
- **AC6** — the in-GUI click-through (type/paste/keyboard-nav render, source-on-focus, NA gesture, theme/
  locale swaps, ~3 s launch) is honestly recorded as **load-bearing, left for human confirmation** under
  the headless sandbox; the on-disk round-trip + locale parse ARE proven headlessly. Same honesty rail as
  2.1/2.2/2.3.

**Findings — 0 CRITICAL, 0 HIGH, 0 MEDIUM, 2 LOW (both auto-fixed):**
- **LOW** (fixed): `editable_cell.slint` `commit()` wrapped the `commit-cell` callback in an empty `if`
  body, evaluating then discarding the `written?` bool — replaced with a bare call + an explaining comment.
- **LOW** (fixed): the File List omitted the modified `_bmad-output/story-automator/orchestration-…md`
  run log — added under Story tracking (issue #18 discipline).

**Noted, not changed:** `viewmodel/form.rs` re-declares a one-line `created_at_date` copy of
`state::created_at_date` rather than importing it — a documented, defensible choice (keeps the adapter
free of a form→state coupling for a trivial pure transform); left as-is. The §2 paste fills consecutive
*years of a field* (a data-model column), which reads horizontally in the transposed §2 grid — already
recorded as a documented partial in issue #21.
