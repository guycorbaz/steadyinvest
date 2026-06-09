//! Named rounding mode + per-field display scale (see `docs/method/ssg-method-spec-v1.md` §8).
//!
//! IMPORTANT: rounding is applied **only at display**. Calculations keep full `Decimal` precision;
//! never round mid-chain.

use rust_decimal::{Decimal, RoundingStrategy};

/// Project-wide display rounding: **half-up** (e.g. 2.5 → 3), for paper-form fidelity.
/// Deliberately NOT `rust_decimal`'s default banker's rounding (MidpointNearestEven).
pub const DISPLAY_ROUNDING: RoundingStrategy = RoundingStrategy::MidpointAwayFromZero;

/// Display field groups (each maps to a fixed number of decimal places).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayField {
    /// Prices: high/low/current/forecast/zone bounds.
    Price,
    /// Per-share values: EPS, dividend per share.
    PerShare,
    /// Price/earnings ratios.
    PeRatio,
    /// Percentages: PTP, ROE, payout, yield, growth, relative value.
    Percent,
    /// Upside/downside ratio.
    Ratio,
    /// Large monetary aggregates (e.g. sales).
    LargeMonetary,
}

impl DisplayField {
    /// Decimal places used when presenting this field.
    pub const fn scale(self) -> u32 {
        match self {
            DisplayField::Price => 2,
            DisplayField::PerShare => 2,
            DisplayField::PeRatio => 1,
            DisplayField::Percent => 1,
            DisplayField::Ratio => 1,
            DisplayField::LargeMonetary => 0,
        }
    }
}

/// Round a value for display using the named mode + the field's scale.
/// Never call this mid-calculation — only when presenting a final value.
pub fn round_for_display(value: Decimal, field: DisplayField) -> Decimal {
    value.round_dp_with_strategy(field.scale(), DISPLAY_ROUNDING)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_rounding_strategy_is_half_up_not_bankers() {
        // Strategy at scale 0: 2.5 -> 3 (half-up). Banker's rounding would give 2.
        assert_eq!(
            Decimal::new(25, 1).round_dp_with_strategy(0, DISPLAY_ROUNDING),
            Decimal::new(3, 0)
        );
        // 0.5 -> 1 (half-up). Banker's would give 0.
        assert_eq!(
            Decimal::new(5, 1).round_dp_with_strategy(0, DISPLAY_ROUNDING),
            Decimal::ONE
        );
    }

    #[test]
    fn ratio_field_rounds_at_one_decimal_half_up() {
        // Ratio keeps 1 decimal; midpoint at the 1st decimal: 3.05 -> 3.1 (half-up; banker's -> 3.0).
        assert_eq!(
            round_for_display(Decimal::new(305, 2), DisplayField::Ratio),
            Decimal::new(31, 1)
        );
    }

    #[test]
    fn price_scale_is_two_decimals() {
        let v = Decimal::new(141005, 3); // 141.005
        assert_eq!(
            round_for_display(v, DisplayField::Price),
            Decimal::new(14101, 2)
        ); // 141.01
    }
}
