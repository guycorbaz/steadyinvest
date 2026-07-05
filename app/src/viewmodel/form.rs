//! Faithful-form view-model adapter (Story 2.3 read-only display → Story 2.4 editable entry): map a
//! `contract::Study` into the Slint form structs the §1–§5 SSG layout renders. **Presentation only**
//! — nothing here calculates (Cardinal Rule). The number-shaped work 2.4 adds is *parsing* on the way
//! IN ([`crate::viewmodel::format::parse_amount`], in `state.rs`), never arithmetic here.
//!
//! The load-bearing rails:
//! - **money crosses as an already-formatted string**, via the 2.1 [`format_amount`] helper — never an
//!   `f64`/`rust_decimal` (Slint has no `Decimal`);
//! - an **absent value is a gap, NEVER `0`** (the prior project's blank-coercion class of bug): a
//!   present cell with no value and the computed/derived slots render [`EMPTY_SLOT`]; an editable
//!   to-fill cell carries `coverage = "to-fill"` so the screen shouts it with a gap glyph.
//!
//! 2.4 surfaces each editable cell's **`source × freshness × coverage`** state (not just its value) so
//! the screen can render the attention hierarchy with **no colour spent**. The A/B/C/F columns are
//! direct editable inputs; D/E/G/H and every §2/§4/§5 *result* are engine-computed (Story 2.6) and
//! stay caption-only [`EMPTY_SLOT`] slots. The header's `method_version` is `core`'s static
//! method-identity string (a `&str` constant, NOT a computed verdict — no engine call).

use steadyinvest_contract::{Cell, Freshness, Provenance, Source, Study, Timestamp, YearData};
use steadyinvest_core::normalize::PlausibilityKey;
use steadyinvest_core::ssg::SsgOutputs;

use crate::viewmodel::engine::{self, PlausibilityWarnings};
use crate::viewmodel::entry::{
    self, FIELD_DIVIDEND, FIELD_EPS, FIELD_HIGH, FIELD_LOW, MGMT_FIELDS,
};
use crate::viewmodel::format::{format_amount, NumberFormat};
use crate::{FormHeader, GridCellState, MgmtRow, PeRow};

/// The faithful empty/absent slot — an em dash. A computed/derived value not yet produced (D/E/G/H,
/// §4/§5 results) renders as this, **never coerced to `0`**. The *editable* to-fill gap gets a
/// distinct shouting glyph in the screen (driven by `coverage = "to-fill"`), not this quiet em dash.
pub const EMPTY_SLOT: &str = "—";

/// The materialized year window the form renders: the study's own years once any edit has persisted
/// them, otherwise the deterministic to-fill skeleton ([`entry::materialize_year_window`]). The
/// skeleton's cells are display-only here (value `None`), so a sentinel provenance is fine — the
/// authoritative skeleton with a clock-stamped provenance is materialized in `state.rs` on first edit.
fn materialized_years(study: &Study) -> Vec<YearData> {
    if study.years.is_empty() {
        entry::materialize_year_window(&study.created_at, &view_provenance())
    } else {
        study.years.clone()
    }
}

/// The cell's pending provider divergence as a formatted display value (Story 3.4, FR22), revealed
/// on demand beside the source/timestamp. `""` when there is no pending — the common case. The two
/// values are never merged: this is the *provider's* number, shown alongside the live manual value.
fn pending_value(cell: Option<&Cell>, format: NumberFormat) -> String {
    match cell
        .and_then(|c| c.pending.as_ref())
        .and_then(|p| p.value.as_ref())
    {
        Some(money) => format_amount(&money.to_string(), format),
        None => String::new(),
    }
}

/// The cell's freshness timestamp as a display date (the `YYYY-MM-DD` prefix of the RFC3339
/// provenance timestamp), revealed on demand (Story 3.3, FR18). Only a **present** cell carries a
/// meaningful as-of; a gap (or an empty/sentinel timestamp) yields `""` so the screen hides it.
fn provenance_date(cell: Option<&Cell>) -> String {
    match cell {
        Some(c) if c.value.is_some() => {
            let ts = &c.provenance.timestamp.0;
            // RFC3339 "2026-06-27T10:00:00Z" → the calendar date; tolerate a bare/empty string.
            ts.split_once('T').map(|(d, _)| d).unwrap_or(ts).to_string()
        }
        _ => String::new(),
    }
}

/// A display-only sentinel provenance for the view-side skeleton (never persisted from here).
fn view_provenance() -> Provenance {
    Provenance {
        source: Source::Manual,
        logical_version: 0,
        timestamp: Timestamp(String::new()),
        hash_of_dependencies: String::new(),
    }
}

/// Build one editable cell's full presentation state. Money crosses as a locale-formatted string; a
/// gap carries an empty `value` and a `coverage` the screen renders as a glyph/marker — never `0`.
fn editable_cell(
    cell: Option<&Cell>,
    field: &str,
    year_index: usize,
    format: NumberFormat,
    warning: Option<PlausibilityKey>,
) -> GridCellState {
    let value = match cell.and_then(|c| c.value.as_ref()) {
        // `Money: Display` is the canonical decimal string; `format_amount` only re-groups it.
        Some(money) => format_amount(&money.to_string(), format),
        None => String::new(),
    };
    GridCellState {
        value: value.into(),
        coverage: entry::coverage_str(cell).into(),
        stale: cell.is_some_and(|c| c.freshness == Freshness::Stale),
        source: entry::source_label(cell).into(),
        // Story 3.3 (FR18/AC4) — the cell's freshness timestamp, revealed on demand alongside the
        // source (the attention-hierarchy rule: provenance is a murmur, never an always-on column).
        // Only a present cell carries a meaningful "as-of"; a gap reveals nothing.
        timestamp: provenance_date(cell).into(),
        // Story 3.4 (FR22) — a divergent provider value preserved alongside a manual value, revealed
        // on demand. "" when there is no pending divergence (the common case).
        pending: pending_value(cell, format).into(),
        // The tri-state review tag crosses as an enum-derived string (Story 2.5) — "none" draws no
        // marker, "to-review" the ? glyph, "validated" the ✓ check. Never a float / 0/1/2.
        review: entry::review_str(cell).into(),
        editable: true,
        year_index: year_index as i32,
        field: field.into(),
        // Story 2.7 — the plausibility warning channel: a neutral attention glyph + the stable key
        // (its human-readable fact is revealed on focus). Independent of the review/coverage/stale
        // channels (AC3): a cell can be both `✓ validated` AND warned.
        warning: warning.is_some(),
        warning_key: warning.map(|k| k.as_str()).unwrap_or("").into(),
    }
}

/// The non-collapsible header identity block: ticker · native currency · created-at (date) · the
/// method-identity string. All already-display-ready strings; no money, no computation.
pub fn header(study: &Study) -> FormHeader {
    FormHeader {
        ticker: study.security_ticker.clone().into(),
        currency: study.native_currency.clone().into(),
        created_at: created_at_date(&study.created_at).into(),
        // `core::METHOD_VERSION` is a `&'static str` identity constant, NOT a verdict — display.
        method_version: steadyinvest_core::METHOD_VERSION.into(),
    }
}

/// The §3 A–H P/E table rows, one per materialized year (no fixed cap — issue #20 / the 2.3 review
/// LOW: the prior `PE_TABLE_ROWS = 5` dropped years 6+). A/B/C/F are editable input cells carrying
/// their coverage/source/freshness state; **D/E/G/H are the engine's** (Story 2.6) — filled from the
/// snapshot `outputs` (the last-5-usable window), each `None`/out-of-window cell the em-dash, never
/// `0`. When `outputs` is `None` (a study that does not yet compute) every computed cell is the
/// faithful em-dash, exactly as in 2.3.
pub fn pe_rows(
    study: &Study,
    format: NumberFormat,
    outputs: Option<&SsgOutputs>,
    warnings: &PlausibilityWarnings,
) -> Vec<PeRow> {
    materialized_years(study)
        .iter()
        .enumerate()
        .map(|(i, y)| {
            let [d, e, g, h] = match outputs {
                Some(o) => engine::pe_year_cells(o, y.year, format),
                None => [
                    EMPTY_SLOT.to_string(),
                    EMPTY_SLOT.to_string(),
                    EMPTY_SLOT.to_string(),
                    EMPTY_SLOT.to_string(),
                ],
            };
            let warn = |field: &str| warnings.cell_key(i, field);
            PeRow {
                year: y.year.to_string().into(),
                year_index: i as i32,
                a: editable_cell(Some(&y.high_price), FIELD_HIGH, i, format, warn(FIELD_HIGH)), // A · Haut
                b: editable_cell(Some(&y.low_price), FIELD_LOW, i, format, warn(FIELD_LOW)), // B · Bas
                c: editable_cell(Some(&y.eps), FIELD_EPS, i, format, warn(FIELD_EPS)), // C · BPA
                d: d.into(), // D · PER haut = A÷C
                e: e.into(), // E · PER bas = B÷C
                f: editable_cell(
                    y.dividend_per_share.as_ref(),
                    FIELD_DIVIDEND,
                    i,
                    format,
                    warn(FIELD_DIVIDEND),
                ), // F
                g: g.into(), // G · F÷C×100
                h: h.into(), // H · F÷B×100
            }
        })
        .collect()
}

/// The §2 management-grid editable raw-input rows, in the fixed order **sales · pre-tax profit ·
/// book value per share** (the screen pairs each with its own `@tr()` label by index). Each row is
/// one editable cell per materialized year. The displayed PTP/ROE ratios are computed (2.6) and stay
/// caption-only in the screen — not here.
pub fn mgmt_rows(
    study: &Study,
    format: NumberFormat,
    warnings: &PlausibilityWarnings,
) -> Vec<MgmtRow> {
    let years = materialized_years(study);
    MGMT_FIELDS
        .iter()
        .map(|field| {
            let cells: Vec<GridCellState> = years
                .iter()
                .enumerate()
                .map(|(i, y)| {
                    editable_cell(
                        entry::get_cell(y, field).as_ref(),
                        field,
                        i,
                        format,
                        warnings.cell_key(i, field),
                    )
                })
                .collect();
            MgmtRow {
                cells: slint::ModelRc::new(slint::VecModel::from(cells)),
            }
        })
        .collect()
}

/// The materialized year numbers (oldest → newest) — the alignment key the §2 computed-ratio adapter
/// uses so its per-year cells line up with the input columns (Story 2.6).
pub fn materialized_year_numbers(study: &Study) -> Vec<i32> {
    materialized_years(study).iter().map(|y| y.year).collect()
}

/// The §2 grid's year-column header labels (the materialized window's years, oldest → newest).
pub fn year_headers(study: &Study) -> Vec<slint::SharedString> {
    materialized_years(study)
        .iter()
        .map(|y| y.year.to_string().into())
        .collect()
}

/// A neutral RFC3339 timestamp rendered for the header: the date portion only. A non-RFC3339 string
/// passes through unchanged — a display transform, it never repairs a value. (Mirrors
/// `state::created_at_date`; duplicated to keep the adapter self-contained.)
fn created_at_date(ts: &Timestamp) -> String {
    ts.0.split('T').next().unwrap_or(&ts.0).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_contract::{
        Cell, Coverage, Judgment, Money, Review, Source, Timestamp, YearData,
    };
    use uuid::Uuid;

    fn money(s: &str) -> Money {
        Money::from(rust_decimal::Decimal::from_str_exact(s).unwrap())
    }

    fn provenance() -> Provenance {
        Provenance {
            source: Source::Manual,
            logical_version: 1,
            timestamp: Timestamp("2026-06-13T00:00:00Z".to_string()),
            hash_of_dependencies: "manual".to_string(),
        }
    }

    fn cell(value: Option<&str>) -> Cell {
        Cell {
            value: value.map(money),
            source: Source::Manual,
            freshness: Freshness::Current,
            review: Review::None,
            coverage: if value.is_some() {
                Coverage::Present
            } else {
                Coverage::ToFill
            },
            provenance: provenance(),
            pending: None,
        }
    }

    fn empty_judgment() -> Judgment {
        Judgment {
            estimated_high_eps: None,
            estimated_low_eps: None,
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: None,
            judged_avg_low_pe: None,
            forecast_low_option: steadyinvest_contract::ForecastLowOption::AvgLowPeTimesEps,
            recent_severe_low: None,
            current_price: None,
            present_full_year_dividend: None,
            ttm_eps: None,
        }
    }

    fn study(years: Vec<YearData>) -> Study {
        let mut s = Study::new(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            "NESN",
            "CHF",
            empty_judgment(),
            Timestamp("2026-06-13T09:30:00Z".to_string()),
        );
        s.years = years;
        s
    }

    #[test]
    fn header_carries_identity_and_a_method_version_never_empty() {
        let h = header(&study(vec![]));
        assert_eq!(h.ticker, "NESN");
        assert_eq!(h.currency, "CHF");
        assert_eq!(h.created_at, "2026-06-13", "created-at trimmed to the day");
        assert!(!h.method_version.is_empty());
    }

    #[test]
    fn a_fresh_study_materializes_the_window_as_editable_to_fill_rows_never_zero() {
        let rows = pe_rows(
            &study(vec![]),
            NumberFormat::Comma,
            None,
            &PlausibilityWarnings::default(),
        );
        assert_eq!(
            rows.len(),
            entry::YEAR_WINDOW,
            "no fixed 5-cap — render the window (issue #20)"
        );
        for row in &rows {
            for editable in [&row.a, &row.b, &row.c, &row.f] {
                assert_eq!(editable.value, "", "a to-fill cell shows no value, never 0");
                assert_eq!(
                    editable.coverage, "to-fill",
                    "an unfilled editable cell shouts a gap"
                );
                assert!(editable.editable, "A/B/C/F are entry cells");
            }
            for computed in [&row.d, &row.e, &row.g, &row.h] {
                assert_eq!(
                    computed.as_str(),
                    EMPTY_SLOT,
                    "computed columns stay caption-only (2.6)"
                );
            }
        }
        // Year labels are the 10 complete years before the created-at year (2026 → 2016..=2025).
        assert_eq!(rows[0].year, "2016");
        assert_eq!(rows[9].year, "2025");
    }

    #[test]
    fn a_present_cell_crosses_as_a_locale_formatted_string_with_source_on_demand() {
        let year = YearData {
            year: 2025,
            sales: cell(Some("1234567.89")),
            eps: cell(Some("4.2")),
            high_price: cell(Some("120.5")),
            low_price: cell(Some("88")),
            dividend_per_share: Some(cell(Some("2.95"))),
            pre_tax_profit: None,
            book_value_per_share: None,
        };
        let rows = pe_rows(
            &study(vec![year]),
            NumberFormat::Comma,
            None,
            &PlausibilityWarnings::default(),
        );
        let r = &rows[0];
        assert_eq!(
            r.a.value, "120,5",
            "money is locale-formatted (comma preset)"
        );
        assert_eq!(r.a.coverage, "present");
        assert_eq!(
            r.a.source, "manuel",
            "source is revealed on demand for a present cell"
        );
        assert_eq!(r.a.field, "a");
        assert_eq!(r.a.year_index, 0);
        // An absent optional cell is an editable to-fill gap (never 0).
        let r2 = &rows[0];
        let _ = r2;
        assert_eq!(rows[0].c.value, "4,2");
    }

    #[test]
    fn the_review_tag_crosses_the_adapter_as_an_enum_derived_string() {
        let validated = Cell {
            review: Review::Validated,
            ..cell(Some("120.5"))
        };
        let year = YearData {
            year: 2025,
            sales: cell(Some("1")),
            eps: cell(Some("1")),
            high_price: validated,
            low_price: cell(Some("1")),
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
        };
        let rows = pe_rows(
            &study(vec![year]),
            NumberFormat::Comma,
            None,
            &PlausibilityWarnings::default(),
        );
        assert_eq!(rows[0].a.review, "validated", "✓ crosses as a string");
        assert_eq!(rows[0].b.review, "none", "an unreviewed cell is none");
        assert_eq!(
            rows[0].f.review, "none",
            "an absent optional cell carries no sign-off"
        );
    }

    #[test]
    fn an_absent_optional_cell_is_a_to_fill_gap_not_zero() {
        let year = YearData {
            year: 2024,
            sales: cell(Some("1")),
            eps: cell(Some("1")),
            high_price: cell(Some("1")),
            low_price: cell(Some("1")),
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
        };
        let rows = pe_rows(
            &study(vec![year]),
            NumberFormat::Comma,
            None,
            &PlausibilityWarnings::default(),
        );
        assert_eq!(rows[0].f.value, "", "no dividend cell ⇒ empty gap, never 0");
        assert_eq!(rows[0].f.coverage, "to-fill");
        assert_eq!(rows[0].f.source, "", "a gap reveals no source");
    }

    #[test]
    fn mgmt_rows_are_three_editable_input_rows_one_cell_per_year() {
        let rows = mgmt_rows(
            &study(vec![]),
            NumberFormat::Point,
            &PlausibilityWarnings::default(),
        );
        assert_eq!(rows.len(), 3, "sales · pre-tax · book");
        for row in &rows {
            assert_eq!(slint::Model::row_count(&row.cells), entry::YEAR_WINDOW);
        }
        // The first cell of the first row addresses (year 0, "sales").
        let first = slint::Model::row_data(&rows[0].cells, 0).unwrap();
        assert_eq!(first.field, "sales");
        assert_eq!(first.coverage, "to-fill");
        assert!(first.editable);
    }

    #[test]
    fn year_headers_match_the_materialized_window() {
        let headers = year_headers(&study(vec![]));
        let headers: Vec<&str> = headers.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            headers,
            vec!["2016", "2017", "2018", "2019", "2020", "2021", "2022", "2023", "2024", "2025"]
        );
    }

    // ── Story 2.7 — the plausibility warning channel on the cell state ──

    #[test]
    fn a_warned_validated_cell_carries_both_channels_independently() {
        use crate::viewmodel::engine::{PlausibilityWarning, WarningAnchor};
        // AC3: the plausibility channel is independent of the tri-state review channel — a single
        // cell can be both `✓ validated` (soft-locked) AND carry a warning glyph, with no collision.
        let validated = Cell {
            review: Review::Validated,
            ..cell(Some("120.5"))
        };
        let year = YearData {
            year: 2025,
            sales: cell(Some("1")),
            eps: cell(Some("1")),
            high_price: validated,
            low_price: cell(Some("1")),
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
        };
        let warnings = PlausibilityWarnings {
            items: vec![PlausibilityWarning {
                key: PlausibilityKey::SplitSeriesBreak,
                anchor: WarningAnchor::Cell {
                    year_index: 0,
                    field: FIELD_HIGH,
                },
            }],
        };
        let rows = pe_rows(&study(vec![year]), NumberFormat::Comma, None, &warnings);
        assert_eq!(
            rows[0].a.review, "validated",
            "the sign-off channel is intact"
        );
        assert!(
            rows[0].a.warning,
            "the same cell independently carries the warning glyph"
        );
        assert_eq!(rows[0].a.warning_key, "split_series_break");
        // A cell with no finding stays silent on the warning channel (AC6).
        assert!(!rows[0].b.warning);
        assert_eq!(rows[0].b.warning_key, "");
    }
}
