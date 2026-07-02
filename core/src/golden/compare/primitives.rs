//! Comparison **primitives** of the golden self-check: neutral renderers of engine values
//! (`*_str` / `opt_*`), the `Expected*` → engine-type conversions (compare on engine
//! vocabulary), and the three comparison building blocks — [`compare_numeric`] (method
//! tolerance, spec §7), [`compare_exact`] (categorical) and [`compare_list`] (ordered). The
//! section walkers live in `sections`; nothing here knows the output shape.

use super::GoldenDeviation;
use crate::golden::schema::{ExpectedCriterion, ExpectedTrend, ExpectedZone};
use crate::method::golden_relative_tolerance;
use crate::ssg::{CriterionFact, Trend, UpsideDownside, Zone};
use rust_decimal::Decimal;

// ── Rendering (neutral vocabulary — see the posture test in `core/tests/golden_compare.rs`) ──

pub(super) fn opt_decimal(value: Option<Decimal>) -> String {
    match value {
        Some(d) => d.to_string(),
        None => "null".to_string(),
    }
}

pub(super) fn zone_str(zone: Zone) -> &'static str {
    // Zone labels name the user-defined price bands (spec §6 exemption).
    match zone {
        Zone::Buy => "buy",
        Zone::Neutral => "neutral",
        Zone::Sell => "sell",
    }
}

pub(super) fn opt_zone(zone: Option<Zone>) -> String {
    match zone {
        Some(z) => zone_str(z).to_string(),
        None => "null".to_string(),
    }
}

pub(super) fn trend_str(trend: Trend) -> &'static str {
    match trend {
        Trend::Up => "up",
        Trend::Even => "even",
        Trend::Down => "down",
    }
}

pub(super) fn opt_trend(trend: Option<Trend>) -> String {
    match trend {
        Some(t) => trend_str(t).to_string(),
        None => "null".to_string(),
    }
}

pub(super) fn criterion_str(fact: CriterionFact) -> &'static str {
    match fact {
        CriterionFact::Met => "met",
        CriterionFact::Unmet => "unmet",
        CriterionFact::UnmetByInsufficiency => "unmet_by_insufficiency",
    }
}

pub(super) fn ud_str(ud: UpsideDownside) -> String {
    match ud {
        UpsideDownside::Ratio(r) => format!("ratio {r}"),
        UpsideDownside::Undefined => "undefined".to_string(),
        UpsideDownside::Unknown => "unknown".to_string(),
    }
}

pub(super) fn calc_finding_str(key: &str, year: Option<i32>, context: &str) -> String {
    match year {
        Some(y) => format!("{key} year {y} context {context}"),
        None => format!("{key} study-level context {context}"),
    }
}

// ── Expected → engine-type conversions (compare on engine vocabulary) ──

pub(super) fn expected_zone(zone: ExpectedZone) -> Zone {
    match zone {
        ExpectedZone::Buy => Zone::Buy,
        ExpectedZone::Neutral => Zone::Neutral,
        ExpectedZone::Sell => Zone::Sell,
    }
}

pub(super) fn expected_trend(trend: ExpectedTrend) -> Trend {
    match trend {
        ExpectedTrend::Up => Trend::Up,
        ExpectedTrend::Even => Trend::Even,
        ExpectedTrend::Down => Trend::Down,
    }
}

pub(super) fn expected_criterion(fact: ExpectedCriterion) -> CriterionFact {
    match fact {
        ExpectedCriterion::Met => CriterionFact::Met,
        ExpectedCriterion::Unmet => CriterionFact::Unmet,
        ExpectedCriterion::UnmetByInsufficiency => CriterionFact::UnmetByInsufficiency,
    }
}

// ── Comparison primitives ──

/// Numeric within the method tolerance: `|actual − expected| ≤ tol × |expected|` (spec §7,
/// symmetric, relative to the EXPECTED value — `expected == 0` demands exact equality, no
/// special case needed). Unknown matches only unknown: expected `null` ⇔ actual `None`,
/// both directions.
pub(super) fn compare_numeric(
    path: &str,
    expected: Option<Decimal>,
    actual: Option<Decimal>,
    deviations: &mut Vec<GoldenDeviation>,
) {
    match (expected, actual) {
        (None, None) => {}
        (Some(e), Some(a)) => {
            let diff = a.checked_sub(e).map(|d| d.abs());
            let tolerance = golden_relative_tolerance().checked_mul(e.abs());
            let within = matches!((diff, tolerance), (Some(d), Some(t)) if d <= t);
            if !within {
                let relative_error = match diff {
                    Some(d) if !e.is_zero() => d.checked_div(e.abs()),
                    _ => None,
                };
                deviations.push(GoldenDeviation {
                    path: path.to_string(),
                    expected: e.to_string(),
                    actual: a.to_string(),
                    relative_error,
                });
            }
        }
        (e, a) => deviations.push(GoldenDeviation {
            path: path.to_string(),
            expected: opt_decimal(e),
            actual: opt_decimal(a),
            relative_error: None,
        }),
    }
}

/// Exact categorical comparison on rendered values.
pub(super) fn compare_exact(
    path: &str,
    expected: String,
    actual: String,
    deviations: &mut Vec<GoldenDeviation>,
) {
    if expected != actual {
        deviations.push(GoldenDeviation {
            path: path.to_string(),
            expected,
            actual,
            relative_error: None,
        });
    }
}

/// Ordered-list comparison on rendered elements — one deviation carrying both full lists.
pub(super) fn compare_list(
    path: &str,
    expected: &[String],
    actual: &[String],
    deviations: &mut Vec<GoldenDeviation>,
) {
    if expected != actual {
        deviations.push(GoldenDeviation {
            path: path.to_string(),
            expected: format!("[{}]", expected.join("; ")),
            actual: format!("[{}]", actual.join("; ")),
            relative_error: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        s.parse()
            .unwrap_or_else(|_| panic!("bad decimal literal {s:?}"))
    }

    fn deviations_of(expected: Option<Decimal>, actual: Option<Decimal>) -> Vec<GoldenDeviation> {
        let mut deviations = Vec::new();
        compare_numeric("probe", expected, actual, &mut deviations);
        deviations
    }

    /// Spec §7 boundary: exactly at ±0.5% of the EXPECTED value passes; just beyond fails.
    #[test]
    fn tolerance_boundary_is_inclusive_at_half_percent() {
        // expected 100 ⇒ tolerance 0.5 — actual 100.5 / 99.5 pass, 100.51 / 99.49 fail.
        assert!(deviations_of(Some(d("100")), Some(d("100.5"))).is_empty());
        assert!(deviations_of(Some(d("100")), Some(d("99.5"))).is_empty());
        let over = deviations_of(Some(d("100")), Some(d("100.51")));
        assert_eq!(over.len(), 1, "just beyond the tolerance must deviate");
        assert_eq!(over[0].relative_error, Some(d("0.0051")));
        assert_eq!(deviations_of(Some(d("100")), Some(d("99.49"))).len(), 1);
        // Tolerance is relative to EXPECTED: expected 10.05 vs actual 10 passes
        // (0.05 ≤ 0.005 × 10.05 = 0.05025); expected 10.06 vs actual 10 fails.
        assert!(deviations_of(Some(d("10.05")), Some(d("10"))).is_empty());
        assert_eq!(deviations_of(Some(d("10.06")), Some(d("10"))).len(), 1);
    }

    /// The §7 formula itself makes `expected == 0` demand exact equality (0.005 × 0 = 0).
    #[test]
    fn expected_zero_demands_exact_equality() {
        assert!(deviations_of(Some(d("0")), Some(d("0"))).is_empty());
        assert!(
            deviations_of(Some(d("0")), Some(d("0.00"))).is_empty(),
            "value equality"
        );
        let off = deviations_of(Some(d("0")), Some(d("0.0001")));
        assert_eq!(off.len(), 1);
        assert_eq!(
            off[0].relative_error, None,
            "no relative error against zero"
        );
    }

    /// Unknown matches only unknown — both directions deviate.
    #[test]
    fn null_and_none_must_agree_in_both_directions() {
        assert!(deviations_of(None, None).is_empty());
        let expected_null = deviations_of(None, Some(d("1")));
        assert_eq!(expected_null.len(), 1);
        assert_eq!(expected_null[0].expected, "null");
        assert_eq!(expected_null[0].actual, "1");
        let actual_none = deviations_of(Some(d("1")), None);
        assert_eq!(actual_none.len(), 1);
        assert_eq!(actual_none[0].expected, "1");
        assert_eq!(actual_none[0].actual, "null");
    }
}
