//! Input/output types of [`crate::normalize`] — plain [`Decimal`]-based structs.
//!
//! `core` does NOT depend on `contract`: the raw ↔ `contract::Cell`/`YearData` mapping
//! (provenance stamping, source/freshness/review state) is the caller's job (app in Epic 2,
//! ingestion in Epic 3). What glues the two vocabularies is field-NAME alignment with
//! `contract::YearData` and [`crate::method::LOAD_BEARING_YEAR_FIELDS`] — pinned mechanically
//! by the subset test below (closing the Story-1.2 coherence deferral).

use rust_decimal::Decimal;
use std::fmt;

/// A raw reported amount plus the currency it was reported in. The currency is **checked**
/// against the study's native currency (`currency_mismatch` finding) but **never converted** —
/// FX exists only at the future consolidation layer (FR5/NFR-C4), not in `core`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawAmount {
    pub value: Decimal,
    /// ISO-4217-style reporting currency, compared verbatim to the study's native currency.
    pub currency: String,
}

/// One raw reported year. Every value field is optional: missing data stays missing (`None`) —
/// it is never coerced to 0 anywhere downstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawYear {
    pub year: i32,
    /// Reported period length in months; `Some(m)` with `m != 12` raises
    /// `fiscal_period_misalignment` (the year still passes through).
    pub period_months: Option<u32>,
    /// Fiscal-year-end month (1–12); a shift between consecutive reported years raises
    /// `fiscal_period_misalignment`. Values are compared verbatim, not range-validated: an
    /// out-of-range month is only ever *compared*, so it can at worst raise the same finding.
    pub fiscal_year_end_month: Option<u32>,
    /// Aggregate revenue — share-count independent, never split-adjusted.
    pub sales: Option<RawAmount>,
    pub eps: Option<RawAmount>,
    pub high_price: Option<RawAmount>,
    pub low_price: Option<RawAmount>,
    pub dividend_per_share: Option<RawAmount>,
    /// Directly-reported pre-tax profit (aggregate). When absent, the §2 gross-up derives it
    /// from `net_profit` and `tax_rate`.
    pub pre_tax_profit: Option<RawAmount>,
    /// After-tax net profit (aggregate) — gross-up input only; not part of the canonical output.
    pub net_profit: Option<RawAmount>,
    /// Effective tax rate as a **fraction** in `[0, 1)` (e.g. `0.30` = 30%), matching the spec §2
    /// formula `pre_tax_profit = net_profit / (1 − tax_rate)`. `tax_rate ≥ 1` makes the derived
    /// PTP `unknown` (spec §9), never a computed value. Unitless — carries no currency.
    pub tax_rate: Option<Decimal>,
    pub book_value_per_share: Option<RawAmount>,
}

impl RawYear {
    /// An empty year (all values absent) — construction base for callers and tests.
    pub fn empty(year: i32) -> Self {
        RawYear {
            year,
            period_months: None,
            fiscal_year_end_month: None,
            sales: None,
            eps: None,
            high_price: None,
            low_price: None,
            dividend_per_share: None,
            pre_tax_profit: None,
            net_profit: None,
            tax_rate: None,
            book_value_per_share: None,
        }
    }
}

/// A **declared** stock split, `numerator:denominator` new-for-old shares (a 3:1 split ⇒
/// `numerator = 3, denominator = 1`; a 1:3 reverse split ⇒ `numerator = 1, denominator = 3`).
/// Exact integer ratio — never a pre-divided `Decimal` (1/3 is non-terminating). A zero on
/// either side is a structural input error ([`NormalizeError::InvalidSplitRatio`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitEvent {
    /// First year reported in POST-split shares; all years strictly before it are rebased.
    pub effective_year: i32,
    pub numerator: u32,
    pub denominator: u32,
}

/// The raw input of [`crate::normalize::normalize`] — annual series only in v1 (quarterly/TTM
/// shapes are engine inputs defined by Story 1.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFinancials {
    /// ISO-4217-style native currency of the study; every amount reported in another currency
    /// is flagged (`currency_mismatch`) and passed through unconverted.
    pub native_currency: String,
    /// Reported years, any order (normalization sorts ascending). Duplicate years are a
    /// structural error ([`NormalizeError::DuplicateYear`]).
    pub years: Vec<RawYear>,
    /// Declared split events (order irrelevant — the cumulative factor is a product).
    pub splits: Vec<SplitEvent>,
}

/// Per-year usability (spec §4): a year is usable iff ALL load-bearing fields
/// ([`crate::method::LOAD_BEARING_YEAR_FIELDS`]) are present in the canonical output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YearUsability {
    Usable,
    /// The typed `unknown/insufficient` state: at least one load-bearing field is missing —
    /// the missing fields are NAMED, and the values stay `None` (never coerced to 0).
    Insufficient {
        missing: Vec<&'static str>,
    },
}

/// Typed key of a plausibility finding. Maps 1:1 onto the pinned strings of
/// [`crate::quality_flags::PLAUSIBILITY_RULES`] — no invented keys. The first three are
/// input-shape keys raised by [`crate::normalize`]; the three calc-time keys
/// (`out_of_bounds_ratio`, `negative_or_zero_denominator`, `low_price_above_current`) are
/// raised by the engine ([`crate::ssg`], Story 1.8) via its own finding shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlausibilityKey {
    SplitSeriesBreak,
    CurrencyMismatch,
    FiscalPeriodMisalignment,
    OutOfBoundsRatio,
    NegativeOrZeroDenominator,
    LowPriceAboveCurrent,
}

impl PlausibilityKey {
    /// The pinned catalog string (asserted to be a member of `PLAUSIBILITY_RULES` by test).
    pub const fn as_str(self) -> &'static str {
        match self {
            PlausibilityKey::SplitSeriesBreak => "split_series_break",
            PlausibilityKey::CurrencyMismatch => "currency_mismatch",
            PlausibilityKey::FiscalPeriodMisalignment => "fiscal_period_misalignment",
            PlausibilityKey::OutOfBoundsRatio => "out_of_bounds_ratio",
            PlausibilityKey::NegativeOrZeroDenominator => "negative_or_zero_denominator",
            PlausibilityKey::LowPriceAboveCurrent => "low_price_above_current",
        }
    }
}

/// One plausibility finding — **flag only**: the offending value passes through unchanged
/// (the system never silently rewrites the user's data), and findings never block (spec §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub key: PlausibilityKey,
    /// The reported year the finding is attached to.
    pub year: i32,
    /// The field (or period-metadata) name the finding points at.
    pub context: &'static str,
}

/// One canonical year: declared splits rebased into post-split shares (per-share series only),
/// PTP canonicalized (§2 gross-up when needed), full `Decimal` precision end to end — no
/// rounding (display-only, [`crate::rounding`]). `None` = unknown/insufficient — NEVER 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalYear {
    pub year: i32,
    /// Aggregate revenue — never split-adjusted.
    pub sales: Option<Decimal>,
    pub eps: Option<Decimal>,
    pub high_price: Option<Decimal>,
    pub low_price: Option<Decimal>,
    pub dividend_per_share: Option<Decimal>,
    /// Direct when reported; else the §2 gross-up `net_profit / (1 − tax_rate)`;
    /// `tax_rate ≥ 1` or missing inputs ⇒ `None` (spec §9).
    pub pre_tax_profit: Option<Decimal>,
    pub book_value_per_share: Option<Decimal>,
    pub usability: YearUsability,
}

impl CanonicalYear {
    /// Names of the canonical value fields, in struct order. The subset test below asserts
    /// [`crate::method::LOAD_BEARING_YEAR_FIELDS`] ⊆ this list (the Story-1.2 deferral); the
    /// `value_field_names_match_struct` test keeps the list honest against the struct itself.
    pub const VALUE_FIELD_NAMES: [&'static str; 7] = [
        "sales",
        "eps",
        "high_price",
        "low_price",
        "dividend_per_share",
        "pre_tax_profit",
        "book_value_per_share",
    ];
}

/// The canonical output of [`crate::normalize::normalize`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalFinancials {
    /// Years sorted ascending by `year`, regardless of input order.
    pub years: Vec<CanonicalYear>,
    /// Deterministically ordered findings: currency pass, then fiscal pass, then split-break
    /// pass — each scanning years ascending, fields in catalog order.
    pub findings: Vec<Finding>,
    /// Count of usable years — the FR8 low-confidence **input** (the low-confidence verdict
    /// state itself is Story 1.8).
    pub usable_years: u32,
}

/// Structural input errors — distinct from plausibility findings: these are malformed inputs,
/// not implausible data, and the pinned `PLAUSIBILITY_RULES` catalog must not grow for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizeError {
    /// The same reported year appears more than once.
    DuplicateYear { year: i32 },
    /// A declared split has a zero numerator or denominator.
    InvalidSplitRatio { effective_year: i32 },
}

impl fmt::Display for NormalizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NormalizeError::DuplicateYear { year } => {
                write!(f, "duplicate reported year: {year}")
            }
            NormalizeError::InvalidSplitRatio { effective_year } => {
                write!(
                    f,
                    "declared split effective {effective_year} has a zero numerator or denominator"
                )
            }
        }
    }
}

impl std::error::Error for NormalizeError {}

/// Presence of a canonical value field looked up by its catalog name; `None` for an unknown
/// name. An unknown name counts as *missing* in [`missing_load_bearing`] — the safe direction
/// (a year can only degrade to insufficient, never silently pass) — and the subset test
/// guarantees every pinned load-bearing constant actually resolves.
pub(super) fn canonical_field_present(year: &CanonicalYear, name: &str) -> Option<bool> {
    match name {
        "sales" => Some(year.sales.is_some()),
        "eps" => Some(year.eps.is_some()),
        "high_price" => Some(year.high_price.is_some()),
        "low_price" => Some(year.low_price.is_some()),
        "dividend_per_share" => Some(year.dividend_per_share.is_some()),
        "pre_tax_profit" => Some(year.pre_tax_profit.is_some()),
        "book_value_per_share" => Some(year.book_value_per_share.is_some()),
        _ => None,
    }
}

/// Load-bearing fields missing from the canonical `year`, in catalog order — driven by the
/// pinned [`crate::method::LOAD_BEARING_YEAR_FIELDS`] constant, never a re-literal'd list.
pub(super) fn missing_load_bearing(year: &CanonicalYear) -> Vec<&'static str> {
    crate::method::LOAD_BEARING_YEAR_FIELDS
        .iter()
        .copied()
        .filter(|name| !canonical_field_present(year, name).unwrap_or(false))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::method::LOAD_BEARING_YEAR_FIELDS;
    use crate::quality_flags::PLAUSIBILITY_RULES;

    /// Closes the Story-1.2 deferral (`core::method` test `load_bearing_lists_are_coherent`):
    /// every pinned load-bearing year field IS a field of the year struct — both resolvable by
    /// the presence lookup and present in the canonical field-name list.
    #[test]
    fn load_bearing_year_fields_are_subset_of_year_struct() {
        let probe = CanonicalYear {
            year: 2026,
            sales: None,
            eps: None,
            high_price: None,
            low_price: None,
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
            usability: YearUsability::Usable,
        };
        for name in LOAD_BEARING_YEAR_FIELDS {
            assert!(
                CanonicalYear::VALUE_FIELD_NAMES.contains(&name),
                "load-bearing field {name:?} is not a CanonicalYear value field \
                 ({:?})",
                CanonicalYear::VALUE_FIELD_NAMES
            );
            assert!(
                canonical_field_present(&probe, name).is_some(),
                "load-bearing field {name:?} does not resolve in canonical_field_present"
            );
        }
    }

    /// Keeps `VALUE_FIELD_NAMES` honest: exhaustive destructuring (no `..`) breaks this test at
    /// compile time when a field is added/removed/renamed, forcing the name list to be revisited.
    #[test]
    fn value_field_names_match_struct() {
        let y = CanonicalYear {
            year: 2026,
            sales: None,
            eps: None,
            high_price: None,
            low_price: None,
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
            usability: YearUsability::Usable,
        };
        let CanonicalYear {
            year: _,
            sales: _,
            eps: _,
            high_price: _,
            low_price: _,
            dividend_per_share: _,
            pre_tax_profit: _,
            book_value_per_share: _,
            usability: _,
        } = y;
        // 9 struct fields = 7 value fields + `year` + `usability`.
        assert_eq!(CanonicalYear::VALUE_FIELD_NAMES.len(), 7);
    }

    /// No invented keys: every typed plausibility key maps onto the pinned catalog.
    #[test]
    fn plausibility_keys_are_pinned_catalog_members() {
        let keys = [
            PlausibilityKey::SplitSeriesBreak,
            PlausibilityKey::CurrencyMismatch,
            PlausibilityKey::FiscalPeriodMisalignment,
            PlausibilityKey::OutOfBoundsRatio,
            PlausibilityKey::NegativeOrZeroDenominator,
            PlausibilityKey::LowPriceAboveCurrent,
        ];
        assert_eq!(
            keys.len(),
            PLAUSIBILITY_RULES.len(),
            "every pinned plausibility rule must have a typed key"
        );
        for key in keys {
            assert!(
                PLAUSIBILITY_RULES.contains(&key.as_str()),
                "{:?} -> {:?} is not in the pinned PLAUSIBILITY_RULES catalog",
                key,
                key.as_str()
            );
        }
    }

    #[test]
    fn missing_load_bearing_names_exactly_the_absent_fields() {
        let mut y = CanonicalYear {
            year: 2026,
            sales: Some(Decimal::ONE),
            eps: None,
            high_price: Some(Decimal::ONE),
            low_price: None,
            dividend_per_share: None, // not load-bearing — must not be named
            pre_tax_profit: None,
            book_value_per_share: None,
            usability: YearUsability::Usable,
        };
        assert_eq!(missing_load_bearing(&y), vec!["eps", "low_price"]);
        y.eps = Some(Decimal::ONE);
        y.low_price = Some(Decimal::ONE);
        assert!(missing_load_bearing(&y).is_empty());
    }

    #[test]
    fn normalize_error_displays_are_informative() {
        assert_eq!(
            NormalizeError::DuplicateYear { year: 2020 }.to_string(),
            "duplicate reported year: 2020"
        );
        assert!(NormalizeError::InvalidSplitRatio {
            effective_year: 2021
        }
        .to_string()
        .contains("2021"));
    }
}
