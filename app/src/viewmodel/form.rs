//! Faithful-form view-model adapter (Story 2.3): map a `contract::Study` into the Slint form structs
//! the §1–§5 SSG layout renders. **Presentation only** — nothing here calculates (Cardinal Rule).
//!
//! The two load-bearing rails:
//! - **money crosses as an already-formatted string**, via the 2.1 [`format_amount`] helper — never an
//!   `f64`/`rust_decimal` (Slint has no `Decimal`);
//! - an **absent cell is a faithful empty slot ([`EMPTY_SLOT`], an em dash) — NEVER `0`** (the prior
//!   project's blank-coercion class of bug; architecture + 2.2 retro rail).
//!
//! 2.3 reads only `Cell.value`. The A/B/C/F columns are direct stored values; the D/E/G/H columns and
//! every §2/§4/§5 result are **computed by the engine in Story 2.6**, so here they are faithful
//! empty/caption-only slots. The header's `method_version` is `core`'s static method-identity string
//! (a `&str` constant, **not** a computed verdict — no engine call, Cardinal Rule intact).

use steadyinvest_contract::{Cell, Study};

use crate::state::created_at_date;
use crate::viewmodel::format::{format_amount, NumberFormat};
use crate::{FormHeader, PeRow};

/// The faithful empty/absent slot — an em dash. A missing value is a genuine gap, rendered as this,
/// **never coerced to `0`** (architecture: `unknown/insufficient` is first-class). Not a word, so the
/// posture gate need not scan it; kept public for the adapter's tests and the dashboard's static slots.
pub const EMPTY_SLOT: &str = "—";

/// How many year rows the §3 P/E table always renders — the SSG's pre-printed year grid is faithful
/// structure even before data lands (Story 2.4). Present years fill from the top; the rest are
/// faithful empty rows.
const PE_TABLE_ROWS: usize = 5;

/// Format a present cell's value as a locale-aware string, or the empty slot when the cell carries no
/// value (`coverage`/`source`/etc. are surfaced in 2.4/2.5 — 2.3 reads only `value`).
fn cell_value(cell: &Cell, format: NumberFormat) -> String {
    match &cell.value {
        // `Money: Display` is the canonical decimal string (`"141.50"`); `format_amount` only
        // re-groups/re-separates it — no arithmetic, Cardinal-Rule safe.
        Some(money) => format_amount(&money.to_string(), format),
        None => EMPTY_SLOT.to_string(),
    }
}

/// Same, for the optional columns (dividend / pre-tax profit / book value): an absent *cell* is as
/// empty as a present cell with no value.
fn opt_cell_value(cell: &Option<Cell>, format: NumberFormat) -> String {
    match cell {
        Some(c) => cell_value(c, format),
        None => EMPTY_SLOT.to_string(),
    }
}

/// The non-collapsible header identity block: ticker · native currency · created-at (date) · the
/// method-identity string. All already-display-ready strings; no money, no computation.
pub fn header(study: &Study) -> FormHeader {
    FormHeader {
        ticker: study.security_ticker.clone().into(),
        currency: study.native_currency.clone().into(),
        created_at: created_at_date(&study.created_at).into(),
        // `core::METHOD_VERSION` is a `&'static str` identity constant (e.g. `"ssg-…"`), NOT a
        // verdict — reading it is display, not the forbidden engine call.
        method_version: steadyinvest_core::METHOD_VERSION.into(),
    }
}

/// The §3 A–H P/E table rows. Exactly [`PE_TABLE_ROWS`] faithful rows: present years (from the top)
/// render their A/B/C/F stored values; D/E/G/H are caption-only (computed in 2.6) and every absent
/// cell is the empty slot. Never panics on a short/empty `years` vec.
pub fn pe_rows(study: &Study, format: NumberFormat) -> Vec<PeRow> {
    (0..PE_TABLE_ROWS)
        .map(|i| match study.years.get(i) {
            Some(y) => PeRow {
                year: y.year.to_string().into(),
                a: cell_value(&y.high_price, format).into(), // A · Haut (direct)
                b: cell_value(&y.low_price, format).into(),  // B · Bas (direct)
                c: cell_value(&y.eps, format).into(),        // C · BPA (direct)
                d: EMPTY_SLOT.into(),                        // D · PER haut = A÷C (2.6)
                e: EMPTY_SLOT.into(),                        // E · PER bas = B÷C (2.6)
                f: opt_cell_value(&y.dividend_per_share, format).into(), // F · Div./action (direct)
                g: EMPTY_SLOT.into(),                        // G · % distribution = F÷C×100 (2.6)
                h: EMPTY_SLOT.into(),                        // H · % rdt haut = F÷B×100 (2.6)
            },
            None => empty_pe_row(),
        })
        .collect()
}

/// A fully-empty §3 row (all slots em-dash) — the faithful structure for a year with no data yet.
fn empty_pe_row() -> PeRow {
    let slot = || slint::SharedString::from(EMPTY_SLOT);
    PeRow {
        year: slot(),
        a: slot(),
        b: slot(),
        c: slot(),
        d: slot(),
        e: slot(),
        f: slot(),
        g: slot(),
        h: slot(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use steadyinvest_contract::{
        Cell, Coverage, Freshness, Judgment, Money, Provenance, Review, Source, Timestamp, YearData,
    };
    use uuid::Uuid;

    fn money(s: &str) -> Money {
        serde_json::from_str(&format!("\"{s}\"")).unwrap()
    }

    fn provenance() -> Provenance {
        Provenance {
            source: Source::Manual,
            logical_version: 1,
            timestamp: Timestamp("2026-06-13T00:00:00Z".to_string()),
            hash_of_dependencies: "h".to_string(),
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
        assert!(
            !h.method_version.is_empty(),
            "the header shows the method identity string"
        );
    }

    #[test]
    fn empty_study_renders_five_faithful_empty_rows_never_zero() {
        let rows = pe_rows(&study(vec![]), NumberFormat::Comma);
        assert_eq!(rows.len(), PE_TABLE_ROWS);
        for row in &rows {
            for slot in [
                &row.year, &row.a, &row.b, &row.c, &row.d, &row.e, &row.f, &row.g, &row.h,
            ] {
                assert_eq!(slot.as_str(), EMPTY_SLOT, "absent cell is em-dash, never 0");
            }
        }
    }

    #[test]
    fn present_year_formats_direct_columns_and_caption_only_columns_stay_empty() {
        let year = YearData {
            year: 2025,
            sales: cell(Some("1000")),
            eps: cell(Some("4.2")),
            high_price: cell(Some("120.5")),
            low_price: cell(Some("88")),
            dividend_per_share: Some(cell(Some("2.95"))),
            pre_tax_profit: None,
            book_value_per_share: None,
        };
        let rows = pe_rows(&study(vec![year]), NumberFormat::Point);
        let r = &rows[0];
        assert_eq!(r.year, "2025");
        assert_eq!(r.a, "120.5", "A · Haut is the stored high price, formatted");
        assert_eq!(r.b, "88", "B · Bas is the stored low price");
        assert_eq!(r.c, "4.2", "C · BPA is the stored eps");
        assert_eq!(r.f, "2.95", "F · Div./action is the stored dividend");
        for computed in [&r.d, &r.e, &r.g, &r.h] {
            assert_eq!(
                computed.as_str(),
                EMPTY_SLOT,
                "computed columns are caption-only until the engine lands (2.6)"
            );
        }
        // The remaining four rows are faithful empties.
        assert_eq!(rows.len(), PE_TABLE_ROWS);
        assert_eq!(rows[1].a, EMPTY_SLOT);
    }

    #[test]
    fn an_absent_dividend_cell_is_empty_not_zero() {
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
        let rows = pe_rows(&study(vec![year]), NumberFormat::Comma);
        assert_eq!(rows[0].f, EMPTY_SLOT, "no dividend cell ⇒ em-dash, never 0");
    }

    #[test]
    fn money_crosses_as_a_locale_formatted_string() {
        // The adapter is the ONLY place a value becomes a string — and it is locale-aware.
        let year = YearData {
            year: 2025,
            sales: cell(Some("1234567.89")),
            eps: cell(Some("1")),
            high_price: cell(Some("1234567.89")),
            low_price: cell(Some("1")),
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
        };
        let comma = pe_rows(&study(vec![year.clone()]), NumberFormat::Comma);
        assert_eq!(comma[0].a, "1\u{202F}234\u{202F}567,89");
        let point = pe_rows(&study(vec![year]), NumberFormat::Point);
        assert_eq!(point[0].a, "1,234,567.89");
    }
}
