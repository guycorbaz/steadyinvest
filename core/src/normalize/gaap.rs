//! IFRS ↔ US-GAAP canonicalization (spec §2, story 1.7 AC 3).
//!
//! v1 scope is the spec-named equivalence: pre-tax profit is taken directly when provided, or
//! derived by the §2 gross-up `pre_tax_profit = net_profit / (1 − tax_rate)`. Equivalent
//! representations of the same economics canonicalize to the same output.

use rust_decimal::Decimal;

/// Canonical pre-tax profit for one year.
///
/// Precedence: a directly-reported `pre_tax_profit` always wins (spec §9 "prefer a
/// directly-reported pre-tax profit"). Otherwise the gross-up derives it from `net_profit` and
/// `tax_rate` — `tax_rate` is a fraction in `[0, 1)`; `tax_rate ≥ 1` means a non-positive
/// denominator, so the result is **unknown** (`None`, spec §9), never computed and never 0.
/// All arithmetic is checked: no panic on any input.
pub(super) fn canonical_pre_tax_profit(
    pre_tax_profit: Option<Decimal>,
    net_profit: Option<Decimal>,
    tax_rate: Option<Decimal>,
) -> Option<Decimal> {
    if let Some(direct) = pre_tax_profit {
        return Some(direct);
    }
    let net = net_profit?;
    let rate = tax_rate?;
    if rate >= Decimal::ONE {
        return None;
    }
    let denominator = Decimal::ONE.checked_sub(rate)?;
    net.checked_div(denominator)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(mantissa: i64, scale: u32) -> Decimal {
        Decimal::new(mantissa, scale)
    }

    #[test]
    fn direct_ptp_wins_over_gross_up_inputs() {
        // Direct 120 beats a derivable 100 (70 / 0.7) — spec §9 preference.
        assert_eq!(
            canonical_pre_tax_profit(Some(d(120, 0)), Some(d(70, 0)), Some(d(30, 2))),
            Some(d(120, 0))
        );
    }

    #[test]
    fn gross_up_recovers_the_pre_tax_profit_exactly() {
        // 70 / (1 − 0.30) = 100 exactly.
        assert_eq!(
            canonical_pre_tax_profit(None, Some(d(70, 0)), Some(d(30, 2))),
            Some(d(100, 0))
        );
        // Zero tax rate: PTP = net.
        assert_eq!(
            canonical_pre_tax_profit(None, Some(d(70, 0)), Some(Decimal::ZERO)),
            Some(d(70, 0))
        );
    }

    #[test]
    fn tax_rate_at_or_above_one_yields_unknown_never_a_number() {
        assert_eq!(
            canonical_pre_tax_profit(None, Some(d(70, 0)), Some(Decimal::ONE)),
            None,
            "tax_rate = 1 divides by zero — must be unknown (spec §9)"
        );
        assert_eq!(
            canonical_pre_tax_profit(None, Some(d(70, 0)), Some(d(15, 1))),
            None,
            "tax_rate = 1.5 implies a negative denominator — must be unknown (spec §9)"
        );
    }

    #[test]
    fn missing_inputs_stay_unknown_never_zero() {
        assert_eq!(canonical_pre_tax_profit(None, None, None), None);
        assert_eq!(canonical_pre_tax_profit(None, Some(d(70, 0)), None), None);
        assert_eq!(canonical_pre_tax_profit(None, None, Some(d(30, 2))), None);
    }
}
