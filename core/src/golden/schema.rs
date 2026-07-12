//! The golden-fixture schema (AC 1/3): serde-strict, golden-local mirror of the engine's
//! input types plus the full expected-output surface.
//!
//! Parse discipline (the fixture-strictness INVERSION of the journal rule):
//! - every struct is `#[serde(deny_unknown_fields)]` — a typo'd field fails the parse;
//! - **no** `#[serde(default)]` anywhere — an omitted field fails the parse;
//! - required-but-nullable fields use `deserialize_with` helpers, which disables serde's
//!   implicit missing-`Option` ⇒ `None` special case (the AC-1 `Option` trap): omission is a
//!   parse error, explicit JSON `null` is the typed "unknown/absent" state;
//! - only the optional per-year tables and `normalize_findings` use plain `Option` semantics
//!   (omitted = not asserted);
//! - decimals are JSON **strings** parsed with `Decimal::from_str_exact` and rejected when
//!   non-canonical (the `contract::Money` exactness discipline, replicated locally so `core`
//!   does not depend on `contract`);
//! - expected flag / finding-key strings are validated against the pinned catalogs at parse
//!   time — an unknown string is a parse error, never a silently never-matching expectation.

use crate::normalize::{RawAmount, RawFinancials, RawYear, SplitEvent};
use crate::quality_flags::{PLAUSIBILITY_RULES, QUALITY_FLAGS};
use crate::ssg::{ForecastLowOption, JudgmentInputs, QuarterlyObservations};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer};

/// The fixture-format version this build understands. A fourth version axis scoped to
/// fixtures (distinct from `schema_version` / `METHOD_VERSION` / the app version) — a fixture
/// declaring any other value fails to parse (recorded interpretation, Story 1.9 issue).
pub const FIXTURE_FORMAT_VERSION: u32 = 1;

// ── Deserialization helpers ──

/// A canonical exact-decimal JSON string: `Decimal::from_str_exact` (errors instead of
/// silently rounding past 28 digits) + round-trip canonicality (rejects `1e5`, `+1`, `-0`,
/// underscores, …) — the same posture as `contract::Money`, without the dependency.
struct CanonicalDecimal(Decimal);

impl<'de> Deserialize<'de> for CanonicalDecimal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let parsed = Decimal::from_str_exact(&s).map_err(serde::de::Error::custom)?;
        if parsed.to_string() != s {
            return Err(serde::de::Error::custom(format!(
                "non-canonical decimal string: {s:?} (expected {:?})",
                parsed.to_string()
            )));
        }
        Ok(CanonicalDecimal(parsed))
    }
}

/// Required non-null decimal field.
fn decimal<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Decimal, D::Error> {
    CanonicalDecimal::deserialize(deserializer).map(|c| c.0)
}

/// Required-but-nullable decimal field: omission is a parse error (`deserialize_with`
/// disables the implicit missing-`Option` default), explicit `null` is `None`.
fn nullable_decimal<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Decimal>, D::Error> {
    Option::<CanonicalDecimal>::deserialize(deserializer).map(|o| o.map(|c| c.0))
}

/// Required-but-nullable TTM array: four canonical decimal strings or `null`.
fn nullable_decimal_array<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<[Decimal; 4]>, D::Error> {
    Option::<[CanonicalDecimal; 4]>::deserialize(deserializer).map(|o| o.map(|a| a.map(|c| c.0)))
}

/// Generic required-but-nullable field for non-decimal payloads (presence enforced, `null`
/// allowed) — same trap-avoidance as [`nullable_decimal`].
fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// Expected quality-flag list, each string validated against the pinned `QUALITY_FLAGS`
/// catalog at parse time.
fn quality_flag_list<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    let flags = Vec::<String>::deserialize(deserializer)?;
    for flag in &flags {
        if !QUALITY_FLAGS.iter().any(|f| f.key == flag) {
            return Err(serde::de::Error::custom(format!(
                "unknown quality flag {flag:?} (not in the pinned QUALITY_FLAGS catalog)"
            )));
        }
    }
    Ok(flags)
}

/// A finding key validated against the pinned `PLAUSIBILITY_RULES` catalog at parse time.
fn plausibility_key<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let key = String::deserialize(deserializer)?;
    if !PLAUSIBILITY_RULES.contains(&key.as_str()) {
        return Err(serde::de::Error::custom(format!(
            "unknown plausibility key {key:?} (not in the pinned PLAUSIBILITY_RULES catalog)"
        )));
    }
    Ok(key)
}

/// The declared fixture-format version — only [`FIXTURE_FORMAT_VERSION`] parses.
fn fixture_format_version<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let version = u32::deserialize(deserializer)?;
    if version != FIXTURE_FORMAT_VERSION {
        return Err(serde::de::Error::custom(format!(
            "unsupported fixture_format_version {version} (this build reads {FIXTURE_FORMAT_VERSION})"
        )));
    }
    Ok(version)
}

// ── Fixture root ──

/// One golden reference study: meta (identity + provenance), input (the raw pipeline input),
/// expected (the full SSG output surface).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenStudy {
    /// Fixture identity, provenance and version pins.
    pub meta: GoldenMeta,
    /// The raw pipeline input to replay.
    pub input: GoldenInput,
    /// The independently hand-computed expected output surface.
    pub expected: ExpectedOutputs,
}

/// Fixture identity, provenance and version pins.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenMeta {
    /// Stable fixture identifier (reported in the [`crate::golden::GoldenReport`]).
    pub id: String,
    /// Human-readable fixture title.
    pub title: String,
    /// What scenario the fixture pins.
    pub description: String,
    /// Free-text derivation record (v1): who/when/how the expected values were computed and
    /// how they were cross-checked. NEVER engine output pasted back (the circularity trap).
    pub provenance: String,
    /// The method version the expected values were derived under; ≠ `METHOD_VERSION` ⇒ the
    /// check fails (a stale golden must be re-validated, never silently replayed).
    pub method_version: String,
    /// The declared fixture-format version — only [`FIXTURE_FORMAT_VERSION`] parses.
    #[serde(deserialize_with = "fixture_format_version")]
    pub fixture_format_version: u32,
}

// ── Input mirror (normalize + engine inputs) ──

/// The raw pipeline input: `normalize`'s input shape plus the engine's judgment inputs and
/// quarterly observations.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenInput {
    /// The study's native currency (mirrors `RawFinancials::native_currency`).
    pub native_currency: String,
    /// The raw reported years.
    pub years: Vec<FixtureYear>,
    /// The declared split events.
    pub splits: Vec<FixtureSplit>,
    /// The engine's judgment inputs.
    pub judgment: FixtureJudgment,
    /// The engine's quarterly observations.
    pub quarterly: FixtureQuarterly,
}

/// Golden-local mirror of `normalize::RawAmount`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureAmount {
    /// The reported exact amount (canonical decimal string).
    #[serde(deserialize_with = "decimal")]
    pub value: Decimal,
    /// The amount's reporting currency.
    pub currency: String,
}

/// Golden-local mirror of `normalize::RawYear` (all 12 fields, every one required in the
/// JSON; `null` = not reported).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureYear {
    /// The reported fiscal year.
    pub year: i32,
    /// Mirrors `RawYear::period_months`.
    #[serde(deserialize_with = "nullable")]
    pub period_months: Option<u32>,
    /// Mirrors `RawYear::fiscal_year_end_month`.
    #[serde(deserialize_with = "nullable")]
    pub fiscal_year_end_month: Option<u32>,
    /// Mirrors `RawYear::sales`.
    #[serde(deserialize_with = "nullable")]
    pub sales: Option<FixtureAmount>,
    /// Mirrors `RawYear::eps`.
    #[serde(deserialize_with = "nullable")]
    pub eps: Option<FixtureAmount>,
    /// Mirrors `RawYear::high_price`.
    #[serde(deserialize_with = "nullable")]
    pub high_price: Option<FixtureAmount>,
    /// Mirrors `RawYear::low_price`.
    #[serde(deserialize_with = "nullable")]
    pub low_price: Option<FixtureAmount>,
    /// Mirrors `RawYear::dividend_per_share`.
    #[serde(deserialize_with = "nullable")]
    pub dividend_per_share: Option<FixtureAmount>,
    /// Mirrors `RawYear::pre_tax_profit`.
    #[serde(deserialize_with = "nullable")]
    pub pre_tax_profit: Option<FixtureAmount>,
    /// Mirrors `RawYear::net_profit`.
    #[serde(deserialize_with = "nullable")]
    pub net_profit: Option<FixtureAmount>,
    /// Mirrors `RawYear::tax_rate`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub tax_rate: Option<Decimal>,
    /// Mirrors `RawYear::book_value_per_share`.
    #[serde(deserialize_with = "nullable")]
    pub book_value_per_share: Option<FixtureAmount>,
}

/// Golden-local mirror of `normalize::SplitEvent`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureSplit {
    /// Mirrors `SplitEvent::effective_year`.
    pub effective_year: i32,
    /// Mirrors `SplitEvent::numerator`.
    pub numerator: u32,
    /// Mirrors `SplitEvent::denominator`.
    pub denominator: u32,
}

/// Golden-local mirror of `ssg::ForecastLowOption` — variants match by NAME, glued by the
/// conversion below (alignment, not import).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureForecastLowOption {
    /// Mirrors `ForecastLowOption::AvgLowPeTimesEps` (option a).
    AvgLowPeTimesEps,
    /// Mirrors `ForecastLowOption::AvgLowPriceLast5y` (option b).
    AvgLowPriceLast5y,
    /// Mirrors `ForecastLowOption::RecentSevereLow` (option c).
    RecentSevereLow,
    /// Mirrors `ForecastLowOption::DividendSupported` (option d).
    DividendSupported,
}

/// Golden-local mirror of `ssg::JudgmentInputs` (issue #14: the fixture schema follows the
/// ENGINE, not the contract).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureJudgment {
    /// Mirrors `JudgmentInputs::estimated_high_eps`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub estimated_high_eps: Option<Decimal>,
    /// Mirrors `JudgmentInputs::estimated_low_eps`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub estimated_low_eps: Option<Decimal>,
    /// Mirrors `JudgmentInputs::projected_sales_growth_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub projected_sales_growth_pct: Option<Decimal>,
    /// Mirrors `JudgmentInputs::projected_eps_growth_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub projected_eps_growth_pct: Option<Decimal>,
    /// Mirrors `JudgmentInputs::judged_avg_high_pe`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub judged_avg_high_pe: Option<Decimal>,
    /// Mirrors `JudgmentInputs::judged_avg_low_pe`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub judged_avg_low_pe: Option<Decimal>,
    /// Mirrors `JudgmentInputs::forecast_low_option`.
    pub forecast_low_option: FixtureForecastLowOption,
    /// Mirrors `JudgmentInputs::recent_severe_low`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub recent_severe_low: Option<Decimal>,
    /// Mirrors `JudgmentInputs::current_price`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub current_price: Option<Decimal>,
    /// Mirrors `JudgmentInputs::present_full_year_dividend`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub present_full_year_dividend: Option<Decimal>,
}

/// Golden-local mirror of `ssg::QuarterlyObservations`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureQuarterly {
    /// Mirrors `QuarterlyObservations::ttm_quarterly_eps`.
    #[serde(deserialize_with = "nullable_decimal_array")]
    pub ttm_quarterly_eps: Option<[Decimal; 4]>,
    /// Mirrors `QuarterlyObservations::latest_quarter_sales`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub latest_quarter_sales: Option<Decimal>,
    /// Mirrors `QuarterlyObservations::latest_quarter_eps`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub latest_quarter_eps: Option<Decimal>,
    /// Mirrors `QuarterlyObservations::year_ago_quarter_sales`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub year_ago_quarter_sales: Option<Decimal>,
    /// Mirrors `QuarterlyObservations::year_ago_quarter_eps`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub year_ago_quarter_eps: Option<Decimal>,
}

// ── Conversions into the engine input types ──

impl From<&FixtureAmount> for RawAmount {
    fn from(a: &FixtureAmount) -> Self {
        RawAmount {
            value: a.value,
            currency: a.currency.clone(),
        }
    }
}

impl From<&FixtureYear> for RawYear {
    fn from(y: &FixtureYear) -> Self {
        let amount = |a: &Option<FixtureAmount>| a.as_ref().map(RawAmount::from);
        RawYear {
            year: y.year,
            period_months: y.period_months,
            fiscal_year_end_month: y.fiscal_year_end_month,
            sales: amount(&y.sales),
            eps: amount(&y.eps),
            high_price: amount(&y.high_price),
            low_price: amount(&y.low_price),
            dividend_per_share: amount(&y.dividend_per_share),
            pre_tax_profit: amount(&y.pre_tax_profit),
            net_profit: amount(&y.net_profit),
            tax_rate: y.tax_rate,
            book_value_per_share: amount(&y.book_value_per_share),
        }
    }
}

impl From<&FixtureSplit> for SplitEvent {
    fn from(s: &FixtureSplit) -> Self {
        SplitEvent {
            effective_year: s.effective_year,
            numerator: s.numerator,
            denominator: s.denominator,
        }
    }
}

impl From<&GoldenInput> for RawFinancials {
    fn from(input: &GoldenInput) -> Self {
        RawFinancials {
            native_currency: input.native_currency.clone(),
            years: input.years.iter().map(RawYear::from).collect(),
            splits: input.splits.iter().map(SplitEvent::from).collect(),
        }
    }
}

impl From<FixtureForecastLowOption> for ForecastLowOption {
    fn from(option: FixtureForecastLowOption) -> Self {
        match option {
            FixtureForecastLowOption::AvgLowPeTimesEps => ForecastLowOption::AvgLowPeTimesEps,
            FixtureForecastLowOption::AvgLowPriceLast5y => ForecastLowOption::AvgLowPriceLast5y,
            FixtureForecastLowOption::RecentSevereLow => ForecastLowOption::RecentSevereLow,
            FixtureForecastLowOption::DividendSupported => ForecastLowOption::DividendSupported,
        }
    }
}

impl From<&FixtureJudgment> for JudgmentInputs {
    fn from(j: &FixtureJudgment) -> Self {
        JudgmentInputs {
            estimated_high_eps: j.estimated_high_eps,
            estimated_low_eps: j.estimated_low_eps,
            projected_sales_growth_pct: j.projected_sales_growth_pct,
            projected_eps_growth_pct: j.projected_eps_growth_pct,
            judged_avg_high_pe: j.judged_avg_high_pe,
            judged_avg_low_pe: j.judged_avg_low_pe,
            forecast_low_option: j.forecast_low_option.into(),
            recent_severe_low: j.recent_severe_low,
            current_price: j.current_price,
            present_full_year_dividend: j.present_full_year_dividend,
        }
    }
}

impl From<&FixtureQuarterly> for QuarterlyObservations {
    fn from(q: &FixtureQuarterly) -> Self {
        QuarterlyObservations {
            ttm_quarterly_eps: q.ttm_quarterly_eps,
            latest_quarter_sales: q.latest_quarter_sales,
            latest_quarter_eps: q.latest_quarter_eps,
            year_ago_quarter_sales: q.year_ago_quarter_sales,
            year_ago_quarter_eps: q.year_ago_quarter_eps,
        }
    }
}

// ── Expected output surface (AC 3) ──

/// The expected block: the full SSG output surface. Required fields must be present
/// (`null` = expected unknown); the per-year tables and `normalize_findings` are optional
/// (omitted = not asserted).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedOutputs {
    /// Expected §1 growth values.
    pub growth: ExpectedGrowth,
    /// Expected §2 management values.
    pub management: ExpectedManagement,
    /// Expected §3 valuation values.
    pub valuation: ExpectedValuation,
    /// Expected §4 risk & reward values.
    pub risk_reward: ExpectedRiskReward,
    /// Expected §5 return values.
    pub returns: ExpectedReturns,
    /// Raised flags as pinned catalog strings, in raise (catalog) order — compared as an
    /// ordered list.
    #[serde(deserialize_with = "quality_flag_list")]
    pub quality_flags: Vec<String>,
    /// Calc-time findings, in the engine's fixed pass order.
    pub findings: Vec<ExpectedCalcFinding>,
    /// The expected FR8 low-confidence state.
    pub low_confidence: bool,
    /// The expected neutral verdict facts.
    pub verdict_facts: ExpectedVerdictFacts,
    /// Optional: the normalize-side findings (omitted = not asserted). Fixture (b) asserts it
    /// to prove the pipeline runs through `normalize`.
    pub normalize_findings: Option<Vec<ExpectedNormalizeFinding>>,
}

/// §1 expected values.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedGrowth {
    /// Expected `growth.sales_cagr_pct` (`null` = expected unknown).
    #[serde(deserialize_with = "nullable_decimal")]
    pub sales_cagr_pct: Option<Decimal>,
    /// Expected `growth.eps_cagr_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub eps_cagr_pct: Option<Decimal>,
    /// Expected `growth.quarterly_sales_change_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub quarterly_sales_change_pct: Option<Decimal>,
    /// Expected `growth.quarterly_eps_change_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub quarterly_eps_change_pct: Option<Decimal>,
    /// Expected `growth.estimated_high_eps`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub estimated_high_eps: Option<Decimal>,
    /// Expected `growth.estimated_low_eps`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub estimated_low_eps: Option<Decimal>,
}

/// One expected §2 per-year row (optional table).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedYearRatios {
    /// The row's fiscal year.
    pub year: i32,
    /// Expected per-year `ptp_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub ptp_pct: Option<Decimal>,
    /// Expected per-year `roe_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub roe_pct: Option<Decimal>,
}

/// §2 expected values.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedManagement {
    /// Expected `management.avg_ptp_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub avg_ptp_pct: Option<Decimal>,
    /// Expected `management.avg_roe_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub avg_roe_pct: Option<Decimal>,
    /// Expected `management.latest_ptp_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub latest_ptp_pct: Option<Decimal>,
    /// Expected `management.latest_roe_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub latest_roe_pct: Option<Decimal>,
    /// Expected `management.ptp_trend`.
    #[serde(deserialize_with = "nullable")]
    pub ptp_trend: Option<ExpectedTrend>,
    /// Expected `management.roe_trend`.
    #[serde(deserialize_with = "nullable")]
    pub roe_trend: Option<ExpectedTrend>,
    /// Optional per-year table (omitted = not asserted).
    pub per_year: Option<Vec<ExpectedYearRatios>>,
}

/// One expected §3 per-year row (optional table).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedYearValuation {
    /// The row's fiscal year.
    pub year: i32,
    /// Expected per-year `high_pe`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub high_pe: Option<Decimal>,
    /// Expected per-year `low_pe`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub low_pe: Option<Decimal>,
    /// Expected per-year `payout_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub payout_pct: Option<Decimal>,
    /// Expected per-year `high_yield_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub high_yield_pct: Option<Decimal>,
}

/// §3 expected values.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedValuation {
    /// Expected `valuation.avg_high_pe`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub avg_high_pe: Option<Decimal>,
    /// Expected `valuation.avg_low_pe`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub avg_low_pe: Option<Decimal>,
    /// Expected `valuation.avg_pe`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub avg_pe: Option<Decimal>,
    /// Expected `valuation.avg_payout_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub avg_payout_pct: Option<Decimal>,
    /// Expected `valuation.avg_high_yield_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub avg_high_yield_pct: Option<Decimal>,
    /// Expected `valuation.avg_low_price`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub avg_low_price: Option<Decimal>,
    /// Expected `valuation.ttm_eps`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub ttm_eps: Option<Decimal>,
    /// Expected `valuation.current_pe`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub current_pe: Option<Decimal>,
    /// Expected `valuation.relative_value_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub relative_value_pct: Option<Decimal>,
    /// Optional per-year table (omitted = not asserted).
    pub per_year: Option<Vec<ExpectedYearValuation>>,
}

/// Expected §4 zone bounds (all four required when the zones are expected known).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedZoneBounds {
    /// Expected `zones.forecast_low`.
    #[serde(deserialize_with = "decimal")]
    pub forecast_low: Decimal,
    /// Expected `zones.buy_top`.
    #[serde(deserialize_with = "decimal")]
    pub buy_top: Decimal,
    /// Expected `zones.neutral_top`.
    #[serde(deserialize_with = "decimal")]
    pub neutral_top: Decimal,
    /// Expected `zones.forecast_high`.
    #[serde(deserialize_with = "decimal")]
    pub forecast_high: Decimal,
}

/// Expected zone label (a noun naming a user-defined price band, spec §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedZone {
    /// Mirrors `Zone::Buy`.
    Buy,
    /// Mirrors `Zone::Neutral`.
    Neutral,
    /// Mirrors `Zone::Sell`.
    Sell,
}

/// Expected U/D state: a measured ratio (tolerance-compared), or one of the two exact
/// non-numeric states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedUpsideDownside {
    /// A measured ratio (tolerance-compared).
    Ratio(#[serde(deserialize_with = "decimal")] Decimal),
    /// Mirrors `UpsideDownside::Undefined` (denominator ≤ 0).
    Undefined,
    /// Mirrors `UpsideDownside::Unknown` (missing input / degenerate range).
    Unknown,
}

/// §4 expected values.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedRiskReward {
    /// Expected `risk_reward.forecast_high`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub forecast_high: Option<Decimal>,
    /// Expected `risk_reward.forecast_low`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub forecast_low: Option<Decimal>,
    /// Expected `risk_reward.zones` (`null` = expected unknown).
    #[serde(deserialize_with = "nullable")]
    pub zones: Option<ExpectedZoneBounds>,
    /// Expected `risk_reward.present_price_zone`.
    #[serde(deserialize_with = "nullable")]
    pub present_price_zone: Option<ExpectedZone>,
    /// Expected `risk_reward.upside_downside`.
    pub upside_downside: ExpectedUpsideDownside,
}

/// §5 expected values.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedReturns {
    /// Expected `returns.present_yield_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub present_yield_pct: Option<Decimal>,
    /// Expected `returns.avg_annual_eps`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub avg_annual_eps: Option<Decimal>,
    /// Expected `returns.avg_annual_dividend`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub avg_annual_dividend: Option<Decimal>,
    /// Expected `returns.avg_yield_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub avg_yield_pct: Option<Decimal>,
    /// Expected `returns.projected_appreciation_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub projected_appreciation_pct: Option<Decimal>,
    /// Expected `returns.projected_total_annualized_return_pct`.
    #[serde(deserialize_with = "nullable_decimal")]
    pub projected_total_annualized_return_pct: Option<Decimal>,
}

/// Expected trend (§2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedTrend {
    /// Mirrors `Trend::Up`.
    Up,
    /// Mirrors `Trend::Even`.
    Even,
    /// Mirrors `Trend::Down`.
    Down,
}

/// Expected tri-valued criterion fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedCriterion {
    /// Mirrors `CriterionFact::Met`.
    Met,
    /// Mirrors `CriterionFact::Unmet`.
    Unmet,
    /// Mirrors `CriterionFact::UnmetByInsufficiency`.
    UnmetByInsufficiency,
}

/// Expected verdict facts.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedVerdictFacts {
    /// Expected `verdict_facts.present_price_zone`.
    #[serde(deserialize_with = "nullable")]
    pub present_price_zone: Option<ExpectedZone>,
    /// Expected `verdict_facts.ud_at_or_above_target`.
    pub ud_at_or_above_target: ExpectedCriterion,
    /// Expected `verdict_facts.relative_value_below_ceiling`.
    pub relative_value_below_ceiling: ExpectedCriterion,
    /// Expected `verdict_facts.present_price_in_buy_zone`.
    pub present_price_in_buy_zone: ExpectedCriterion,
    /// Expected `verdict_facts.appreciation_at_or_above_double`.
    pub appreciation_at_or_above_double: ExpectedCriterion,
    /// Expected `verdict_facts.quality_value_candidate`.
    pub quality_value_candidate: bool,
}

/// One expected calc-time finding (`ssg::CalcFinding`: study-level findings carry no year).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedCalcFinding {
    /// The pinned plausibility key (validated at parse time).
    #[serde(deserialize_with = "plausibility_key")]
    pub key: String,
    /// The finding's year; `null` for study-level findings.
    #[serde(deserialize_with = "nullable")]
    pub year: Option<i32>,
    /// The ratio or denominator the finding points at.
    pub context: String,
}

/// One expected normalize-side finding (`normalize::Finding`: always per-year).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedNormalizeFinding {
    /// The pinned plausibility key (validated at parse time).
    #[serde(deserialize_with = "plausibility_key")]
    pub key: String,
    /// The reported year the finding is attached to.
    pub year: i32,
    /// The field (or period-metadata) name the finding points at.
    pub context: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn growth_json(body: &str) -> Result<ExpectedGrowth, serde_json::Error> {
        serde_json::from_str(body)
    }

    const FULL_GROWTH: &str = r#"{
        "sales_cagr_pct": "10",
        "eps_cagr_pct": null,
        "quarterly_sales_change_pct": "25",
        "quarterly_eps_change_pct": "25",
        "estimated_high_eps": "2.48832",
        "estimated_low_eps": "1.40"
    }"#;

    #[test]
    fn explicit_null_is_expected_unknown() {
        let g = growth_json(FULL_GROWTH).expect("valid expected block");
        assert_eq!(g.sales_cagr_pct, Some(Decimal::new(10, 0)));
        assert_eq!(g.eps_cagr_pct, None, "explicit null = expected unknown");
    }

    #[test]
    fn omitted_required_nullable_field_fails_the_parse() {
        // Drop a required field (`eps_cagr_pct`) — the serde Option trap must NOT default it.
        let r = growth_json(
            r#"{
                "sales_cagr_pct": "10",
                "quarterly_sales_change_pct": "25",
                "quarterly_eps_change_pct": "25",
                "estimated_high_eps": "2.48832",
                "estimated_low_eps": "1.40"
            }"#,
        );
        let err = r.expect_err("omission must be a parse error").to_string();
        assert!(
            err.contains("eps_cagr_pct"),
            "the error must name the omitted field: {err}"
        );
    }

    #[test]
    fn unknown_field_fails_the_parse() {
        let with_typo = FULL_GROWTH.replace("\"sales_cagr_pct\"", "\"sales_cagr_pct_typo\"");
        let err = growth_json(&with_typo)
            .expect_err("deny_unknown_fields must reject the typo")
            .to_string();
        assert!(
            err.contains("sales_cagr_pct_typo") || err.contains("unknown field"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn decimal_strings_are_exact_and_canonical() {
        for (good, value) in [
            ("\"10\"", Decimal::new(10, 0)),
            ("\"0.30\"", Decimal::new(30, 2)),
            ("\"-0.40\"", Decimal::new(-40, 2)),
        ] {
            let d: CanonicalDecimal = serde_json::from_str(good).expect("canonical decimal");
            assert_eq!(d.0, value);
        }
        for bad in [
            "\"1e5\"",
            "\"+1\"",
            "\"-0\"",
            "\"1_000\"",
            "\"1.\"",
            "\".5\"",
            "10",
            // Exceeds 28-digit precision: must error, never silently round.
            "\"0.00000000000000000000000000001\"",
        ] {
            let r: Result<CanonicalDecimal, _> = serde_json::from_str(bad);
            assert!(r.is_err(), "expected rejection of {bad}");
        }
    }

    #[test]
    fn unknown_flag_or_plausibility_strings_fail_the_parse() {
        #[derive(Deserialize)]
        struct FlagsProbe(#[serde(deserialize_with = "quality_flag_list")] Vec<String>);

        let ok: FlagsProbe =
            serde_json::from_str(r#"["ptp_trend_declining", "high_debt"]"#).expect("catalog flags");
        assert_eq!(ok.0.len(), 2);
        let bad: Result<FlagsProbe, _> = serde_json::from_str(r#"["invented_flag"]"#);
        assert!(bad.is_err(), "unknown flag string must not parse");

        let finding: Result<ExpectedCalcFinding, _> =
            serde_json::from_str(r#"{"key": "invented_key", "year": null, "context": "eps"}"#);
        assert!(finding.is_err(), "unknown plausibility key must not parse");

        let finding: ExpectedCalcFinding = serde_json::from_str(
            r#"{"key": "negative_or_zero_denominator", "year": 2023, "context": "eps"}"#,
        )
        .expect("known key parses");
        assert_eq!(finding.year, Some(2023));
    }

    #[test]
    fn fixture_format_version_other_than_one_fails_the_parse() {
        let meta = |v: u32| {
            format!(
                r#"{{
                    "id": "x", "title": "t", "description": "d", "provenance": "p",
                    "method_version": "ssg-1.1.0", "fixture_format_version": {v}
                }}"#
            )
        };
        let ok: Result<GoldenMeta, _> = serde_json::from_str(&meta(1));
        assert!(ok.is_ok(), "version 1 must parse: {ok:?}");
        let bad: Result<GoldenMeta, _> = serde_json::from_str(&meta(2));
        assert!(bad.is_err(), "version 2 must not parse");
    }

    #[test]
    fn forecast_low_option_uses_snake_case_variant_names() {
        for (json, expected) in [
            (
                "\"avg_low_pe_times_eps\"",
                ForecastLowOption::AvgLowPeTimesEps,
            ),
            (
                "\"avg_low_price_last5y\"",
                ForecastLowOption::AvgLowPriceLast5y,
            ),
            ("\"recent_severe_low\"", ForecastLowOption::RecentSevereLow),
            (
                "\"dividend_supported\"",
                ForecastLowOption::DividendSupported,
            ),
        ] {
            let parsed: FixtureForecastLowOption =
                serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
            assert_eq!(ForecastLowOption::from(parsed), expected);
        }
        let bad: Result<FixtureForecastLowOption, _> = serde_json::from_str("\"unknown_option\"");
        assert!(bad.is_err());
    }

    #[test]
    fn upside_downside_states_parse() {
        let ratio: ExpectedUpsideDownside = serde_json::from_str(r#"{"ratio": "3"}"#).unwrap();
        assert_eq!(ratio, ExpectedUpsideDownside::Ratio(Decimal::new(3, 0)));
        let undefined: ExpectedUpsideDownside = serde_json::from_str("\"undefined\"").unwrap();
        assert_eq!(undefined, ExpectedUpsideDownside::Undefined);
        let unknown: ExpectedUpsideDownside = serde_json::from_str("\"unknown\"").unwrap();
        assert_eq!(unknown, ExpectedUpsideDownside::Unknown);
    }
}
