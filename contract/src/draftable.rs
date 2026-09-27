//! The draftable fields of a study, their units, the one proposed-value parser, and the draft
//! fingerprint (Epic 8, Story 8.2b — arch §Phase 4 A6/A7, owner decision D6).
//!
//! **One place.** Every field an AI draft may target is a [`DraftField`]: the seven study-grid
//! cell fields and the nine judgment fields — **not** `current_price` nor `ttm_eps`, which are
//! provider market facts a refresh writes (D6). Its [`DraftField::key`] is the contract's serde
//! field name, the spelling stored in [`crate::DraftTarget`]. Story 8.3 (MCP submission) and 8.4
//! (tool schema) reuse this enumeration and [`DraftField::parse_value`] verbatim.
//!
//! **Fingerprint (A7).** [`draft_fingerprint`] hashes an explicit, documented line encoding —
//! never the serde form, which changes whenever a type gains a field — so the fingerprint a draft
//! was submitted against can be compared with today's across builds. `None` means the target is
//! gone (its year row no longer exists).

use crate::ai::AiOrigin;
use crate::cell::{Cell, Coverage, Source};
use crate::draft::DraftTarget;
use crate::export::sha256_hex;
use crate::money::Money;
use crate::study::{AiPlaced, ForecastLowOption, Judgment, Study, YearData};
use rust_decimal::Decimal;
use std::fmt;

/// Whether a draftable field is a per-year grid cell or a judgment input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftFieldKind {
    /// A per-year study-grid cell (target carries a fiscal year).
    Cell,
    /// A judgment input of the study (no fiscal year).
    Judgment,
}

/// The unit a proposed value is expressed in (plain decimal text, the stored unit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftUnit {
    /// An absolute amount in the study's native currency (sales, pre-tax profit — **not** the
    /// millions the grid displays; storage is absolute, Issue #117).
    Amount,
    /// A per-share amount in the native currency (EPS, dividend, book value).
    PerShare,
    /// A share price in the native currency.
    Price,
    /// A percent value itself (`12.5` = 12.5 %).
    Percent,
    /// A plain multiple (a P/E ratio).
    Ratio,
    /// One of the [`ForecastLowOption`] names (see [`DraftField::options`]).
    Option,
}

/// One field an AI draft may propose a value for (Dev Notes §2 of Story 8.2b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DraftField {
    /// Cell: aggregate revenue (load-bearing).
    Sales,
    /// Cell: earnings per share (load-bearing).
    Eps,
    /// Cell: the year's high price (load-bearing).
    HighPrice,
    /// Cell: the year's low price (load-bearing).
    LowPrice,
    /// Cell: dividend per share (optional slot).
    DividendPerShare,
    /// Cell: pre-tax profit (optional slot).
    PreTaxProfit,
    /// Cell: book value per share (optional slot).
    BookValuePerShare,
    /// Judgment: estimated high EPS.
    EstimatedHighEps,
    /// Judgment: estimated low EPS.
    EstimatedLowEps,
    /// Judgment: projected sales growth, percent.
    ProjectedSalesGrowthPct,
    /// Judgment: projected EPS growth, percent.
    ProjectedEpsGrowthPct,
    /// Judgment: judged average high P/E.
    JudgedAvgHighPe,
    /// Judgment: judged average low P/E.
    JudgedAvgLowPe,
    /// Judgment: the option (c) recent severe low.
    RecentSevereLow,
    /// Judgment: the present full-year dividend.
    PresentFullYearDividend,
    /// Judgment: which §4 forecast-low option is used.
    ForecastLowOption,
}

/// A parsed proposed value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftValue {
    /// A decimal value in the field's unit.
    Number(Money),
    /// A forecast-low option.
    Option(ForecastLowOption),
}

/// Why a proposed text is not a value for its field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftValueProblem {
    /// The field wants a plain decimal (`12.5`, `-3`) and the text is not one.
    NotANumber,
    /// The field wants one of its option names and the text is none of them.
    NotAnOption,
}

impl fmt::Display for DraftValueProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DraftValueProblem::NotANumber => f.write_str("not a plain decimal number"),
            DraftValueProblem::NotAnOption => f.write_str("not one of the field's options"),
        }
    }
}

impl std::error::Error for DraftValueProblem {}

const OPTIONS: [ForecastLowOption; 4] = [
    ForecastLowOption::AvgLowPeTimesEps,
    ForecastLowOption::AvgLowPriceLast5y,
    ForecastLowOption::RecentSevereLow,
    ForecastLowOption::DividendSupported,
];

/// The snake-case name of a forecast-low option — its serde spelling.
pub fn option_name(o: ForecastLowOption) -> &'static str {
    match o {
        ForecastLowOption::AvgLowPeTimesEps => "avg_low_pe_times_eps",
        ForecastLowOption::AvgLowPriceLast5y => "avg_low_price_last5y",
        ForecastLowOption::RecentSevereLow => "recent_severe_low",
        ForecastLowOption::DividendSupported => "dividend_supported",
    }
}

impl DraftField {
    /// Every draftable field, cells first, in the order of Dev Notes §2.
    pub const ALL: [DraftField; 16] = [
        DraftField::Sales,
        DraftField::Eps,
        DraftField::HighPrice,
        DraftField::LowPrice,
        DraftField::DividendPerShare,
        DraftField::PreTaxProfit,
        DraftField::BookValuePerShare,
        DraftField::EstimatedHighEps,
        DraftField::EstimatedLowEps,
        DraftField::ProjectedSalesGrowthPct,
        DraftField::ProjectedEpsGrowthPct,
        DraftField::JudgedAvgHighPe,
        DraftField::JudgedAvgLowPe,
        DraftField::RecentSevereLow,
        DraftField::PresentFullYearDividend,
        DraftField::ForecastLowOption,
    ];

    /// The stable key — the contract's serde field name, as stored in a [`DraftTarget`].
    pub fn key(self) -> &'static str {
        match self {
            DraftField::Sales => "sales",
            DraftField::Eps => "eps",
            DraftField::HighPrice => "high_price",
            DraftField::LowPrice => "low_price",
            DraftField::DividendPerShare => "dividend_per_share",
            DraftField::PreTaxProfit => "pre_tax_profit",
            DraftField::BookValuePerShare => "book_value_per_share",
            DraftField::EstimatedHighEps => "estimated_high_eps",
            DraftField::EstimatedLowEps => "estimated_low_eps",
            DraftField::ProjectedSalesGrowthPct => "projected_sales_growth_pct",
            DraftField::ProjectedEpsGrowthPct => "projected_eps_growth_pct",
            DraftField::JudgedAvgHighPe => "judged_avg_high_pe",
            DraftField::JudgedAvgLowPe => "judged_avg_low_pe",
            DraftField::RecentSevereLow => "recent_severe_low",
            DraftField::PresentFullYearDividend => "present_full_year_dividend",
            DraftField::ForecastLowOption => "forecast_low_option",
        }
    }

    /// The field for a key, or `None` (unknown, or not draftable — `current_price`, `ttm_eps`).
    pub fn from_key(key: &str) -> Option<DraftField> {
        DraftField::ALL.into_iter().find(|f| f.key() == key)
    }

    /// Cell or judgment.
    pub fn kind(self) -> DraftFieldKind {
        match self {
            DraftField::Sales
            | DraftField::Eps
            | DraftField::HighPrice
            | DraftField::LowPrice
            | DraftField::DividendPerShare
            | DraftField::PreTaxProfit
            | DraftField::BookValuePerShare => DraftFieldKind::Cell,
            _ => DraftFieldKind::Judgment,
        }
    }

    /// The unit a proposed value is expressed in.
    pub fn unit(self) -> DraftUnit {
        match self {
            DraftField::Sales | DraftField::PreTaxProfit => DraftUnit::Amount,
            DraftField::Eps
            | DraftField::DividendPerShare
            | DraftField::BookValuePerShare
            | DraftField::EstimatedHighEps
            | DraftField::EstimatedLowEps
            | DraftField::PresentFullYearDividend => DraftUnit::PerShare,
            DraftField::HighPrice | DraftField::LowPrice | DraftField::RecentSevereLow => {
                DraftUnit::Price
            }
            DraftField::ProjectedSalesGrowthPct | DraftField::ProjectedEpsGrowthPct => {
                DraftUnit::Percent
            }
            DraftField::JudgedAvgHighPe | DraftField::JudgedAvgLowPe => DraftUnit::Ratio,
            DraftField::ForecastLowOption => DraftUnit::Option,
        }
    }

    /// The option names of an [`DraftUnit::Option`] field (empty for every other field).
    pub fn options(self) -> &'static [ForecastLowOption] {
        match self {
            DraftField::ForecastLowOption => &OPTIONS,
            _ => &[],
        }
    }

    /// Parse a proposed text into a value for this field. Numbers: **plain notation only** —
    /// optional leading `-`, digits, optional `.` and digits (no exponent, no `+`, no locale or
    /// thousands separators); `-0` reads `0`. Options: the exact snake name.
    pub fn parse_value(self, text: &str) -> Result<DraftValue, DraftValueProblem> {
        let t = text.trim();
        if self.unit() == DraftUnit::Option {
            return OPTIONS
                .into_iter()
                .find(|o| option_name(*o) == t)
                .map(DraftValue::Option)
                .ok_or(DraftValueProblem::NotAnOption);
        }
        if !is_plain_decimal(t) {
            return Err(DraftValueProblem::NotANumber);
        }
        let d = Decimal::from_str_exact(t).map_err(|_| DraftValueProblem::NotANumber)?;
        let d = if d.is_zero() { Decimal::ZERO } else { d };
        Ok(DraftValue::Number(Money::from(d)))
    }

    /// The field and fiscal year a target names, or `None` when the key is unknown / not draftable
    /// or names a field of the other kind (a judgment key in a cell target, or the reverse).
    pub fn of_target(target: &DraftTarget) -> Option<(DraftField, Option<i32>)> {
        let (key, year, kind) = match target {
            DraftTarget::Cell { fiscal_year, field } => {
                (field.as_str(), Some(*fiscal_year), DraftFieldKind::Cell)
            }
            DraftTarget::Judgment { field } => (field.as_str(), None, DraftFieldKind::Judgment),
        };
        let f = DraftField::from_key(key)?;
        (f.kind() == kind).then_some((f, year))
    }

    /// The cell this (cell) field names in a year row, or `None` — an optional slot never touched,
    /// or a judgment field.
    pub fn cell_in(self, y: &YearData) -> Option<&Cell> {
        match self {
            DraftField::Sales => Some(&y.sales),
            DraftField::Eps => Some(&y.eps),
            DraftField::HighPrice => Some(&y.high_price),
            DraftField::LowPrice => Some(&y.low_price),
            DraftField::DividendPerShare => y.dividend_per_share.as_ref(),
            DraftField::PreTaxProfit => y.pre_tax_profit.as_ref(),
            DraftField::BookValuePerShare => y.book_value_per_share.as_ref(),
            _ => None,
        }
    }

    /// Put `cell` in this (cell) field's slot of a year row. Returns `false` (nothing written) for a
    /// judgment field.
    pub fn put_cell(self, y: &mut YearData, cell: Cell) -> bool {
        match self {
            DraftField::Sales => y.sales = cell,
            DraftField::Eps => y.eps = cell,
            DraftField::HighPrice => y.high_price = cell,
            DraftField::LowPrice => y.low_price = cell,
            DraftField::DividendPerShare => y.dividend_per_share = Some(cell),
            DraftField::PreTaxProfit => y.pre_tax_profit = Some(cell),
            DraftField::BookValuePerShare => y.book_value_per_share = Some(cell),
            _ => return false,
        }
        true
    }

    /// This (judgment) field's current value, or `None` (unset, or a cell field).
    pub fn judgment_value(self, j: &Judgment) -> Option<DraftValue> {
        let n = |m: &Option<Money>| m.map(DraftValue::Number);
        match self {
            DraftField::EstimatedHighEps => n(&j.estimated_high_eps),
            DraftField::EstimatedLowEps => n(&j.estimated_low_eps),
            DraftField::ProjectedSalesGrowthPct => n(&j.projected_sales_growth_pct),
            DraftField::ProjectedEpsGrowthPct => n(&j.projected_eps_growth_pct),
            DraftField::JudgedAvgHighPe => n(&j.judged_avg_high_pe),
            DraftField::JudgedAvgLowPe => n(&j.judged_avg_low_pe),
            DraftField::RecentSevereLow => n(&j.recent_severe_low),
            DraftField::PresentFullYearDividend => n(&j.present_full_year_dividend),
            DraftField::ForecastLowOption => Some(DraftValue::Option(j.forecast_low_option)),
            _ => None,
        }
    }

    /// Write a value into this (judgment) field. Returns `false` (nothing written) for a cell field
    /// or a value of the wrong shape (a number for the option, an option for a number).
    pub fn set_judgment(self, j: &mut Judgment, v: DraftValue) -> bool {
        let slot = match self {
            DraftField::EstimatedHighEps => &mut j.estimated_high_eps,
            DraftField::EstimatedLowEps => &mut j.estimated_low_eps,
            DraftField::ProjectedSalesGrowthPct => &mut j.projected_sales_growth_pct,
            DraftField::ProjectedEpsGrowthPct => &mut j.projected_eps_growth_pct,
            DraftField::JudgedAvgHighPe => &mut j.judged_avg_high_pe,
            DraftField::JudgedAvgLowPe => &mut j.judged_avg_low_pe,
            DraftField::RecentSevereLow => &mut j.recent_severe_low,
            DraftField::PresentFullYearDividend => &mut j.present_full_year_dividend,
            DraftField::ForecastLowOption => {
                return match v {
                    DraftValue::Option(o) => {
                        j.forecast_low_option = o;
                        true
                    }
                    DraftValue::Number(_) => false,
                };
            }
            _ => return false,
        };
        match v {
            DraftValue::Number(m) => {
                *slot = Some(m);
                true
            }
            DraftValue::Option(_) => false,
        }
    }

    /// This (judgment) field's "placed by AI" slot, or `None` for a cell field.
    pub fn ai_slot(self, p: &AiPlaced) -> Option<&Option<AiOrigin>> {
        Some(match self {
            DraftField::EstimatedHighEps => &p.estimated_high_eps,
            DraftField::EstimatedLowEps => &p.estimated_low_eps,
            DraftField::ProjectedSalesGrowthPct => &p.projected_sales_growth_pct,
            DraftField::ProjectedEpsGrowthPct => &p.projected_eps_growth_pct,
            DraftField::JudgedAvgHighPe => &p.judged_avg_high_pe,
            DraftField::JudgedAvgLowPe => &p.judged_avg_low_pe,
            DraftField::RecentSevereLow => &p.recent_severe_low,
            DraftField::PresentFullYearDividend => &p.present_full_year_dividend,
            DraftField::ForecastLowOption => &p.forecast_low_option,
            _ => return None,
        })
    }

    /// Mutable access to this (judgment) field's "placed by AI" slot, or `None` for a cell field.
    pub fn ai_slot_mut(self, p: &mut AiPlaced) -> Option<&mut Option<AiOrigin>> {
        Some(match self {
            DraftField::EstimatedHighEps => &mut p.estimated_high_eps,
            DraftField::EstimatedLowEps => &mut p.estimated_low_eps,
            DraftField::ProjectedSalesGrowthPct => &mut p.projected_sales_growth_pct,
            DraftField::ProjectedEpsGrowthPct => &mut p.projected_eps_growth_pct,
            DraftField::JudgedAvgHighPe => &mut p.judged_avg_high_pe,
            DraftField::JudgedAvgLowPe => &mut p.judged_avg_low_pe,
            DraftField::RecentSevereLow => &mut p.recent_severe_low,
            DraftField::PresentFullYearDividend => &mut p.present_full_year_dividend,
            DraftField::ForecastLowOption => &mut p.forecast_low_option,
            _ => return None,
        })
    }
}

/// Plain decimal notation: `-?\d+(\.\d+)?`.
fn is_plain_decimal(t: &str) -> bool {
    let digits = t.strip_prefix('-').unwrap_or(t);
    let (int, frac) = match digits.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (digits, None),
    };
    let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    all_digits(int) && frac.is_none_or(all_digits)
}

/// The canonical text of a decimal — normalized, so `3` and `3.0` agree (the `Money` scale caveat,
/// `money.rs`).
fn dec(m: Option<Money>) -> String {
    match m {
        Some(m) => m.as_decimal().normalize().to_string(),
        None => "-".to_string(),
    }
}

fn source_text(s: Source) -> &'static str {
    match s {
        Source::Provider => "provider",
        Source::Manual => "manual",
        Source::Derived => "derived",
    }
}

fn coverage_text(c: Coverage) -> &'static str {
    match c {
        Coverage::Present => "present",
        Coverage::ToFill => "to_fill",
        Coverage::NotAvailableAccepted => "not_available_accepted",
    }
}

/// The version prefix of every fingerprint this build writes.
pub const FINGERPRINT_VERSION: &str = "fp1";

/// The fingerprint of a draft's target in `study` (arch A7, encoding `fp1` — Story 8.2b Dev Notes
/// §3): `"fp1:" + sha256_hex(lines)`. A draft is **stale** when this differs from the fingerprint
/// it was submitted against. Returns `None` when the target is **gone** (its year row no longer
/// exists) or names no draftable field.
///
/// Encoding, `\n`-terminated lines, decimals normalized, absent = `-`:
/// - `fp1`
/// - cell target: `cell`, `{fiscal_year}`, `{key}`, then either `slot:absent` (an optional slot
///   never touched) or `v:{value}`, `s:{source}`, `c:{coverage}`, `p:{-|none|<pending value>}` —
///   **never** a timestamp, digest, freshness or review (a value-identical re-stamp changes those);
/// - judgment target: `judgment`, `{key}`, `v:{value}` (an option by its name), `m:{method_version}`,
///   `cp:{current_price}`, `ttm:{ttm_eps}`, then one `y:{year}:{sales}:{eps}:{high}:{low}` line per
///   year sorted by year (values only) — so a refresh of the series, of the price or a method change
///   marks a judgment draft stale.
pub fn draft_fingerprint(
    study: &Study,
    target: &DraftTarget,
    method_version: &str,
) -> Option<String> {
    let (field, year) = DraftField::of_target(target)?;
    let mut lines: Vec<String> = vec![FINGERPRINT_VERSION.to_string()];
    match (field.kind(), year) {
        (DraftFieldKind::Cell, Some(year)) => {
            let row = study.years.iter().find(|y| y.year == year)?;
            lines.push("cell".into());
            lines.push(year.to_string());
            lines.push(field.key().into());
            match field.cell_in(row) {
                None => lines.push("slot:absent".into()),
                Some(c) => {
                    lines.push(format!("v:{}", dec(c.value)));
                    lines.push(format!("s:{}", source_text(c.source)));
                    lines.push(format!("c:{}", coverage_text(c.coverage)));
                    lines.push(match &c.pending {
                        None => "p:-".into(),
                        Some(p) => match p.value {
                            None => "p:none".into(),
                            Some(v) => format!("p:{}", dec(Some(v))),
                        },
                    });
                }
            }
        }
        (DraftFieldKind::Judgment, None) => {
            let j = &study.judgment;
            lines.push("judgment".into());
            lines.push(field.key().into());
            lines.push(match field.judgment_value(j) {
                None => "v:-".into(),
                Some(DraftValue::Number(m)) => format!("v:{}", dec(Some(m))),
                Some(DraftValue::Option(o)) => format!("v:{}", option_name(o)),
            });
            lines.push(format!("m:{method_version}"));
            lines.push(format!("cp:{}", dec(j.current_price)));
            lines.push(format!("ttm:{}", dec(j.ttm_eps)));
            let mut years: Vec<&YearData> = study.years.iter().collect();
            years.sort_by_key(|y| y.year);
            for y in years {
                lines.push(format!(
                    "y:{}:{}:{}:{}:{}",
                    y.year,
                    dec(y.sales.value),
                    dec(y.eps.value),
                    dec(y.high_price.value),
                    dec(y.low_price.value)
                ));
            }
        }
        _ => return None,
    }
    let mut bytes = String::new();
    for l in lines {
        bytes.push_str(&l);
        bytes.push('\n');
    }
    Some(format!(
        "{FINGERPRINT_VERSION}:{}",
        sha256_hex(bytes.as_bytes())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_round_trips_and_market_facts_are_not_draftable() {
        for f in DraftField::ALL {
            assert_eq!(DraftField::from_key(f.key()), Some(f));
        }
        assert_eq!(DraftField::from_key("current_price"), None);
        assert_eq!(DraftField::from_key("ttm_eps"), None);
        let cells = DraftField::ALL
            .iter()
            .filter(|f| f.kind() == DraftFieldKind::Cell)
            .count();
        assert_eq!(cells, 7);
        assert_eq!(DraftField::ALL.len() - cells, 9);
    }

    #[test]
    fn units_follow_the_story_table() {
        assert_eq!(DraftField::Sales.unit(), DraftUnit::Amount);
        assert_eq!(DraftField::PreTaxProfit.unit(), DraftUnit::Amount);
        assert_eq!(DraftField::Eps.unit(), DraftUnit::PerShare);
        assert_eq!(DraftField::HighPrice.unit(), DraftUnit::Price);
        assert_eq!(DraftField::ProjectedEpsGrowthPct.unit(), DraftUnit::Percent);
        assert_eq!(DraftField::JudgedAvgLowPe.unit(), DraftUnit::Ratio);
        assert_eq!(DraftField::RecentSevereLow.unit(), DraftUnit::Price);
        assert_eq!(DraftField::ForecastLowOption.unit(), DraftUnit::Option);
        assert_eq!(DraftField::ForecastLowOption.options().len(), 4);
        assert!(DraftField::Eps.options().is_empty());
    }

    #[test]
    fn plain_decimals_parse_and_everything_else_is_refused() {
        let n = |t: &str| DraftField::Eps.parse_value(t);
        let num = |s: &str| DraftValue::Number(Money::from(Decimal::from_str_exact(s).unwrap()));
        assert_eq!(n("12.5"), Ok(num("12.5")));
        assert_eq!(n(" -3 "), Ok(num("-3")));
        assert_eq!(n("0.001"), Ok(num("0.001")));
        assert_eq!(n("-0"), Ok(num("0")));
        for bad in [
            "1e3", "+2", "1'000", "1,5", "", "abc", ".5", "5.", "1 000", "--1", "1.2.3",
        ] {
            assert_eq!(n(bad), Err(DraftValueProblem::NotANumber), "{bad:?}");
        }
    }

    #[test]
    fn option_names_parse_exactly() {
        let f = DraftField::ForecastLowOption;
        for o in OPTIONS {
            assert_eq!(f.parse_value(option_name(o)), Ok(DraftValue::Option(o)));
            assert_eq!(
                serde_json::to_string(&o).unwrap(),
                format!("\"{}\"", option_name(o))
            );
        }
        assert_eq!(f.parse_value("12"), Err(DraftValueProblem::NotAnOption));
        assert_eq!(
            f.parse_value("Recent_Severe_Low"),
            Err(DraftValueProblem::NotAnOption)
        );
    }

    #[test]
    fn a_target_names_a_field_of_its_own_kind_only() {
        let cell = DraftTarget::Cell {
            fiscal_year: 2024,
            field: "eps".into(),
        };
        assert_eq!(
            DraftField::of_target(&cell),
            Some((DraftField::Eps, Some(2024)))
        );
        let wrong = DraftTarget::Cell {
            fiscal_year: 2024,
            field: "judged_avg_low_pe".into(),
        };
        assert_eq!(DraftField::of_target(&wrong), None);
        let j = DraftTarget::Judgment {
            field: "eps".into(),
        };
        assert_eq!(DraftField::of_target(&j), None);
        let price = DraftTarget::Judgment {
            field: "current_price".into(),
        };
        assert_eq!(DraftField::of_target(&price), None);
    }
}
