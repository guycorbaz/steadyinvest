//! Currency-of-report + fiscal-period plausibility checks (spec §3, story 1.7 AC 4–5).
//!
//! Flag only: values pass through unconverted/unchanged, and findings never block computation.

use super::types::{Finding, PlausibilityKey, RawAmount, RawYear};

/// The monetary fields of a year in deterministic catalog order — findings order is part of
/// the determinism contract.
fn amounts(year: &RawYear) -> [(&'static str, Option<&RawAmount>); 8] {
    [
        ("sales", year.sales.as_ref()),
        ("eps", year.eps.as_ref()),
        ("high_price", year.high_price.as_ref()),
        ("low_price", year.low_price.as_ref()),
        ("dividend_per_share", year.dividend_per_share.as_ref()),
        ("pre_tax_profit", year.pre_tax_profit.as_ref()),
        ("net_profit", year.net_profit.as_ref()),
        ("book_value_per_share", year.book_value_per_share.as_ref()),
    ]
}

/// `currency_mismatch`: any amount whose reporting currency ≠ the study's native currency
/// (verbatim comparison) is flagged per year/field. `core` performs **no FX conversion** — the
/// value passes through unconverted (FX exists only at the future consolidation layer).
pub(super) fn currency_findings(years: &[RawYear], native: &str, findings: &mut Vec<Finding>) {
    for year in years {
        for (field, amount) in amounts(year) {
            if let Some(a) = amount
                && a.currency != native
            {
                findings.push(Finding {
                    key: PlausibilityKey::CurrencyMismatch,
                    year: year.year,
                    context: field,
                });
            }
        }
    }
}

/// `fiscal_period_misalignment`: a reported period length ≠ 12 months, or a fiscal-year-end
/// month shift between consecutive reported years (spec §3). Flag only — the year still passes
/// through. `years` must already be sorted ascending.
pub(super) fn fiscal_findings(years: &[RawYear], findings: &mut Vec<Finding>) {
    for year in years {
        if year.period_months.is_some_and(|m| m != 12) {
            findings.push(Finding {
                key: PlausibilityKey::FiscalPeriodMisalignment,
                year: year.year,
                context: "period_months",
            });
        }
    }
    for pair in years.windows(2) {
        let (prev, cur) = (&pair[0], &pair[1]);
        if let (Some(a), Some(b)) = (prev.fiscal_year_end_month, cur.fiscal_year_end_month)
            && a != b
        {
            findings.push(Finding {
                key: PlausibilityKey::FiscalPeriodMisalignment,
                year: cur.year,
                context: "fiscal_year_end_month",
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn amount(currency: &str) -> Option<RawAmount> {
        Some(RawAmount {
            value: Decimal::ONE,
            currency: currency.to_string(),
        })
    }

    #[test]
    fn foreign_currency_amounts_are_flagged_per_field() {
        let mut year = RawYear::empty(2024);
        year.sales = amount("USD");
        year.eps = amount("EUR");
        year.net_profit = amount("EUR");
        let mut findings = Vec::new();
        currency_findings(&[year], "USD", &mut findings);
        assert_eq!(
            findings,
            vec![
                Finding {
                    key: PlausibilityKey::CurrencyMismatch,
                    year: 2024,
                    context: "eps",
                },
                Finding {
                    key: PlausibilityKey::CurrencyMismatch,
                    year: 2024,
                    context: "net_profit",
                },
            ],
            "exactly the two EUR amounts must be flagged"
        );
    }

    #[test]
    fn native_currency_amounts_raise_no_finding() {
        let mut year = RawYear::empty(2024);
        year.sales = amount("CHF");
        year.eps = amount("CHF");
        let mut findings = Vec::new();
        currency_findings(&[year], "CHF", &mut findings);
        assert!(findings.is_empty(), "no mismatch expected: {findings:?}");
    }

    #[test]
    fn non_twelve_month_period_is_flagged() {
        let mut short = RawYear::empty(2023);
        short.period_months = Some(9);
        let mut normal = RawYear::empty(2024);
        normal.period_months = Some(12);
        let mut findings = Vec::new();
        fiscal_findings(&[short, normal], &mut findings);
        assert_eq!(
            findings,
            vec![Finding {
                key: PlausibilityKey::FiscalPeriodMisalignment,
                year: 2023,
                context: "period_months",
            }],
            "only the 9-month period must be flagged"
        );
    }

    #[test]
    fn fiscal_year_end_shift_is_flagged_on_the_later_year() {
        let mut a = RawYear::empty(2022);
        a.fiscal_year_end_month = Some(12);
        let mut b = RawYear::empty(2023);
        b.fiscal_year_end_month = Some(6);
        let mut c = RawYear::empty(2024);
        c.fiscal_year_end_month = Some(6);
        let mut findings = Vec::new();
        fiscal_findings(&[a, b, c], &mut findings);
        assert_eq!(
            findings,
            vec![Finding {
                key: PlausibilityKey::FiscalPeriodMisalignment,
                year: 2023,
                context: "fiscal_year_end_month",
            }],
            "the shift lands on the later year, once"
        );
    }

    #[test]
    fn missing_fiscal_metadata_raises_no_finding() {
        let a = RawYear::empty(2023);
        let mut b = RawYear::empty(2024);
        b.fiscal_year_end_month = Some(6);
        let mut findings = Vec::new();
        fiscal_findings(&[a, b], &mut findings);
        assert!(
            findings.is_empty(),
            "absent metadata is not evidence of misalignment: {findings:?}"
        );
    }
}
