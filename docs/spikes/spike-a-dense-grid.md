# Spike A — dense editable grid + paste-a-column

**Story:** 1.4 · **Date:** 2026-06-10 · **Type:** throwaway spike (go/no-go).
**Question:** can a **native Slint dense grid** support spreadsheet-grade entry — keyboard cell-cursor
navigation, type-to-edit, and **pasting a whole column of year-values** that land in the correct cells
parsed as exact `Decimal` — well enough to build the entry regime on? No web, no egui.

## What was built

`app/examples/spike_a_grid.rs` (throwaway, inline `slint::slint!` markup; run with `just spike-a` or
`cargo run -p steadyinvest-app --example spike_a_grid`):
- A **dense 10×4 grid** rendered with a Slint `for`-grid over a Rust `VecModel<GridCell>` (the
  production approach is a `TableModel` + virtualized `ListView`; this fixed grid stands in for it).
  Row height **28 px**, visible cell borders, right-aligned figures, dark theme.
- **Keyboard cell-cursor navigation** via a `FocusScope`: arrows move the active cell; **Enter** moves
  down; **Tab / →** moves right; the active cell gets a **brighter surface + 1 px ink ring** (no colour
  spent). **Type-to-edit** appends digits / `.` / `,` / `-` to the active cell; **Backspace** deletes.
- **Paste-a-column (the make-or-break):** **Ctrl+V** is captured in the `FocusScope`, the clipboard is
  read via **`arboard`** (text-only, `default-features = false`), parsed by `parse_pasted_column`, and
  the values fill the **current column downward** from the cursor. A status line reports
  `N line(s): X parsed as Decimal, Y left empty (blank / non-numeric — never 0)`.
- Parsing is **exact**: `Decimal::from_str_exact` → `contract::Money`. A blank or non-numeric line →
  **empty cell, never `0`** (mirrors the `Cell` "missing ≠ 0" rule). A locale decimal comma (`1,5`) is
  **not** silently accepted — locale-aware entry is deferred to Story 2.4.

## Automated evidence (headless — runs without a display)

The pure paste-parsing + column-fill logic is unit-tested (the genuinely testable, make-or-break core):

```
cargo test -p steadyinvest-app --example spike_a_grid
```

| Test | Asserts | Result |
|------|---------|--------|
| `parses_a_column_keeping_gaps_and_never_zero` | CRLF/blank/garbage column → values + gaps; trailing newline ignored | ✅ |
| `blanks_and_garbage_never_become_a_value` | `""`, spaces, `abc`, `1.2.3`, `--` → no value (never 0) | ✅ |
| `eu_decimal_comma_is_not_silently_accepted` | `1,5` → `None` (locale = Story 2.4) | ✅ |
| `a_ten_value_column_all_parse` | a 10-line numeric column → 10 parsed | ✅ |
| `paste_fills_the_current_column_downward_and_clips_at_grid_height` | fills down the cursor's column; gap stays empty; other columns untouched; clips at grid height | ✅ |

Repo gates green with the example present: `cargo fmt --all --check`, `cargo clippy --all-targets
--all-features --locked -- -D warnings`, `cargo test --all --locked`, `cargo deny check` (arboard's
licenses pass — `clipboard-win` BSL-1.0 was already allow-listed).

> Note: `just test` (`cargo test --all`) compiles examples but does not *run* example tests; run the
> command above to execute the 5 logic tests. Acceptable for a throwaway spike.

## How to verify the interaction (Guy, on a display)

1. `just spike-a` (Linux needs `libfontconfig1-dev`; needs a desktop session with a clipboard).
2. In any app (a spreadsheet, a text editor), **copy a column of ~10 numbers** (one per line).
3. In the spike, click/arrow to a cell, press **Ctrl+V** → the values should fill that column downward;
   the status line reports how many parsed vs were left empty.
4. Try: arrow-key navigation feel; type-to-edit; a column containing a blank line and a non-number
   (those cells must stay **empty**, dimmed — never `0`).

## Results — RUN 2026-06-10 (Guy, on display)

| Metric | Value |
|--------|-------|
| Builds + clippy `-D warnings` clean | ✅ (Linux) |
| Logic unit tests pass (parse + column-fill) | ✅ (5/5) |
| Keyboard cell-cursor nav feels usable | ✅ **Yes** (Guy) |
| Paste-a-column lands the values in the correct cells | ✅ **Yes** (Guy) |
| Blank / non-numeric cells stay empty (never 0) on screen | ✅ **Yes** (Guy) |

Run via `cargo run -p steadyinvest-app --example spike_a_grid` (`just` not installed locally — the
direct cargo command is equivalent). All three on-display checks passed.

## Decision — **GO** (2026-06-10)

- [x] **GO** — a custom Slint grid (Rust model + `for`-grid, `FocusScope` keys, `arboard` paste)
  supports spreadsheet-grade entry. The **entry-regime feasibility is settled**. Epic 2 (Stories
  2.3/2.4) builds the real grid this way: production uses a `TableModel` + virtualized `ListView`,
  locale-aware parsing (the CH/EU decimal comma), and the tri-state review markers.
- [ ] ~~NO-GO fallback~~ — not needed (custom Slint grid + `arboard` clipboard works).

The throwaway example `app/examples/spike_a_grid.rs` has served its purpose (the decision above). It is
kept as a reference for Stories 2.3/2.4 and may be deleted when the production grid lands.
