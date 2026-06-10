//! Story 1.4 — Spike A (THROWAWAY). A dense, keyboard-driven Slint grid that proves spreadsheet-grade
//! entry: cell-cursor navigation, type-to-edit, and **paste-a-column** (the make-or-break test). The
//! deliverable is a GO/NO-GO decision (see `docs/spikes/spike-a-dense-grid.md`), NOT production code.
//! The real grid is Epic 2 (Stories 2.3/2.4).
//!
//! Run: `cargo run -p steadyinvest-app --example spike_a_grid` (needs a display) or `just spike-a`.
//! Logic test (no display): `cargo test -p steadyinvest-app --example spike_a_grid`.
//!
//! Grid model = a Rust `VecModel` + a Slint `for` grid (the production approach is a `TableModel` +
//! virtualized `ListView`; this fixed 10×4 spike stands in for it). Pasted values are parsed **exactly**
//! as `Decimal` and wrapped in `contract::Money`; a blank or non-numeric line stays **empty, never 0**.

use rust_decimal::Decimal;
use slint::{Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use steadyinvest_contract::Money;

const ROWS: usize = 10;
const COLS: usize = 4;

slint::slint! {
    struct GridCell { text: string, filled: bool }

    export component SpikeWindow inherits Window {
        title: "steadyinvest — Spike A (dense editable grid)";
        preferred-width: 560px;
        preferred-height: 460px;
        background: #0e0f12;
        forward-focus: scope;

        in property <[GridCell]> cells;       // row-major, length ROWS*COLS
        in property <int> rows-count;
        in property <int> cols;
        in property <int> current;            // active cell index
        in property <string> status-text;

        callback nav(int);                    // 0=up 1=down 2=left 3=right
        callback typed(string);
        callback backspace();
        callback paste();

        init => { scope.focus(); }

        VerticalLayout {
            padding: 12px;
            spacing: 8px;

            Text {
                text: "Spike A — arrows move · type to edit · Ctrl+V pastes a column into the current column.";
                color: #eceef2;
                font-size: 13px;
            }

            scope := FocusScope {
                key-pressed(e) => {
                    if (e.text == Key.UpArrow) { root.nav(0); }
                    else if (e.text == Key.DownArrow) { root.nav(1); }
                    else if (e.text == Key.LeftArrow) { root.nav(2); }
                    else if (e.text == Key.RightArrow || e.text == Key.Tab) { root.nav(3); }
                    else if (e.text == Key.Return) { root.nav(1); }
                    else if (e.text == Key.Backspace) { root.backspace(); }
                    else if (e.modifiers.control && e.text == "v") { root.paste(); }
                    else if (!e.modifiers.control && !e.modifiers.meta) { root.typed(e.text); }
                    accept
                }

                Rectangle {
                    background: #16181d;
                    border-color: #2a2e37;
                    border-width: 1px;

                    VerticalLayout {
                        padding: 1px;
                        alignment: start;

                        for r in root.rows-count : HorizontalLayout {
                            alignment: start;
                            for c in root.cols : Rectangle {
                                property <int> idx: r * root.cols + c;
                                width: 120px;
                                height: 28px;                      // dense grid row height (UX-DR5)
                                background: idx == root.current ? #232a38 : #14161b;
                                border-width: 1px;
                                border-color: idx == root.current ? #6da3ff : #242832;  // active = 1px ink ring
                                Text {
                                    text: root.cells[idx].text;
                                    color: root.cells[idx].filled ? #eceef2 : #565d68;
                                    font-size: 13px;
                                    horizontal-alignment: right;
                                    vertical-alignment: center;
                                    x: parent.width - self.width - 8px;
                                }
                            }
                        }
                    }
                }
            }

            Text {
                text: root.status-text;
                color: #b8bdc7;
                font-size: 12px;
                wrap: word-wrap;
            }
        }
    }
}

/// Parse a pasted clipboard column into one optional exact value per line. Normalises CRLF/CR to LF,
/// drops a trailing newline (spreadsheet copies end with one), trims each line, and parses with
/// `Decimal::from_str_exact` (**exact** — errors instead of silently rounding). A blank or non-numeric
/// line yields `None` — **never coerced to 0** (mirrors the `Cell` "missing ≠ 0" rule). A locale
/// decimal comma (`1,5`) is intentionally NOT accepted here — locale-aware entry is Story 2.4.
fn parse_pasted_column(text: &str) -> Vec<Option<Money>> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let body = normalized.trim_end_matches('\n');
    if body.is_empty() {
        return Vec::new();
    }
    body.split('\n')
        .map(|line| {
            let t = line.trim();
            if t.is_empty() {
                None
            } else {
                Decimal::from_str_exact(t).ok().map(Money::from)
            }
        })
        .collect()
}

/// In-memory grid state (the spike's stand-in for the production `TableModel`).
struct Grid {
    cells: Vec<CellState>,
    current: usize,
}

#[derive(Clone, Default)]
struct CellState {
    text: String,
    filled: bool,
}

impl Grid {
    fn new() -> Self {
        Grid {
            cells: vec![CellState::default(); ROWS * COLS],
            current: 0,
        }
    }

    fn move_cursor(&mut self, dir: i32) {
        let (mut row, mut col) = (self.current / COLS, self.current % COLS);
        match dir {
            0 => row = row.saturating_sub(1),
            1 => row = (row + 1).min(ROWS - 1),
            2 => col = col.saturating_sub(1),
            3 => col = (col + 1).min(COLS - 1),
            _ => {}
        }
        self.current = row * COLS + col;
    }

    /// Append one printable edit char to the active cell.
    fn type_char(&mut self, s: &str) {
        // Only accept a single printable char (digits, separators, sign) — ignore everything else.
        let mut chars = s.chars();
        let (Some(ch), None) = (chars.next(), chars.next()) else {
            return;
        };
        if ch.is_ascii_digit() || matches!(ch, '.' | ',' | '-') {
            let cell = &mut self.cells[self.current];
            cell.text.push(ch);
            cell.filled = !cell.text.is_empty();
        }
    }

    fn backspace(&mut self) {
        let cell = &mut self.cells[self.current];
        cell.text.pop();
        cell.filled = !cell.text.is_empty();
    }

    /// Fill the current column downward from the cursor with a parsed clipboard column.
    /// Returns `(parsed_count, empty_count)`. Blank/non-numeric entries leave the cell empty.
    fn paste_column(&mut self, values: &[Option<Money>]) -> (usize, usize) {
        let (start_row, col) = (self.current / COLS, self.current % COLS);
        let (mut parsed, mut empty) = (0, 0);
        for (i, value) in values.iter().enumerate() {
            let row = start_row + i;
            if row >= ROWS {
                break; // ran past the grid — the spike's fixed height; production grows the model
            }
            let cell = &mut self.cells[row * COLS + col];
            match value {
                Some(m) => {
                    cell.text = m.to_string();
                    cell.filled = true;
                    parsed += 1;
                }
                None => {
                    cell.text.clear();
                    cell.filled = false;
                    empty += 1;
                }
            }
        }
        (parsed, empty)
    }
}

/// Push the whole grid into the Slint model (40 cells — rebuild is trivially cheap).
fn refresh(model: &VecModel<GridCell>, grid: &Grid) {
    for (i, c) in grid.cells.iter().enumerate() {
        model.set_row_data(
            i,
            GridCell {
                text: SharedString::from(c.text.as_str()),
                filled: c.filled,
            },
        );
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = SpikeWindow::new()?;
    let grid = Rc::new(RefCell::new(Grid::new()));

    let initial: Vec<GridCell> = (0..ROWS * COLS)
        .map(|_| GridCell {
            text: SharedString::new(),
            filled: false,
        })
        .collect();
    let model = Rc::new(VecModel::from(initial));
    window.set_cells(ModelRc::from(model.clone()));
    window.set_rows_count(ROWS as i32);
    window.set_cols(COLS as i32);
    window.set_current(0);
    window.set_status_text(
        "Ready. Copy a column of ~10 numbers (one per line) elsewhere, click a cell, press Ctrl+V."
            .into(),
    );

    let weak = window.as_weak();
    let g = grid.clone();
    let m = model.clone();
    window.on_nav(move |dir| {
        let Some(w) = weak.upgrade() else { return };
        g.borrow_mut().move_cursor(dir);
        w.set_current(g.borrow().current as i32);
        refresh(&m, &g.borrow());
    });

    let weak = window.as_weak();
    let g = grid.clone();
    let m = model.clone();
    window.on_typed(move |s| {
        let Some(_w) = weak.upgrade() else { return };
        g.borrow_mut().type_char(&s);
        refresh(&m, &g.borrow());
    });

    let weak = window.as_weak();
    let g = grid.clone();
    let m = model.clone();
    window.on_backspace(move || {
        let Some(_w) = weak.upgrade() else { return };
        g.borrow_mut().backspace();
        refresh(&m, &g.borrow());
    });

    let weak = window.as_weak();
    let g = grid.clone();
    let m = model.clone();
    window.on_paste(move || {
        let Some(w) = weak.upgrade() else { return };
        let status = match arboard::Clipboard::new().and_then(|mut cb| cb.get_text()) {
            Ok(text) => {
                let values = parse_pasted_column(&text);
                let (parsed, empty) = g.borrow_mut().paste_column(&values);
                refresh(&m, &g.borrow());
                let dropped = values.len() - (parsed + empty);
                let dropped_note = if dropped > 0 {
                    format!("; {dropped} line(s) past the grid bottom were dropped")
                } else {
                    String::new()
                };
                format!(
                    "Pasted {} cell(s) into the current column: {parsed} parsed as Decimal, \
                     {empty} left empty (blank / non-numeric — never 0){dropped_note}.",
                    parsed + empty
                )
            }
            Err(e) => {
                format!("Clipboard read failed: {e} (needs a desktop session with a clipboard).")
            }
        };
        eprintln!("[spike-a] {status}");
        w.set_status_text(status.into());
    });

    window.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn money(s: &str) -> Money {
        Money::from(Decimal::from_str_exact(s).unwrap())
    }

    #[test]
    fn parses_a_column_keeping_gaps_and_never_zero() {
        let pasted = "12.5\r\n13\n\nfoo\n14.25\n200\n"; // trailing newline + a blank + garbage
        let got = parse_pasted_column(pasted);
        assert_eq!(got.len(), 6, "trailing newline must not add a 7th cell");
        assert_eq!(got[0], Some(money("12.5")));
        assert_eq!(got[1], Some(money("13")));
        assert_eq!(got[2], None, "blank line → empty cell, NOT 0");
        assert_eq!(got[3], None, "non-numeric → empty cell, NOT 0");
        assert_eq!(got[4], Some(money("14.25")));
        assert_eq!(got[5], Some(money("200")));
    }

    #[test]
    fn blanks_and_garbage_never_become_a_value() {
        for s in ["", "   ", "abc", "1.2.3", "--"] {
            assert_eq!(
                parse_pasted_column(s).into_iter().flatten().count(),
                0,
                "{s:?} must not parse to a value"
            );
        }
    }

    #[test]
    fn eu_decimal_comma_is_not_silently_accepted() {
        // CH/EU spreadsheets emit "1,5"; canonical parse rejects it (locale handling = Story 2.4).
        assert_eq!(parse_pasted_column("1,5"), vec![None]);
    }

    #[test]
    fn a_ten_value_column_all_parse() {
        let col = (1..=10)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let got = parse_pasted_column(&col);
        assert_eq!(got.len(), 10);
        assert!(got.iter().all(Option::is_some));
    }

    #[test]
    fn paste_fills_the_current_column_downward_and_clips_at_grid_height() {
        let mut grid = Grid::new();
        grid.current = COLS + 1; // row 1, col 1
        let twelve = vec![Some(money("1")), None, Some(money("3"))];
        let (parsed, empty) = grid.paste_column(&twelve);
        assert_eq!((parsed, empty), (2, 1));
        assert_eq!(grid.cells[COLS + 1].text, "1"); // row 1, col 1
        assert!(grid.cells[COLS + 1].filled);
        assert_eq!(grid.cells[2 * COLS + 1].text, ""); // row 2, col 1 — gap, empty not 0
        assert!(!grid.cells[2 * COLS + 1].filled);
        assert_eq!(grid.cells[3 * COLS + 1].text, "3"); // row 3, col 1
                                                        // Other columns untouched.
        assert_eq!(grid.cells[COLS].text, "");
    }
}
