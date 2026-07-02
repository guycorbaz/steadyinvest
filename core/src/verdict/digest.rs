//! The ADD9 **content address**: a deterministic SHA-256 hex digest over a documented, stable
//! encoding of the three engine inputs. The encoding is normative and FROZEN — every stored
//! `inputs_hash` depends on it byte-for-byte — so changes here are method-visible: a changed
//! line, order or sentinel silently orphans every prior verdict. Values only: the gates
//! (review/freshness) are deliberately NOT hashed (see the module docs of [`crate::verdict`]).

use crate::normalize::{CanonicalFinancials, CanonicalYear};
use crate::ssg::{ForecastLowOption, JudgmentInputs, QuarterlyObservations};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};

/// One encoded `Option<Decimal>` value: scale-normalized decimal string, or the `absent`
/// sentinel (not a valid decimal spelling, so it can never collide with a value).
fn enc(value: Option<Decimal>) -> String {
    match value {
        Some(v) => v.normalize().to_string(),
        None => "absent".to_string(),
    }
}

/// Deterministic SHA-256 hex digest over a documented, stable encoding of the three engine
/// inputs (ADD9 content address).
///
/// **Encoding (normative for this digest):** newline-joined `name=value` lines, fields in
/// struct order — every value field of every [`CanonicalFinancials`] year (years in their
/// canonical ascending order) plus `usable_years`, every [`JudgmentInputs`] field (the
/// forecast-low option by its snake_case name), and every [`QuarterlyObservations`] field.
/// Every `Decimal` is [`Decimal::normalize`]d before encoding, so value-equal inputs digest
/// equal (`"3.0"` = `"3"` — the `contract::provenance` NOTE). Derived per-year `usability`
/// and the `findings` are NOT encoded: both are pure functions of the encoded values and
/// would add no discriminating power. No map iteration, no pointer identity — stable across
/// runs and platforms.
///
/// Every input struct is **exhaustively destructured** (no `..`): adding a field to
/// [`CanonicalFinancials`], [`CanonicalYear`], [`JudgmentInputs`] or [`QuarterlyObservations`]
/// is a compile error here, forcing a decision on whether the new field enters the digest.
/// The encoded byte stream is unchanged by this pattern — golden hashes stay stable.
pub(super) fn inputs_digest(
    financials: &CanonicalFinancials,
    judgment: &JudgmentInputs,
    observations: &QuarterlyObservations,
) -> String {
    // `findings` is derived from the encoded values (see the encoding note above).
    let CanonicalFinancials {
        years,
        findings: _,
        usable_years,
    } = financials;
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("usable_years={usable_years}"));
    for y in years {
        // `usability` is derived from the encoded values (see the encoding note above).
        let CanonicalYear {
            year,
            sales,
            eps,
            high_price,
            low_price,
            dividend_per_share,
            pre_tax_profit,
            book_value_per_share,
            usability: _,
        } = y;
        lines.push(format!("year={year}"));
        lines.push(format!("sales={}", enc(*sales)));
        lines.push(format!("eps={}", enc(*eps)));
        lines.push(format!("high_price={}", enc(*high_price)));
        lines.push(format!("low_price={}", enc(*low_price)));
        lines.push(format!("dividend_per_share={}", enc(*dividend_per_share)));
        lines.push(format!("pre_tax_profit={}", enc(*pre_tax_profit)));
        lines.push(format!(
            "book_value_per_share={}",
            enc(*book_value_per_share)
        ));
    }
    let JudgmentInputs {
        estimated_high_eps,
        estimated_low_eps,
        projected_sales_growth_pct,
        projected_eps_growth_pct,
        judged_avg_high_pe,
        judged_avg_low_pe,
        forecast_low_option,
        recent_severe_low,
        current_price,
        present_full_year_dividend,
    } = judgment;
    lines.push(format!(
        "judgment.estimated_high_eps={}",
        enc(*estimated_high_eps)
    ));
    lines.push(format!(
        "judgment.estimated_low_eps={}",
        enc(*estimated_low_eps)
    ));
    lines.push(format!(
        "judgment.projected_sales_growth_pct={}",
        enc(*projected_sales_growth_pct)
    ));
    lines.push(format!(
        "judgment.projected_eps_growth_pct={}",
        enc(*projected_eps_growth_pct)
    ));
    lines.push(format!(
        "judgment.judged_avg_high_pe={}",
        enc(*judged_avg_high_pe)
    ));
    lines.push(format!(
        "judgment.judged_avg_low_pe={}",
        enc(*judged_avg_low_pe)
    ));
    lines.push(format!(
        "judgment.forecast_low_option={}",
        forecast_low_option_name(*forecast_low_option)
    ));
    lines.push(format!(
        "judgment.recent_severe_low={}",
        enc(*recent_severe_low)
    ));
    lines.push(format!("judgment.current_price={}", enc(*current_price)));
    lines.push(format!(
        "judgment.present_full_year_dividend={}",
        enc(*present_full_year_dividend)
    ));
    let QuarterlyObservations {
        ttm_quarterly_eps,
        latest_quarter_sales,
        latest_quarter_eps,
        year_ago_quarter_sales,
        year_ago_quarter_eps,
    } = observations;
    lines.push(format!(
        "observations.ttm_quarterly_eps={}",
        match ttm_quarterly_eps {
            Some(quarters) => quarters
                .iter()
                .map(|q| q.normalize().to_string())
                .collect::<Vec<_>>()
                .join("|"),
            None => "absent".to_string(),
        }
    ));
    lines.push(format!(
        "observations.latest_quarter_sales={}",
        enc(*latest_quarter_sales)
    ));
    lines.push(format!(
        "observations.latest_quarter_eps={}",
        enc(*latest_quarter_eps)
    ));
    lines.push(format!(
        "observations.year_ago_quarter_sales={}",
        enc(*year_ago_quarter_sales)
    ));
    lines.push(format!(
        "observations.year_ago_quarter_eps={}",
        enc(*year_ago_quarter_eps)
    ));

    let mut hasher = Sha256::new();
    hasher.update(lines.join("\n").as_bytes());
    crate::hex_sha256(hasher)
}

/// The snake_case name of a forecast-low option, for the digest encoding. NOTE: these are the
/// digest's OWN pinned spellings, not the contract's serde wire names — for `AvgLowPriceLast5y`
/// the digest encodes `avg_low_price_last_5y` while the `contract::ForecastLowOption` wire name
/// is `avg_low_price_last5y` (serde `snake_case` of `Last5y`). The divergence is frozen: changing
/// an encoded string here would silently orphan every stored `inputs_hash`.
fn forecast_low_option_name(option: ForecastLowOption) -> &'static str {
    match option {
        ForecastLowOption::AvgLowPeTimesEps => "avg_low_pe_times_eps",
        ForecastLowOption::AvgLowPriceLast5y => "avg_low_price_last_5y",
        ForecastLowOption::RecentSevereLow => "recent_severe_low",
        ForecastLowOption::DividendSupported => "dividend_supported",
    }
}
