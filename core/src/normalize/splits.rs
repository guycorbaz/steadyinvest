//! Declared-split rebasing + undeclared series-break detection (spec §3, story 1.7 AC 2).
//!
//! Only **declared** splits adjust data; a **suspected** undeclared break is a finding at the
//! year and the values pass through unchanged — silently "fixing" data would be the exact
//! silent-wrong-signal this layer exists to kill.

use super::types::{CanonicalYear, Finding, PlausibilityKey, SplitEvent};
use crate::method::{split_jump_high, split_jump_low};
use rust_decimal::Decimal;

/// Cumulative rebase ratio for `year`: the product over all declared splits effective strictly
/// AFTER it (a split rebases strictly-pre-split years into post-split shares). Returned as an
/// exact integer pair `(numerator, denominator)`; the per-share rebase multiplies by
/// `denominator` and divides by `numerator`. `None` on (astronomically unlikely) `u64`
/// overflow — callers degrade the value to unknown rather than panic.
pub(super) fn cumulative_ratio(year: i32, splits: &[SplitEvent]) -> Option<(u64, u64)> {
    let mut numerator: u64 = 1;
    let mut denominator: u64 = 1;
    for split in splits {
        if split.effective_year > year {
            numerator = numerator.checked_mul(u64::from(split.numerator))?;
            denominator = denominator.checked_mul(u64::from(split.denominator))?;
        }
    }
    Some((numerator, denominator))
}

/// Rebase one per-share value into post-split shares: `value × denominator / numerator`, fully
/// checked (`checked_mul`/`checked_div`) — never a panic; any failure degrades to `None`
/// (unknown), never to a silent 0. A `1/numerator` quotient may be non-terminating (e.g. 1/3):
/// `Decimal` division keeps 28 significant digits, deterministically — exactness for the
/// metamorphic tests comes from exact-by-construction inputs, not from this function.
pub(super) fn rebase_per_share(
    value: Option<Decimal>,
    ratio: Option<(u64, u64)>,
) -> Option<Decimal> {
    let v = value?;
    let (numerator, denominator) = ratio?;
    if numerator == denominator {
        return Some(v);
    }
    let scaled = v.checked_mul(Decimal::from(denominator))?;
    scaled.checked_div(Decimal::from(numerator))
}

/// Year-over-year factor `cur / prev` — `None` when either value is absent or `prev` is zero
/// (no factor ⇒ no flag: absence of evidence is not evidence; and no division-by-zero panic).
fn yoy_factor(prev: Option<Decimal>, cur: Option<Decimal>) -> Option<Decimal> {
    let p = prev?;
    let c = cur?;
    if p.is_zero() {
        return None;
    }
    c.checked_div(p)
}

/// Detect suspected **undeclared** splits / series breaks (pinned key `split_series_break`).
///
/// Runs on the POST-adjustment series, so a correctly declared split no longer trips the
/// detector — only *unexplained* breaks are flagged. Year `t` is flagged (per field) when its
/// EPS / high-price / low-price y/y factor is ≥ [`split_jump_high`] (1.5) or ≤
/// [`split_jump_low`] (0.67) — boundary values included, the comparators are normative
/// (spec §7) — while the sales factor does NOT move beyond the same band in the same direction
/// (absent sales or an opposite move ⇒ still flagged). Comparison only across calendar-
/// consecutive years (a gap in the series is not "year-over-year"). Flag only — values pass
/// through unchanged. The spec leaves "inconsistent with sales" unquantified; this
/// interpretation is recorded as a GitHub issue for the next METHOD_VERSION bump (story Task 3).
pub(super) fn detect_breaks(years: &[CanonicalYear], findings: &mut Vec<Finding>) {
    for pair in years.windows(2) {
        let (prev, cur) = (&pair[0], &pair[1]);
        if cur.year != prev.year + 1 {
            continue;
        }
        let sales_factor = yoy_factor(prev.sales, cur.sales);
        let per_share: [(&'static str, Option<Decimal>, Option<Decimal>); 3] = [
            ("eps", prev.eps, cur.eps),
            ("high_price", prev.high_price, cur.high_price),
            ("low_price", prev.low_price, cur.low_price),
        ];
        for (field, p, c) in per_share {
            let Some(factor) = yoy_factor(p, c) else {
                continue;
            };
            let jump_up = factor >= split_jump_high();
            let jump_down = factor <= split_jump_low();
            let explained_by_sales = (jump_up
                && sales_factor.is_some_and(|s| s >= split_jump_high()))
                || (jump_down && sales_factor.is_some_and(|s| s <= split_jump_low()));
            if (jump_up || jump_down) && !explained_by_sales {
                findings.push(Finding {
                    key: PlausibilityKey::SplitSeriesBreak,
                    year: cur.year,
                    context: field,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::types::YearUsability;

    fn d(mantissa: i64, scale: u32) -> Decimal {
        Decimal::new(mantissa, scale)
    }

    fn year(
        y: i32,
        sales: Option<Decimal>,
        eps: Option<Decimal>,
        high: Option<Decimal>,
        low: Option<Decimal>,
    ) -> CanonicalYear {
        CanonicalYear {
            year: y,
            sales,
            eps,
            high_price: high,
            low_price: low,
            dividend_per_share: None,
            pre_tax_profit: None,
            book_value_per_share: None,
            usability: YearUsability::Usable,
        }
    }

    #[test]
    fn cumulative_ratio_only_counts_splits_effective_after_the_year() {
        let splits = [
            SplitEvent {
                effective_year: 2020,
                numerator: 2,
                denominator: 1,
            },
            SplitEvent {
                effective_year: 2022,
                numerator: 3,
                denominator: 1,
            },
        ];
        assert_eq!(cumulative_ratio(2019, &splits), Some((6, 1)));
        assert_eq!(cumulative_ratio(2020, &splits), Some((3, 1)));
        assert_eq!(cumulative_ratio(2021, &splits), Some((3, 1)));
        assert_eq!(cumulative_ratio(2022, &splits), Some((1, 1)));
        assert_eq!(cumulative_ratio(2023, &[]), Some((1, 1)));
    }

    #[test]
    fn rebase_is_exact_for_divisible_values() {
        // 9.00 / 3 = 3.00 exactly — the exact-by-construction discipline of the metamorphic suite.
        assert_eq!(
            rebase_per_share(Some(d(900, 2)), Some((3, 1))),
            Some(d(300, 2))
        );
        // Reverse split 1:3 multiplies: 2.50 × 3 = 7.50 exactly.
        assert_eq!(
            rebase_per_share(Some(d(250, 2)), Some((1, 3))),
            Some(d(750, 2))
        );
        // Identity ratio passes the value through untouched.
        assert_eq!(
            rebase_per_share(Some(d(123, 2)), Some((1, 1))),
            Some(d(123, 2))
        );
        // Missing value stays missing — never a 0.
        assert_eq!(rebase_per_share(None, Some((3, 1))), None);
        // Overflowed ratio degrades to unknown, never panics.
        assert_eq!(rebase_per_share(Some(d(100, 0)), None), None);
    }

    #[test]
    fn yoy_factor_guards_zero_and_missing_priors() {
        assert_eq!(yoy_factor(None, Some(d(2, 0))), None);
        assert_eq!(yoy_factor(Some(d(2, 0)), None), None);
        assert_eq!(yoy_factor(Some(Decimal::ZERO), Some(d(2, 0))), None);
        assert_eq!(yoy_factor(Some(d(2, 0)), Some(d(3, 0))), Some(d(15, 1)));
    }

    #[test]
    fn band_edges_are_normative_inclusive() {
        // Factor exactly 1.5 (2.00 → 3.00) with flat sales ⇒ flagged (≥, spec §7).
        let up = [
            year(2020, Some(d(100, 0)), Some(d(200, 2)), None, None),
            year(2021, Some(d(100, 0)), Some(d(300, 2)), None, None),
        ];
        let mut findings = Vec::new();
        detect_breaks(&up, &mut findings);
        assert_eq!(
            findings,
            vec![Finding {
                key: PlausibilityKey::SplitSeriesBreak,
                year: 2021,
                context: "eps",
            }],
            "factor exactly 1.5 must be flagged (normative ≥): {findings:?}"
        );

        // Factor exactly 0.67 (1.00 → 0.67) with flat sales ⇒ flagged (≤, spec §7).
        let down = [
            year(2020, Some(d(100, 0)), Some(d(100, 2)), None, None),
            year(2021, Some(d(100, 0)), Some(d(67, 2)), None, None),
        ];
        let mut findings = Vec::new();
        detect_breaks(&down, &mut findings);
        assert_eq!(
            findings.len(),
            1,
            "factor exactly 0.67 must be flagged (normative ≤): {findings:?}"
        );

        // Factors strictly inside the band ⇒ no flag.
        let inside = [
            year(
                2020,
                Some(d(100, 0)),
                Some(d(100, 2)),
                Some(d(100, 2)),
                None,
            ),
            year(2021, Some(d(100, 0)), Some(d(149, 2)), Some(d(68, 2)), None),
        ];
        let mut findings = Vec::new();
        detect_breaks(&inside, &mut findings);
        assert!(
            findings.is_empty(),
            "factors 1.49 and 0.68 are inside the band: {findings:?}"
        );
    }

    #[test]
    fn jump_explained_by_sales_moving_same_direction_is_not_flagged() {
        // EPS doubles AND sales double: a real business event, not a suspected split.
        let years = [
            year(2020, Some(d(100, 0)), Some(d(100, 2)), None, None),
            year(2021, Some(d(200, 0)), Some(d(200, 2)), None, None),
        ];
        let mut findings = Vec::new();
        detect_breaks(&years, &mut findings);
        assert!(
            findings.is_empty(),
            "co-moving sales explains the jump: {findings:?}"
        );
    }

    #[test]
    fn jump_with_opposite_or_absent_sales_is_flagged() {
        // EPS doubles while sales halve ⇒ flagged.
        let opposite = [
            year(2020, Some(d(200, 0)), Some(d(100, 2)), None, None),
            year(2021, Some(d(100, 0)), Some(d(200, 2)), None, None),
        ];
        let mut findings = Vec::new();
        detect_breaks(&opposite, &mut findings);
        assert_eq!(
            findings.len(),
            1,
            "opposite sales move must flag: {findings:?}"
        );

        // EPS doubles, sales absent ⇒ flagged (cannot be explained).
        let absent = [
            year(2020, None, Some(d(100, 2)), None, None),
            year(2021, None, Some(d(200, 2)), None, None),
        ];
        let mut findings = Vec::new();
        detect_breaks(&absent, &mut findings);
        assert_eq!(
            findings.len(),
            1,
            "absent sales cannot explain: {findings:?}"
        );
    }

    #[test]
    fn gap_years_are_not_compared() {
        // 2019 → 2021 is not year-over-year: no factor, no flag.
        let years = [
            year(2019, Some(d(100, 0)), Some(d(100, 2)), None, None),
            year(2021, Some(d(100, 0)), Some(d(300, 2)), None, None),
        ];
        let mut findings = Vec::new();
        detect_breaks(&years, &mut findings);
        assert!(
            findings.is_empty(),
            "gap years must not be compared: {findings:?}"
        );
    }

    #[test]
    fn zero_prior_value_produces_no_factor_and_no_panic() {
        let years = [
            year(2020, Some(Decimal::ZERO), Some(Decimal::ZERO), None, None),
            year(2021, Some(d(100, 0)), Some(d(300, 2)), None, None),
        ];
        let mut findings = Vec::new();
        detect_breaks(&years, &mut findings);
        assert!(
            findings.is_empty(),
            "zero prior ⇒ no factor ⇒ no flag: {findings:?}"
        );
    }
}
