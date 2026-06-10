# Story 1.4: Spike A — dense editable grid with paste-a-column (go/no-go)

Status: review

<!-- Note: THROWAWAY SPIKE. Deliverable = a GO/NO-GO decision + findings note, NOT production code. -->
<!-- Epic 1. Run after 1.5 (Spike B GO is locked); this settles the entry-regime feasibility. -->
<!-- Validation is optional. Run validate-create-story for a quality check before dev-story. -->

## Story

As the developer (Guy, solo),
I want to prove a Slint **dense editable grid** supports spreadsheet-grade entry — keyboard cell-cursor navigation **and pasting a whole column of year-values**,
so that the **entry-regime feasibility is settled** (and the "custom-grid-on-Slint" approach is locked, or a fallback chosen) **before** the study UI is built in Epic 2.

## Acceptance Criteria

1. **A throwaway Slint example renders a dense editable grid.** A small SSG-style grid (≈10 data rows × a few year-columns) built the way **production will build it** per the locked architecture: a Rust-side model + a **virtualized `ListView`** of rows with per-cell inline edit — **not** `StandardTableView` (that may be tried for comparison, but the deliverable proves the custom approach). Visual realism: **row height 28 px**, a **visible cell grid** (light separators, no aggressive gridlines), right-aligned **tabular figures**. [Source: architecture.md "Grid = custom on Slint (Rust TableModel + virtualized ListView…)"; ux-design-specification.md UX-DR5/UX-DR8]
2. **Keyboard cell-cursor navigation works.** Arrow keys move the active cell; **Enter/Tab commit + advance**; typing edits the focused cell inline. The active cell shows a visible cursor (**brighter surface + 1 px ink ring**), no colour spent. [Source: ux UX-DR5/UX-DR8]
3. **Paste-a-column lands values in the correct cells, parsed as `Decimal` (THE make-or-break test).** With a cell focused, **Ctrl+V** pastes a clipboard column of **≥10 newline-separated values**; the values fill consecutive cells downward from the cursor, each parsed **exactly** as a decimal (reuse `rust_decimal::Decimal::from_str_exact`, wrap in `contract::Money`). A blank or non-parseable entry is **flagged/left empty — never coerced to `0`** (mirrors the `Cell` "missing ≠ 0" rule). The clipboard-read mechanism is documented. [Source: epics.md Story 1.4 AC; contract/src/cell.rs module doc; contract/src/money.rs]
4. **Explicit GO / NO-GO conclusion.** The spike ends with a written **GO/NO-GO** note (suggested: `docs/spikes/spike-a-dense-grid.md`) recording **how the clipboard was read**, what worked / didn't, and the decision. **NO-GO triggers a documented alternative** (e.g. a dedicated paste-target `TextEdit` parsed on `edited`, `StandardTableView`, or a different clipboard path) **before** the study UI (Epic 2 Stories 2.3/2.4) is committed — **NOT web, NOT egui**. [Source: epics.md Story 1.4 AC + "spikes are throwaway: deliverable = go/no-go + note"]
5. **Throwaway & isolated.** The spike lives in `app/examples/spike_a_grid.rs` (inline `slint::slint!{ … }` macro — no `build.rs` change), runs via `just spike-a` / `cargo run -p steadyinvest-app --example spike_a_grid`, does **not** touch production code, and **passes all repo gates** (fmt, clippy `-D warnings` over `--all-targets`, test, `cargo deny`) so CI stays green.

## Tasks / Subtasks

- [x] **Task 1 — Throwaway dense-grid Slint example (AC: 1, 5)**
  - [x] Add `app/examples/spike_a_grid.rs` using the inline `slint::slint!{ … }` macro (no `app/build.rs` change — keeps it isolated and deletable).
  - [x] Model the grid in **Rust** (a `VecModel<GridCell>` exposed as `ModelRc`, mirroring the production "Rust `TableModel` + virtualized `ListView`" approach); render rows with a Slint nested `for`-grid, each row a horizontal run of cells.
  - [x] Style for realism: row height **28 px**, visible cell borders, right-aligned digits; neutral labels only (**no NAIC marks/logos/verbatim prose**).
- [x] **Task 2 — Keyboard cell-cursor + inline edit (AC: 2)**
  - [x] Track the active cell index; a `FocusScope` `key-pressed` handler moves it with arrows, advances on **Enter (down) / Tab (right)**, edits inline on type (digits/`.`/`,`/`-`), deletes on Backspace.
  - [x] Active-cell cursor visual: brighter surface + **1 px ink ring** (no colour).
- [x] **Task 3 — Paste-a-column (AC: 3) — the make-or-break**
  - [x] Capture **Ctrl+V** (FocusScope `key-pressed`) → Rust `paste` callback reads clipboard text via **`arboard`** (`Clipboard::new()?.get_text()`), added as an `app` **dev-dependency** (example-only, `default-features = false`). (Slint-native `Platform::clipboard_text()` noted as the alternative; arboard chosen for app-level reachability.)
  - [x] `parse_pasted_column` splits on `\n`/`\r\n`, trims, parses each with `Decimal::from_str_exact` → `Money`; fills consecutive cells **downward from the cursor**. Blank/unparseable → cell left **empty, never `0`**. Unit-tested (5 tests).
  - [x] CH/EU reality recorded: a decimal **comma** (`1,5`) is **not** silently accepted by the canonical parse (test asserts it → `None`); locale-aware entry is a production concern (Story 2.4), not this spike's pass/fail.
- [x] **Task 4 — Run wiring (AC: 5)**
  - [x] Added a `spike-a` recipe to the `justfile` → `cargo run -p steadyinvest-app --example spike_a_grid`; renamed the existing `spike` recipe → `spike-b` for symmetry (`spike_b_chart.rs` untouched).
  - [x] Confirmed the example compiles and `cargo clippy --all-targets --all-features --locked -- -D warnings` stays clean.
- [x] **Task 5 — GO/NO-GO findings note (AC: 4)**
  - [x] Created `docs/spikes/spike-a-dense-grid.md`: what was built, clipboard read via `arboard`, the unit-test evidence, the on-display verification steps, and the GO/NO-GO checklist. **Perceptual verdict + final decision left for Guy** to fill after running on a display (headless here — see caveat).

## Dev Notes

### This is a SPIKE — what "done" means
The **deliverable is the GO/NO-GO decision**, not shippable code. The example is throwaway — deleted/ignored once Epic 2's real grid (Stories 2.3 faithful collapsible form, 2.4 manual data entry) exists. Optimise for *answering the question* (does a custom Slint grid support keyboard entry + **paste-a-column**?), not code polish — but keep it gate-clean so CI passes. [Source: epics.md Epic 1 "(Spikes are throwaway: their deliverable is a go/no-go decision + a short findings note)"; architecture.md "Week-1 spikes"]

### The locked technical approach (and the fallback)
- **Grid = custom on Slint:** a Rust-side model (`VecModel`/`ModelRc`, the spike's stand-in for the production `TableModel`) + a **virtualized `ListView`**; cell-cursor keyboard nav, inline edit, **paste-a-column**. **Paste-a-column is explicitly the make-or-break test.** [Source: architecture.md "Grid = custom on Slint (Rust TableModel + virtualized ListView…)", "Week-1 spikes: (A) grid — paste-a-column is the make-or-break"]
- **Clipboard read is the real unknown.** Slint exposes `slint::platform::Clipboard` + `Platform::clipboard_text()` at the *platform* layer (added by 1.16-era releases), which is awkward to reach from app code; Slint's `TextInput` handles Ctrl+V *into a field* internally. So the pragmatic spike path is **`arboard`** (well-vetted, cross-platform: Linux/X11+Wayland, macOS, Windows) called from a Ctrl+V key handler. The point of the spike is to confirm *one* reliable path; record it. [Source: docs.slint.dev `slint::platform::Clipboard`; WebSearch 2026-06]
- **If NO-GO** (e.g. clipboard unreadable, or `ListView`-grid can't do spreadsheet-grade nav), the fallback (record which, and why): a dedicated **paste-target `TextEdit`** the user pastes into, parsed on `edited`; or `StandardTableView` (editable text cells exist) with its nav limitations; or a different clipboard mechanism. **Never egui, never web.** [Source: epics.md Story 1.4 AC; architecture.md Core Technical Decisions; project memory GUI = Slint-only]

### Exact-decimal parsing — reuse, don't reinvent
- Parse pasted values with **`rust_decimal::Decimal::from_str_exact`** and wrap in **`contract::Money`** (`steadyinvest-contract`). `Money` already enforces exact, canonical, string-based decimals (errors instead of silently rounding; rejects floats/scientific notation). **Do the value parse in `Decimal`, never `f64`** (only pixel/layout math may use floats — rendering ≠ decision chain). [Source: contract/src/money.rs; architecture.md Cardinal Rule]
- A blank or unparseable pasted line maps to **no value** (`None`) — the `Cell` model makes "missing/insufficient" first-class and **never coerces to 0**. The spike should visibly distinguish an empty cell from a `0`. [Source: contract/src/cell.rs module doc — "A missing value is `value: None` … never coerced to 0"]
- `core` exposes method constants if useful (`USABLE_YEARS_FLOOR=5`, `FORECAST_HORIZON_YEARS=5`, `LOAD_BEARING_YEAR_FIELDS = [sales, eps, high_price, low_price]`) — handy for labelling realistic SSG rows/columns, though the spike may stay self-contained. [Source: core/src/method/mod.rs]

### Headless caveat (same as Spike B)
This environment has **no display**: the event loop starts but the **perceptual verdict (keyboard feel, paste lands correctly) must be made by Guy** on his desktop. The agent's job: make it compile + gate-clean, wire the clipboard read + parse, and write the findings template with the decision left for Guy. Linux Slint runtime needs `libfontconfig1-dev` (already in CI). [Source: 1-5 dev record; architecture.md]

### UX target the spike emulates (entry/reconciliation regime)
- **Excel is the entry gold standard:** instant inline editing, pure keyboard nav, **paste a whole column**, dense yet legible. The grid + chart are the two genuinely custom-heavy pieces. [Source: ux-design-specification.md "Excel — entry gold standard", "Bespoke components"]
- Dense grid: row **28 px**, cell padding 4 v / 8 h, zebra ~4 % *or* a focus/hover step + crisp 1 px ink ring; multi-cell selection is a later refinement (paste-a-column is the spike's focus). [Source: ux UX-DR5/UX-DR8]
- The `✓`/`?` tri-state review markers and source×freshness textures are **out of scope** for this spike (they land with the real grid in Epic 2). Keep the spike to: render + keyboard nav + paste. [Source: contract/src/cell.rs; ux two-regime delta]

### Previous story intelligence (from 1-1 / 1-2 / 1-3 / 1-5 dev records)
- **MSRV `1.96`** (`rust-toolchain.toml`; floor driven by Slint 1.16 transitive deps + `libsqlite3-sys`). The architecture doc's "1.88" is stale — use **1.96**. CI is **Linux-only**; gates run **`--locked`**; clippy `-D warnings` covers **`--all-targets`** (so the example *is* linted — keep it clean). [Source: rust-toolchain.toml; 1-5 dev record; project memory Linux-only]
- **`app` crate = `steadyinvest-app`**; Slint **1.16** is its pinned UI dep. The `slint!` **inline macro** avoids touching `app/build.rs` (which compiles `ui/app.slint`). Examples inherit `app`'s deps and may use its **`[dev-dependencies]`** (where `rust_decimal` already lives for the 1.5 spike — add `arboard` there too, example-only, so the shipping binary stays lean). [Source: app/Cargo.toml; 1-5 dev record]
- **Pattern set by Spike B (1.5, GO locked):** throwaway `app/examples/*.rs` + inline `slint!` + a `docs/spikes/*.md` findings note + a `justfile` recipe; production code untouched; gates green. Follow the same shape. [Source: 1-5-spike-b-native-slint-draggable-judgment-line-recolor.md]
- **Don't silence errors** (a clipboard/parse error must surface, not `.ok()`-swallow) and **"done" = it visibly works**, not "it compiles" — the cautionary lesson behind the project's visual-verification rule. [Source: docs/lessons-learned-chart-rendering.md (old project, principle still applies)]

### Project Structure Notes
- **New (throwaway / docs):** `app/examples/spike_a_grid.rs`; `docs/spikes/spike-a-dense-grid.md`. **Modify:** `justfile` (add `spike-a`); `app/Cargo.toml` (add `arboard` dev-dependency, example-only); `_bmad-output/implementation-artifacts/sprint-status.yaml` (1-4 status transitions).
- **Do NOT modify** `app/ui/app.slint`, `app/src/main.rs`, `app/examples/spike_b_chart.rs`, or any crate's production code. **No new runtime/workspace dependencies** (`arboard` is dev-only). The production grid will live at `app/.../ui/.../data_grid.slint` and is built in **Epic 2** — this spike must not pre-empt it. [Source: architecture.md crate layout "data_grid.slint — dense editable grid, paste-a-column, cell cursor (FR16,56)"]
- If `cargo deny` flags an `arboard` transitive licence, record it in the findings note and pick the leanest acceptable clipboard crate (or fall back to the Slint-native/TextEdit path) — keep the tree GPL-3.0-compatible. [Source: deny.toml; project memory open-source/licensing]

### References
- [Source: epics.md#Story 1.4: Spike A] — user story + AC + GO/NO-GO + fallback; "paste-a-column is the make-or-break test"
- [Source: architecture.md#Core Technical Decisions / Frontend] — Grid = custom on Slint (Rust TableModel + virtualized ListView), Week-1 spikes A/B/C, fallback NOT egui/web
- [Source: ux-design-specification.md UX-DR5 / UX-DR8; "Excel — entry gold standard"; Two Regimes] — dense grid 28 px, cell-cursor, paste-a-column, tabular figures
- [Source: contract/src/money.rs] — exact `Decimal`/`Money` parse (`from_str_exact`, no floats, canonical)
- [Source: contract/src/cell.rs] — missing ≠ 0; tri-state review is Epic-2 scope
- [Source: core/src/method/mod.rs] — SSG constants for realistic row/column labelling (optional reuse)
- [Source: 1-5-spike-b-native-slint-draggable-judgment-line-recolor.md] — the throwaway-example + findings-note + justfile pattern; gates; MSRV 1.96; Linux-only; headless caveat
- [Source: docs.slint.dev `slint::platform::Clipboard`] — Slint-native clipboard at the Platform layer; `arboard` is the pragmatic app-level alternative

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8 (1M context) — claude-opus-4-8 — via Claude Code dev-story (2026-06-10).

### Debug Log References

- `cargo build -p steadyinvest-app --example spike_a_grid` → compiles (after 2 fixes: a Slint hex colour ending in `e` confused the Rust lexer → changed `#23262e`→`#242832`; added `use slint::Model` for `set_row_data`).
- `cargo test -p steadyinvest-app --example spike_a_grid` → **5/5 logic tests pass** (parse + column-fill).
- Gates green: `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --all --locked`, `cargo deny check` (arboard licenses pass; `clipboard-win` BSL-1.0 already allow-listed).
- Headless here (no display / no clipboard) — the **perceptual GO/NO-GO is Guy's call** on a desktop session (`just spike-a`).

### Completion Notes List

- Built the throwaway `app/examples/spike_a_grid.rs` (inline `slint::slint!`, no `build.rs` change): a dense 10×4 grid over a Rust `VecModel<GridCell>` (stand-in for the production `TableModel`), `FocusScope` keyboard cell-cursor (arrows / Enter↓ / Tab→ / type-to-edit / Backspace), active-cell 1px ink ring, 28px rows.
- **Paste-a-column** (the make-or-break) reads the clipboard via **`arboard`** on Ctrl+V; `parse_pasted_column` parses each line **exactly** (`Decimal::from_str_exact` → `contract::Money`) and fills the current column downward. **Blank / non-numeric → empty cell, never `0`** (the `Cell` "missing ≠ 0" rule), asserted by 5 unit tests including the CH/EU decimal-comma case (rejected → locale handling deferred to Story 2.4).
- `arboard` added as an `app` **dev-dependency** (`default-features = false`, text-only) + a `[workspace.dependencies]` pin; **shipping binary stays lean**. No production code touched (`app/ui/app.slint`, `app/src/main.rs`, `app/examples/spike_b_chart.rs`, all crates untouched). `justfile`: added `spike-a`, renamed `spike`→`spike-b`.
- ✅ **VERDICT: GO (2026-06-10, Guy's on-display run).** All three checks passed on screen: keyboard cell-cursor navigation works, **paste-a-column lands the values in the correct cells**, and blank/non-numeric cells stay **empty (never 0)**. The **entry-regime feasibility is settled** — Epic 2 (Stories 2.3/2.4) builds the production grid as a `TableModel` + virtualized `ListView` with locale-aware parsing + tri-state review markers. Findings in `docs/spikes/spike-a-dense-grid.md`.

### File List

**Added (throwaway / docs):**
- `app/examples/spike_a_grid.rs` (throwaway dense-grid + paste-a-column spike)
- `docs/spikes/spike-a-dense-grid.md` (GO/NO-GO findings note — decision pending Guy's run)

**Modified:**
- `Cargo.toml` (added `arboard` to `[workspace.dependencies]`, text-only)
- `app/Cargo.toml` (added `arboard` dev-dependency for the example)
- `justfile` (added `spike-a`; renamed `spike`→`spike-b`)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (1-4 → in-progress → review)

## Change Log

| Date | Change |
|------|--------|
| 2026-06-10 | Story 1.4 created (ready-for-dev): throwaway custom-Slint dense editable grid spike — keyboard cell-cursor nav + **paste-a-column** parsed as exact `Decimal` → GO/NO-GO. Clipboard read via `arboard` (primary) or Slint-native `Platform::clipboard_text()`. Follows the Spike B (1.5) pattern; production grid deferred to Epic 2 (Stories 2.3/2.4). |
| 2026-06-10 | Story 1.4 implemented: `app/examples/spike_a_grid.rs` (dense grid, `FocusScope` cell-cursor, `arboard` paste-a-column, exact-`Decimal` parse, blank≠0) + 5 logic unit tests + `just spike-a` + findings doc. Builds, clippy-clean, all gates green. Status → review. |
| 2026-06-10 | **GO** (Guy's on-display run): keyboard nav, paste-a-column, and blank≠0 all confirmed. Entry-regime feasibility settled; production grid → Epic 2 (Stories 2.3/2.4). |
