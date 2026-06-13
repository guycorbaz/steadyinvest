//! Manual-entry plumbing (Story 2.4): the small, pure pieces that turn the read-only 2.3 form into a
//! spreadsheet-grade ENTRY surface — **field addressing**, **year-grid materialization**, the
//! **cell-cursor move math**, and the **`source × coverage` → display-string** mapping the form
//! adapter and `state.rs` share.
//!
//! **Presentation/addressing only — nothing here calculates** (Cardinal Rule). The one number-shaped
//! thing 2.4 does is *parse* a locale string into a [`Money`] ([`crate::viewmodel::format::parse_amount`]),
//! which is string→`Decimal`, not arithmetic. The manual-mutation itself goes through
//! [`contract::Cell::edited`]; this module only locates which cell and builds the skeleton rows.

use steadyinvest_contract::{
    Cell, Coverage, Freshness, Money, Provenance, Review, Source, Timestamp, YearData,
};

/// Every editable field of a study (the §3 A/B/C/F direct columns + the §2 sales/pretax/book rows),
/// in a stable order. The bulk "unlock all" iterates this list; the cell cursor uses the per-grid
/// sub-lists above. Keep in sync with [`PE_FIELDS`] + [`MGMT_FIELDS`].
pub const ALL_FIELDS: [&str; 7] = [
    FIELD_HIGH,
    FIELD_LOW,
    FIELD_EPS,
    FIELD_DIVIDEND,
    FIELD_SALES,
    FIELD_PRETAX,
    FIELD_BOOK,
];

use crate::viewmodel::format::{parse_amount, NumberFormat};

/// How many most-recent complete fiscal years a freshly-created study materializes for entry. The
/// canonical SSG history window. (Extending the forward projection horizon is Story 2.11; full
/// year-column add/remove beyond this initial window is a documented partial — see the 2.4
/// interpretations issue.)
pub const YEAR_WINDOW: usize = 5;

/// The editable §3 P/E-table columns, in left→right order (A·high, B·low, C·EPS, F·dividend). The
/// computed columns D/E/G/H are caption-only (engine, Story 2.6) and are NOT in this list — the
/// cell cursor skips them.
pub const PE_FIELDS: [&str; 4] = [FIELD_HIGH, FIELD_LOW, FIELD_EPS, FIELD_DIVIDEND];

/// The editable §2 management-grid rows (raw inputs), top→bottom. The displayed PTP/ROE ratios are
/// computed (Story 2.6) and stay caption-only em-dash rows, not entry rows.
pub const MGMT_FIELDS: [&str; 3] = [FIELD_SALES, FIELD_PRETAX, FIELD_BOOK];

// Stable field identifiers — the wire key the Slint callbacks pass back (kebab-free, ASCII).
pub const FIELD_HIGH: &str = "a";
pub const FIELD_LOW: &str = "b";
pub const FIELD_EPS: &str = "c";
pub const FIELD_DIVIDEND: &str = "f";
pub const FIELD_SALES: &str = "sales";
pub const FIELD_PRETAX: &str = "pretax";
pub const FIELD_BOOK: &str = "book";

/// Which grid a field belongs to — the cursor moves *within* one grid (no cross-grid arrow nav).
fn field_group(field: &str) -> Option<&'static [&'static str]> {
    if PE_FIELDS.contains(&field) {
        Some(&PE_FIELDS)
    } else if MGMT_FIELDS.contains(&field) {
        Some(&MGMT_FIELDS)
    } else {
        None
    }
}

/// Read the current cell at `(year, field)` — a clone so the caller can edit a fresh copy
/// (snapshot semantics). `None` for an out-of-range year, an unknown field, or an absent optional
/// cell (dividend / pre-tax / book that has never been touched).
pub fn get_cell(year: &YearData, field: &str) -> Option<Cell> {
    match field {
        FIELD_HIGH => Some(year.high_price.clone()),
        FIELD_LOW => Some(year.low_price.clone()),
        FIELD_EPS => Some(year.eps.clone()),
        FIELD_SALES => Some(year.sales.clone()),
        FIELD_DIVIDEND => year.dividend_per_share.clone(),
        FIELD_PRETAX => year.pre_tax_profit.clone(),
        FIELD_BOOK => year.book_value_per_share.clone(),
        _ => None,
    }
}

/// Write `cell` into the `(year, field)` slot. The optional columns are wrapped in `Some`. Returns
/// `Err(())` for an unknown field (the caller surfaces a neutral notice; it never panics).
pub fn set_cell(year: &mut YearData, field: &str, cell: Cell) -> Result<(), ()> {
    match field {
        FIELD_HIGH => year.high_price = cell,
        FIELD_LOW => year.low_price = cell,
        FIELD_EPS => year.eps = cell,
        FIELD_SALES => year.sales = cell,
        FIELD_DIVIDEND => year.dividend_per_share = Some(cell),
        FIELD_PRETAX => year.pre_tax_profit = Some(cell),
        FIELD_BOOK => year.book_value_per_share = Some(cell),
        _ => return Err(()),
    }
    Ok(())
}

/// A fresh **to-fill** cell (value `None`, [`Coverage::ToFill`]) carrying the given provenance — the
/// skeleton every materialized year starts from, and the base an edit of a never-touched optional
/// column builds on.
pub fn tofill_cell(provenance: Provenance) -> Cell {
    Cell {
        value: None,
        source: provenance.source,
        freshness: Freshness::Current,
        review: Review::None,
        coverage: Coverage::ToFill,
        provenance,
    }
}

/// The 4-digit calendar year of an RFC3339 timestamp (`"2026-06-13T…"` → `Some(2026)`); `None` if
/// the prefix is not a year. v1 treats the fiscal year as the calendar year (no fiscal-offset
/// handling — recorded interpretation).
pub fn created_year(created_at: &Timestamp) -> Option<i32> {
    created_at.0.get(0..4).and_then(|y| y.parse::<i32>().ok())
}

/// The deterministic entry window for a freshly-created study: the [`YEAR_WINDOW`] most-recent
/// **complete** fiscal years preceding the created-at year (created 2026 → 2021..=2025), every cell
/// [`Coverage::ToFill`] / `value: None`. A cell flips to `Present` only when actually entered.
///
/// **Interpretation (filed in the 2.4 issue):** year = the created-at calendar year; the window is
/// the 5 complete years before it; the skeleton is materialized in memory and only persisted on the
/// first edit (never written as an all-empty study the user never touched). Returns an empty vec if
/// the created-at year cannot be read (a degraded but safe render — never a panic).
pub fn materialize_year_window(created_at: &Timestamp, provenance: &Provenance) -> Vec<YearData> {
    let Some(year0) = created_year(created_at) else {
        return Vec::new();
    };
    // Oldest → newest, so the most recent complete year sits at the bottom of the table (SSG order).
    ((year0 - YEAR_WINDOW as i32)..year0)
        .map(|year| YearData {
            year,
            sales: tofill_cell(provenance.clone()),
            eps: tofill_cell(provenance.clone()),
            high_price: tofill_cell(provenance.clone()),
            low_price: tofill_cell(provenance.clone()),
            // Optional columns stay absent until entered — an absent cell renders as a gap too.
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
        })
        .collect()
}

/// The next active cell after a cursor move, clamped to the grid edges (no wrap — the Spike-A
/// saturating model). `up`/`down` step the year; `left`/`right` step the field within its own grid.
/// Returns the unchanged origin for an unknown field/direction or a single-column edge.
pub fn next_cell(year: usize, field: &str, dir: &str, year_count: usize) -> (usize, String) {
    let Some(group) = field_group(field) else {
        return (year, field.to_string());
    };
    let col = group.iter().position(|f| *f == field).unwrap_or(0);
    let last_year = year_count.saturating_sub(1);
    let last_col = group.len().saturating_sub(1);
    // §2 is transposed: its fields are ROWS, the years are COLUMNS — so up/down step the field and
    // left/right step the year. §3 is the natural orientation (years are rows).
    let transposed = MGMT_FIELDS.contains(&field);
    let (mut y, mut c) = (year.min(last_year), col);
    match (dir, transposed) {
        ("up", false) | ("left", true) => y = y.saturating_sub(1),
        ("down", false) | ("right", true) => y = (y + 1).min(last_year),
        ("left", false) | ("up", true) => c = c.saturating_sub(1),
        ("right", false) | ("down", true) => c = (c + 1).min(last_col),
        _ => {}
    }
    (y, group[c].to_string())
}

/// Parse a pasted clipboard column into one optional exact value per line, **locale-aware** (the
/// production, locale-closing evolution of the Spike-A `parse_pasted_column`). Normalises CRLF/CR to
/// LF, drops a trailing newline (spreadsheet copies end with one), and parses each line via
/// [`parse_amount`] under the active preset. A blank or non-numeric line yields `None` — **never
/// coerced to `0`**.
pub fn parse_pasted_column(text: &str, format: NumberFormat) -> Vec<Option<Money>> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let body = normalized.trim_end_matches('\n');
    if body.is_empty() {
        return Vec::new();
    }
    body.split('\n')
        .map(|line| parse_amount(line, format))
        .collect()
}

/// The display string for a cell's `source`, revealed on demand (hover/focus) — only for a
/// **present** cell (a gap reveals nothing). Empty string = nothing to reveal.
pub fn source_label(cell: Option<&Cell>) -> &'static str {
    match cell {
        Some(c) if c.coverage == Coverage::Present => match c.source {
            Source::Manual => "manuel",
            Source::Provider => "fournisseur",
            Source::Derived => "calculé",
        },
        _ => "",
    }
}

/// The coverage state as the stable string the Slint side switches its texture on:
/// `"present"` (a value), `"to-fill"` (a marked gap that shouts), `"not-available"` (a quiet
/// accepted gap). An absent optional cell reads as `"to-fill"`.
pub fn coverage_str(cell: Option<&Cell>) -> &'static str {
    match cell.map(|c| c.coverage) {
        Some(Coverage::Present) => "present",
        Some(Coverage::NotAvailableAccepted) => "not-available",
        Some(Coverage::ToFill) | None => "to-fill",
    }
}

/// The review tag as the stable string the Slint side switches its trust-marker on:
/// `"none"` (no marker), `"to-review"` (the `?` hollow glyph), `"validated"` (the `✓` solid check).
/// The enum crosses the adapter boundary as a string — never `0/1/2` (architecture: tri-state review
/// is an enum). An absent optional cell reads as `"none"` (it carries no sign-off).
pub fn review_str(cell: Option<&Cell>) -> &'static str {
    match cell.map(|c| c.review) {
        Some(Review::ToReview) => "to-review",
        Some(Review::Validated) => "validated",
        Some(Review::None) | None => "none",
    }
}

/// Parse a UI-side review tag string back into the contract enum. The Slint callbacks pass
/// `"none" | "to-review" | "validated"`; anything else is treated as [`Review::None`] (a safe
/// default — the gate never panics on a stray string). The inverse of [`review_str`].
pub fn review_from_str(value: &str) -> Review {
    match value {
        "to-review" => Review::ToReview,
        "validated" => Review::Validated,
        _ => Review::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prov() -> Provenance {
        Provenance {
            source: Source::Manual,
            logical_version: 1,
            timestamp: Timestamp("2026-06-13T00:00:00Z".to_string()),
            hash_of_dependencies: "manual".to_string(),
        }
    }

    fn money(s: &str) -> Money {
        Money::from(rust_decimal::Decimal::from_str_exact(s).unwrap())
    }

    #[test]
    fn window_is_the_five_complete_years_before_created_at_all_to_fill() {
        let years =
            materialize_year_window(&Timestamp("2026-06-13T09:00:00Z".to_string()), &prov());
        let labels: Vec<i32> = years.iter().map(|y| y.year).collect();
        assert_eq!(
            labels,
            vec![2021, 2022, 2023, 2024, 2025],
            "5 complete years, oldest first"
        );
        for y in &years {
            for cell in [&y.sales, &y.eps, &y.high_price, &y.low_price] {
                assert_eq!(cell.value, None, "a materialized cell has no value");
                assert_eq!(
                    cell.coverage,
                    Coverage::ToFill,
                    "a materialized cell is to-fill, never 0"
                );
            }
            assert!(
                y.dividend_per_share.is_none(),
                "optional columns stay absent until entered"
            );
        }
    }

    #[test]
    fn unparseable_created_at_yields_an_empty_window_not_a_panic() {
        assert!(materialize_year_window(&Timestamp("weird".to_string()), &prov()).is_empty());
    }

    #[test]
    fn get_and_set_round_trip_through_every_field() {
        let mut year =
            materialize_year_window(&Timestamp("2026-01-01T00:00:00Z".to_string()), &prov())
                .pop()
                .unwrap();
        for field in PE_FIELDS.iter().chain(MGMT_FIELDS.iter()) {
            let base = get_cell(&year, field).unwrap_or_else(|| tofill_cell(prov()));
            let edited = base.edited(Some(money("42.5")), prov());
            set_cell(&mut year, field, edited).expect("known field");
            let back = get_cell(&year, field).expect("present after set");
            assert_eq!(
                back.value,
                Some(money("42.5")),
                "field {field} did not round-trip"
            );
            assert_eq!(back.coverage, Coverage::Present);
        }
        assert!(set_cell(&mut year, "nope", tofill_cell(prov())).is_err());
        assert!(get_cell(&year, "nope").is_none());
    }

    #[test]
    fn pe_cursor_moves_down_right_and_clamps_at_edges() {
        // §3 (natural orientation): down steps the year, right steps the field.
        assert_eq!(
            next_cell(0, FIELD_HIGH, "down", 5),
            (1, FIELD_HIGH.to_string())
        );
        assert_eq!(
            next_cell(0, FIELD_HIGH, "right", 5),
            (0, FIELD_LOW.to_string())
        );
        assert_eq!(
            next_cell(0, FIELD_HIGH, "up", 5),
            (0, FIELD_HIGH.to_string()),
            "clamps at top"
        );
        assert_eq!(
            next_cell(4, FIELD_DIVIDEND, "down", 5),
            (4, FIELD_DIVIDEND.to_string()),
            "clamps at bottom"
        );
        assert_eq!(
            next_cell(0, FIELD_DIVIDEND, "right", 5),
            (0, FIELD_DIVIDEND.to_string()),
            "clamps at right"
        );
    }

    #[test]
    fn mgmt_cursor_is_transposed_years_are_columns() {
        // §2 (transposed): right steps the YEAR, down steps the indicator FIELD.
        assert_eq!(
            next_cell(0, FIELD_SALES, "right", 5),
            (1, FIELD_SALES.to_string())
        );
        assert_eq!(
            next_cell(0, FIELD_SALES, "down", 5),
            (0, FIELD_PRETAX.to_string())
        );
        assert_eq!(
            next_cell(0, FIELD_SALES, "left", 5),
            (0, FIELD_SALES.to_string()),
            "clamps at left"
        );
        assert_eq!(
            next_cell(0, FIELD_BOOK, "down", 5),
            (0, FIELD_BOOK.to_string()),
            "clamps at last indicator"
        );
    }

    #[test]
    fn source_revealed_only_for_present_cells() {
        let present = tofill_cell(prov()).edited(Some(money("1")), prov());
        assert_eq!(source_label(Some(&present)), "manuel");
        let gap = tofill_cell(prov());
        assert_eq!(source_label(Some(&gap)), "", "a gap reveals no source");
        assert_eq!(source_label(None), "");
    }

    #[test]
    fn pasted_column_is_locale_aware_keeps_gaps_and_never_zero() {
        // CH/EU clipboard: narrow-NBSP groups + decimal commas, a trailing newline, a blank + garbage.
        let pasted = "1\u{202F}234,5\r\n13\n\nfoo\n14,25\n";
        let got = parse_pasted_column(pasted, NumberFormat::Comma);
        assert_eq!(got.len(), 5, "trailing newline must not add a 6th cell");
        assert_eq!(got[0], Some(money("1234.5")));
        assert_eq!(got[1], Some(money("13")));
        assert_eq!(got[2], None, "blank line → gap, never 0");
        assert_eq!(got[3], None, "non-numeric → gap, never 0");
        assert_eq!(got[4], Some(money("14.25")));
        // An empty clipboard yields no cells (not one empty cell).
        assert!(parse_pasted_column("", NumberFormat::Point).is_empty());
    }

    #[test]
    fn review_strings_round_trip_through_every_state() {
        // The enum → string → enum round-trip is total over the three states, and an absent cell is
        // "none" (no sign-off). This is the structural marker mapping AC 6 asks for: each reachable
        // review state maps to a distinct stable string the Slint side draws a distinct glyph from.
        for (review, text) in [
            (Review::None, "none"),
            (Review::ToReview, "to-review"),
            (Review::Validated, "validated"),
        ] {
            let cell = Cell {
                review,
                ..tofill_cell(prov())
            };
            assert_eq!(review_str(Some(&cell)), text, "{review:?} → {text}");
            assert_eq!(review_from_str(text), review, "{text} → {review:?}");
        }
        assert_eq!(
            review_str(None),
            "none",
            "an absent cell carries no sign-off"
        );
        assert_eq!(
            review_from_str("garbage"),
            Review::None,
            "a stray string is a safe None, never a panic"
        );
    }

    #[test]
    fn all_fields_is_the_union_of_the_two_grids() {
        for field in PE_FIELDS.iter().chain(MGMT_FIELDS.iter()) {
            assert!(
                ALL_FIELDS.contains(field),
                "{field} missing from ALL_FIELDS"
            );
        }
        assert_eq!(ALL_FIELDS.len(), PE_FIELDS.len() + MGMT_FIELDS.len());
    }

    #[test]
    fn coverage_strings_distinguish_the_three_states() {
        let present = tofill_cell(prov()).edited(Some(money("1")), prov());
        assert_eq!(coverage_str(Some(&present)), "present");
        assert_eq!(coverage_str(Some(&tofill_cell(prov()))), "to-fill");
        let na = Cell {
            coverage: Coverage::NotAvailableAccepted,
            ..tofill_cell(prov())
        };
        assert_eq!(coverage_str(Some(&na)), "not-available");
        assert_eq!(
            coverage_str(None),
            "to-fill",
            "an absent optional cell is a to-fill gap"
        );
    }
}
