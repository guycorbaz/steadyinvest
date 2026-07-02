//! Input/output types of the SSG engine ([`crate::ssg::compute`]) — plain [`Decimal`] structs.
//!
//! Same boundary rule as [`crate::normalize::types`]: `core` does NOT depend on `contract`.
//! [`JudgmentInputs`] field names mirror `contract::Judgment` and cover
//! [`crate::method::LOAD_BEARING_JUDGMENT_INPUTS`] (pinned mechanically by the subset test
//! below); [`ForecastLowOption`] mirrors `contract::ForecastLowOption`'s four variants by NAME
//! (alignment, not import). The contract → core mapping is the caller's job (app, Epic 2).
//!
//! Every derived value is `Option<Decimal>` — `None` = unknown/insufficient, NEVER 0 (spec §9).

use crate::normalize::PlausibilityKey;
use rust_decimal::Decimal;

/// The user's forecast-low choice (spec §4, four options). Core-local mirror of
/// `contract::ForecastLowOption` — variants match by name, glued by convention not dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForecastLowOption {
    /// (a) judged average low P/E × estimated low EPS.
    AvgLowPeTimesEps,
    /// (b) average low price of the last 5 usable years.
    AvgLowPriceLast5y,
    /// (c) a recent severe market low (user-supplied).
    RecentSevereLow,
    /// (d) price the dividend will support: `present_dividend / (avg_high_yield_pct / 100)`.
    DividendSupported,
}

/// The judgment inputs of [`crate::ssg::compute`] (FR6/FR12). All values optional: a missing
/// judgment degrades the dependent outputs to `None`, never to a guessed number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgmentInputs {
    /// Direct estimated high EPS — wins over the growth-derived projection when present.
    pub estimated_high_eps: Option<Decimal>,
    /// Direct estimated low EPS — direct-ONLY in v1 (a low-EPS judgment is not a growth
    /// projection; absent ⇒ unknown).
    pub estimated_low_eps: Option<Decimal>,
    /// Judged future sales growth, percent per year (FR6).
    pub projected_sales_growth_pct: Option<Decimal>,
    /// Judged future EPS growth, percent per year (FR6) — feeds the estimated-high-EPS
    /// derivation when the direct value is absent, and the §5 yearly EPS projection.
    pub projected_eps_growth_pct: Option<Decimal>,
    /// Judged future average high P/E (spec §4).
    pub judged_avg_high_pe: Option<Decimal>,
    /// Judged future average low P/E (spec §4).
    pub judged_avg_low_pe: Option<Decimal>,
    /// Which §4 forecast-low option the user selected.
    pub forecast_low_option: ForecastLowOption,
    /// Option (c) input: a recent severe market low.
    pub recent_severe_low: Option<Decimal>,
    /// The security's current price — the zone/verdict anchor.
    pub current_price: Option<Decimal>,
    /// Present full-year dividend per share (§4 option (d) numerator, §5 present yield).
    pub present_full_year_dividend: Option<Decimal>,
}

impl JudgmentInputs {
    /// All-absent inputs with the default forecast-low option — construction base for callers
    /// and tests (mirrors `RawYear::empty`).
    pub fn empty() -> Self {
        JudgmentInputs {
            estimated_high_eps: None,
            estimated_low_eps: None,
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: None,
            judged_avg_low_pe: None,
            forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
            recent_severe_low: None,
            current_price: None,
            present_full_year_dividend: None,
        }
    }

    /// Names of the judgment fields, in struct order. The subset test below asserts
    /// [`crate::method::LOAD_BEARING_JUDGMENT_INPUTS`] ⊆ this list (same mechanical pattern as
    /// `CanonicalYear::VALUE_FIELD_NAMES`, Story 1.7).
    pub const FIELD_NAMES: [&'static str; 10] = [
        "estimated_high_eps",
        "estimated_low_eps",
        "projected_sales_growth_pct",
        "projected_eps_growth_pct",
        "judged_avg_high_pe",
        "judged_avg_low_pe",
        "forecast_low_option",
        "recent_severe_low",
        "current_price",
        "present_full_year_dividend",
    ];
}

/// Quarterly / TTM market observations — the third engine input (facts, not judgments; the
/// Story-1.7 deferral this story owns). All optional. **v1 assumption (caller contract):**
/// values are supplied in post-split, native-currency terms — quarterly data bypasses
/// [`crate::normalize`] in v1, so the engine cannot re-check either property here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarterlyObservations {
    /// The last 4 quarterly EPS; their sum is the TTM EPS (current P/E denominator, spec §3).
    pub ttm_quarterly_eps: Option<[Decimal; 4]>,
    /// The latest reported quarter's sales.
    pub latest_quarter_sales: Option<Decimal>,
    /// The latest reported quarter's EPS.
    pub latest_quarter_eps: Option<Decimal>,
    /// Sales of the same quarter one year earlier.
    pub year_ago_quarter_sales: Option<Decimal>,
    /// EPS of the same quarter one year earlier.
    pub year_ago_quarter_eps: Option<Decimal>,
}

impl QuarterlyObservations {
    /// All-absent observations — construction base for callers and tests.
    pub fn empty() -> Self {
        QuarterlyObservations {
            ttm_quarterly_eps: None,
            latest_quarter_sales: None,
            latest_quarter_eps: None,
            year_ago_quarter_sales: None,
            year_ago_quarter_eps: None,
        }
    }
}

/// Typed key of a methodology quality flag the engine can raise (FR7). Maps 1:1 onto the
/// pinned [`crate::quality_flags::QUALITY_FLAGS`] catalog (membership asserted by test).
///
/// `high_debt` is in the catalog but **not raisable in v1**: `CanonicalFinancials` carries no
/// debt field, so the engine has no input to compare — documented interpretation, tracked as a
/// GitHub issue (Story 1.8 method interpretations), never a silent omission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityFlagKey {
    /// §2: the pre-tax-profit-on-sales trend is declining.
    PtpTrendDeclining,
    /// §2: the return-on-equity trend is declining.
    RoeTrendDeclining,
    /// §2: the latest ROE is below `roe_low_pct()` (10%).
    RoeLow,
    /// §1: the EPS CAGR is strictly below the sales CAGR.
    EpsLagsSales,
    /// §2: the judged future high P/E exceeds `high_pe_aggressive()` (20).
    ProjectedHighPeAggressive,
    /// §2: the judged future high P/E exceeds `high_pe_implausible()` (25).
    ProjectedHighPeImplausible,
    /// §4: the upside/downside ratio is below `ud_target()` (3.0).
    UdBelowTarget,
    /// §4: the upside/downside ratio exceeds `ud_extreme()` (15.0).
    UdExtreme,
    /// §3: the relative value is at/above `relative_value_ceiling_pct()` (100).
    RelativeValueHigh,
}

impl QualityFlagKey {
    /// Every raisable flag, in pinned-catalog order (the order flags appear in
    /// [`SsgOutputs::quality_flags`]).
    pub const ALL: [QualityFlagKey; 9] = [
        QualityFlagKey::PtpTrendDeclining,
        QualityFlagKey::RoeTrendDeclining,
        QualityFlagKey::RoeLow,
        QualityFlagKey::EpsLagsSales,
        QualityFlagKey::ProjectedHighPeAggressive,
        QualityFlagKey::ProjectedHighPeImplausible,
        QualityFlagKey::UdBelowTarget,
        QualityFlagKey::UdExtreme,
        QualityFlagKey::RelativeValueHigh,
    ];

    /// The pinned catalog string (asserted to be a `QUALITY_FLAGS` member by test).
    pub const fn as_str(self) -> &'static str {
        match self {
            QualityFlagKey::PtpTrendDeclining => "ptp_trend_declining",
            QualityFlagKey::RoeTrendDeclining => "roe_trend_declining",
            QualityFlagKey::RoeLow => "roe_low",
            QualityFlagKey::EpsLagsSales => "eps_lags_sales",
            QualityFlagKey::ProjectedHighPeAggressive => "projected_high_pe_aggressive",
            QualityFlagKey::ProjectedHighPeImplausible => "projected_high_pe_implausible",
            QualityFlagKey::UdBelowTarget => "ud_below_target",
            QualityFlagKey::UdExtreme => "ud_extreme",
            QualityFlagKey::RelativeValueHigh => "relative_value_high",
        }
    }
}

/// One calc-time plausibility finding (FR10) — the engine's own finding shape: unlike the
/// input-shape `normalize::Finding`, the year is optional because `low_price_above_current`
/// (and the current-P/E checks) are study-level, not per-year. Flag only — the value is still
/// reported; plausibility never blocks (spec §3).
///
/// When a module introduces a NEW `context` string, add it to the emitted-strings inventory
/// of `engine_emitted_strings_contain_no_banned_verbs` (the FR13 posture gate in
/// `crate::ssg`'s tests) — the inventory is maintained by hand and will not see it otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcFinding {
    /// One of the three calc-time keys (`out_of_bounds_ratio`, `negative_or_zero_denominator`,
    /// `low_price_above_current`).
    pub key: PlausibilityKey,
    /// The reported year, when the finding is per-year; `None` for study-level findings.
    pub year: Option<i32>,
    /// The ratio or denominator the finding points at (e.g. `"eps"`, `"ptp_pct"`).
    pub context: &'static str,
}

/// 5-year trend of a §2 ratio vs its 5-year average (spec §2, ±`trend_even_band_pp()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trend {
    /// The recent ratio is above the 5-year average, beyond the even band.
    Up,
    /// The recent ratio is within ±`trend_even_band_pp()` of the 5-year average.
    Even,
    /// The recent ratio is below the 5-year average, beyond the even band.
    Down,
}

/// A §4 price zone. These are **labels naming the user-defined price bands** (spec §6 exempts
/// them from the banned-verb posture gate) — they are nouns, not imperatives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// The lower third of the forecast range `[forecast_low, buy_top]`.
    Buy,
    /// The middle third `(buy_top, neutral_top]`.
    Neutral,
    /// The upper third `(neutral_top, forecast_high]`.
    Sell,
}

impl Zone {
    /// The zone label noun (exempt from the banned-verb gate, spec §6).
    pub const fn label(self) -> &'static str {
        match self {
            Zone::Buy => "Buy",
            Zone::Neutral => "Neutral",
            Zone::Sell => "Sell",
        }
    }
}

/// The upside/downside ratio as a typed state (spec §9: never a number when the denominator
/// is non-positive, never a panic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsideDownside {
    /// `(forecast_high − current) / (current − forecast_low)` with a positive denominator.
    Ratio(Decimal),
    /// §9 row: denominator ≤ 0 (`current_price ≤ forecast_low`) — a state, not a number.
    Undefined,
    /// An input is missing or the forecast range is degenerate (`forecast_high ≤ forecast_low`).
    Unknown,
}

/// One verdict-candidate criterion as a tri-valued **fact** (spec §1 verdict + §9):
/// an `unknown` input makes the criterion NOT met while staying queryably distinct from a
/// measured miss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriterionFact {
    /// Measured, and the comparison holds.
    Met,
    /// Measured, and the comparison does not hold.
    Unmet,
    /// The criterion's input is `unknown` — unmet by insufficiency, not by measurement.
    UnmetByInsufficiency,
}

/// Neutral verdict **facts** (FR13 posture): the present-price zone and the four
/// quality-candidate criteria. No recommendation — the integrity-gated `FullVerdict`
/// (validation/freshness) is Story 1.11, not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerdictFacts {
    /// Which zone the current price falls in; `None` when outside `[forecast_low,
    /// forecast_high]` or when the zones are unknown.
    pub present_price_zone: Option<Zone>,
    /// U/D ratio ≥ `ud_target()` (3.0).
    pub ud_at_or_above_target: CriterionFact,
    /// Relative value < `relative_value_ceiling_pct()` (100, strict).
    pub relative_value_below_ceiling: CriterionFact,
    /// Present price inside the Buy zone.
    pub present_price_in_buy_zone: CriterionFact,
    /// Projected appreciation ≥ `verdict_double_appreciation_pct()` (100% ≈ doubling; the `≥`
    /// comparator is a recorded interpretation — "roughly doubling" has no §7 table row).
    pub appreciation_at_or_above_double: CriterionFact,
    /// ALL four criteria above are `Met` — a fact surfaced, never a recommendation.
    pub quality_value_candidate: bool,
}

/// §1 Growth outputs (spec §1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrowthOutputs {
    /// Historical sales CAGR, percent, by the endpoints method over the usable years.
    pub sales_cagr_pct: Option<Decimal>,
    /// Historical EPS CAGR, percent.
    pub eps_cagr_pct: Option<Decimal>,
    /// Recent quarterly sales % change: `(latest − year_ago) / year_ago × 100`.
    pub quarterly_sales_change_pct: Option<Decimal>,
    /// Recent quarterly EPS % change.
    pub quarterly_eps_change_pct: Option<Decimal>,
    /// Direct judgment when present; else `latest_usable_eps × (1 + g_eps)^horizon`.
    pub estimated_high_eps: Option<Decimal>,
    /// Direct judgment only (v1) — never derived.
    pub estimated_low_eps: Option<Decimal>,
}

/// One year's §2 ratios. `None` = unknown (missing input or §9-excluded denominator).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YearRatios {
    /// The reported fiscal year.
    pub year: i32,
    /// `pre_tax_profit / sales × 100`.
    pub ptp_pct: Option<Decimal>,
    /// `eps / book_value_per_share × 100`.
    pub roe_pct: Option<Decimal>,
}

/// §2 Management outputs (spec §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagementOutputs {
    /// Per-year ratios over ALL canonical years (ascending) — usability gates only the
    /// average/trend window, not the per-year reporting.
    pub per_year: Vec<YearRatios>,
    /// Mean over the last-5-usable-years window, excluding years whose ratio is unknown.
    pub avg_ptp_pct: Option<Decimal>,
    /// Mean ROE over the window, excluding unknown years.
    pub avg_roe_pct: Option<Decimal>,
    /// The latest usable year's ratio — the "recent" side of the trend comparison, and the
    /// `roe_low` flag input.
    pub latest_ptp_pct: Option<Decimal>,
    /// The latest usable year's ROE.
    pub latest_roe_pct: Option<Decimal>,
    /// Latest usable year's ratio vs the 5-yr average, ±`trend_even_band_pp()`.
    pub ptp_trend: Option<Trend>,
    /// Latest usable year's ROE vs the 5-yr average, same band.
    pub roe_trend: Option<Trend>,
}

/// One year's §3 valuation figures (inside the last-5-usable-years window).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YearValuation {
    /// The reported fiscal year.
    pub year: i32,
    /// `high_price / eps`.
    pub high_pe: Option<Decimal>,
    /// `low_price / eps`.
    pub low_pe: Option<Decimal>,
    /// `dividend_per_share / eps × 100`.
    pub payout_pct: Option<Decimal>,
    /// `dividend_per_share / low_price × 100`.
    pub high_yield_pct: Option<Decimal>,
}

/// §3 Price–earnings-history outputs (spec §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuationOutputs {
    /// The last 5 usable years (ascending; fewer when fewer are usable).
    pub per_year: Vec<YearValuation>,
    /// Means over the window, excluding `unknown` years (spec §9).
    pub avg_high_pe: Option<Decimal>,
    /// Mean low P/E over the window, excluding unknown years.
    pub avg_low_pe: Option<Decimal>,
    /// `(avg_high_pe + avg_low_pe) / 2`.
    pub avg_pe: Option<Decimal>,
    /// Mean payout percentage over the window, excluding unknown years.
    pub avg_payout_pct: Option<Decimal>,
    /// The §4 option-(d) divisor.
    pub avg_high_yield_pct: Option<Decimal>,
    /// Mean of the window's low prices (§4 option (b)).
    pub avg_low_price: Option<Decimal>,
    /// Σ of the 4 quarterly EPS observations (checked — an overflowing sum is `None`).
    pub ttm_eps: Option<Decimal>,
    /// `current_price / ttm_eps`; TTM EPS ≤ 0 ⇒ `None` (spec §9).
    pub current_pe: Option<Decimal>,
    /// `current_pe / avg_pe × 100`.
    pub relative_value_pct: Option<Decimal>,
}

/// The §4 zone bounds: exact thirds of `forecast_high − forecast_low` with the spec's
/// normative interval comparators — Buy `[forecast_low, buy_top]`, Neutral
/// `(buy_top, neutral_top]`, Sell `(neutral_top, forecast_high]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneBounds {
    /// The bottom of the forecast range (and of the Buy zone).
    pub forecast_low: Decimal,
    /// The top of the Buy zone (`forecast_low` + one third of the range).
    pub buy_top: Decimal,
    /// The top of the Neutral zone (`forecast_low` + two thirds of the range).
    pub neutral_top: Decimal,
    /// The top of the forecast range (and of the Sell zone).
    pub forecast_high: Decimal,
}

/// §4 Risk & reward outputs (spec §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskRewardOutputs {
    /// `judged_avg_high_pe × estimated_high_eps`.
    pub forecast_high: Option<Decimal>,
    /// The user-selected option (a)–(d); §9 guards applied.
    pub forecast_low: Option<Decimal>,
    /// `None` when either forecast is unknown or the range is degenerate
    /// (`forecast_high ≤ forecast_low`) — never inverted bands.
    pub zones: Option<ZoneBounds>,
    /// `None` when the current price is outside `[forecast_low, forecast_high]` or the zones
    /// are unknown.
    pub present_price_zone: Option<Zone>,
    /// The §4 upside/downside ratio as a typed state.
    pub upside_downside: UpsideDownside,
}

/// §5 Five-year-potential outputs (spec §5). All `None` when `current_price` is absent or ≤ 0
/// (recorded interpretation — no §9 row covers it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnOutputs {
    /// `present_full_year_dividend / current_price × 100`.
    pub present_yield_pct: Option<Decimal>,
    /// Mean of the projected yearly EPS over the horizon (exact integer powers).
    pub avg_annual_eps: Option<Decimal>,
    /// `avg_annual_eps × avg_payout_pct / 100`.
    pub avg_annual_dividend: Option<Decimal>,
    /// `avg_annual_dividend / current_price × 100`.
    pub avg_yield_pct: Option<Decimal>,
    /// `(forecast_high − current_price) / current_price × 100`.
    pub projected_appreciation_pct: Option<Decimal>,
    /// Annualised appreciation percent (`((high/current)^(1/horizon) − 1) × 100`, §9-guarded)
    /// **plus** average yield percent (gross dividend, study side).
    pub projected_total_annualized_return_pct: Option<Decimal>,
}

/// The full deterministic SSG output set (FR4): five sections + flags, findings, confidence
/// and neutral verdict facts. Every derived value is `Option<Decimal>` (`None` = unknown,
/// never 0); full `Decimal` precision end to end (rounding is display-only,
/// [`crate::rounding`] — the engine calls none of it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsgOutputs {
    /// §1 growth outputs.
    pub growth: GrowthOutputs,
    /// §2 management outputs.
    pub management: ManagementOutputs,
    /// §3 price–earnings-history outputs.
    pub valuation: ValuationOutputs,
    /// §4 risk & reward outputs.
    pub risk_reward: RiskRewardOutputs,
    /// §5 five-year-potential outputs.
    pub returns: ReturnOutputs,
    /// Raised flags in pinned-catalog order ([`QualityFlagKey::ALL`]); a flag is never raised
    /// on an `unknown` metric (unknown ≠ a comparison result).
    pub quality_flags: Vec<QualityFlagKey>,
    /// Calc-time plausibility findings, in fixed pass order: management (years ascending;
    /// sales denominator, ptp bound, book-value denominator, roe bound), then valuation
    /// (years ascending; eps denominator, high-P/E bound, low-P/E bound, low-price
    /// denominator; then TTM denominator, current-P/E bound), then risk & reward
    /// (`forecast_low` vs current price).
    pub findings: Vec<CalcFinding>,
    /// `usable_years < USABLE_YEARS_FLOOR` (FR8) — computation still ran on available data.
    pub low_confidence: bool,
    /// The neutral verdict facts (FR13).
    pub verdict_facts: VerdictFacts,
}

/// Presence of a judgment field looked up by its catalog name; `None` for an unknown name
/// (same safe-direction contract as `normalize::types::canonical_field_present`). Test glue
/// only: unlike 1.7's lookup it has no production caller yet — the contract → core mapping
/// (Epic 2) is where one would appear.
#[cfg(test)]
fn judgment_field_present(inputs: &JudgmentInputs, name: &str) -> Option<bool> {
    match name {
        "estimated_high_eps" => Some(inputs.estimated_high_eps.is_some()),
        "estimated_low_eps" => Some(inputs.estimated_low_eps.is_some()),
        "projected_sales_growth_pct" => Some(inputs.projected_sales_growth_pct.is_some()),
        "projected_eps_growth_pct" => Some(inputs.projected_eps_growth_pct.is_some()),
        "judged_avg_high_pe" => Some(inputs.judged_avg_high_pe.is_some()),
        "judged_avg_low_pe" => Some(inputs.judged_avg_low_pe.is_some()),
        // The option is always chosen (an enum, not an Option) — always "present".
        "forecast_low_option" => Some(true),
        "recent_severe_low" => Some(inputs.recent_severe_low.is_some()),
        "current_price" => Some(inputs.current_price.is_some()),
        "present_full_year_dividend" => Some(inputs.present_full_year_dividend.is_some()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::method::LOAD_BEARING_JUDGMENT_INPUTS;
    use crate::quality_flags::QUALITY_FLAGS;

    /// Same mechanical glue as 1.7's `load_bearing_year_fields_are_subset_of_year_struct`:
    /// every pinned load-bearing judgment input IS a field of `JudgmentInputs`.
    #[test]
    fn load_bearing_judgment_inputs_are_subset_of_judgment_struct() {
        let probe = JudgmentInputs::empty();
        for name in LOAD_BEARING_JUDGMENT_INPUTS {
            assert!(
                JudgmentInputs::FIELD_NAMES.contains(&name),
                "load-bearing judgment input {name:?} is not a JudgmentInputs field ({:?})",
                JudgmentInputs::FIELD_NAMES
            );
            assert!(
                judgment_field_present(&probe, name).is_some(),
                "load-bearing judgment input {name:?} does not resolve in judgment_field_present"
            );
        }
    }

    /// Keeps `FIELD_NAMES` honest: exhaustive destructuring (no `..`) breaks this test at
    /// compile time when a field is added/removed/renamed.
    #[test]
    fn judgment_field_names_match_struct() {
        let JudgmentInputs {
            estimated_high_eps: _,
            estimated_low_eps: _,
            projected_sales_growth_pct: _,
            projected_eps_growth_pct: _,
            judged_avg_high_pe: _,
            judged_avg_low_pe: _,
            forecast_low_option: _,
            recent_severe_low: _,
            current_price: _,
            present_full_year_dividend: _,
        } = JudgmentInputs::empty();
        assert_eq!(JudgmentInputs::FIELD_NAMES.len(), 10);
    }

    /// No invented keys: every raisable flag maps onto the pinned `QUALITY_FLAGS` catalog.
    /// (`high_debt` is catalog-only — not raisable in v1, no debt input exists; the strict
    /// subset here is the documented gap, tracked by GitHub issue.)
    #[test]
    fn quality_flag_keys_are_pinned_catalog_members() {
        for key in QualityFlagKey::ALL {
            assert!(
                QUALITY_FLAGS.iter().any(|f| f.key == key.as_str()),
                "{key:?} -> {:?} is not in the pinned QUALITY_FLAGS catalog",
                key.as_str()
            );
        }
        // Exactly one catalog entry is unraisable in v1: high_debt.
        let raisable: Vec<&str> = QualityFlagKey::ALL.iter().map(|k| k.as_str()).collect();
        let unraisable: Vec<&str> = QUALITY_FLAGS
            .iter()
            .map(|f| f.key)
            .filter(|k| !raisable.contains(k))
            .collect();
        assert_eq!(
            unraisable,
            vec!["high_debt"],
            "the only catalog flag the engine cannot raise must be high_debt (no debt input in v1)"
        );
    }
}
