//! The FX conversion primitive (Story 6.5, FR28) — pure, explicit, consolidation-only.
//!
//! FR28's rule is structural, not advisory: every study and every per-currency figure stays in
//! its NATIVE currency; conversion happens **only at the consolidation points** — the Story-6.6
//! per-currency → per-bank → global roll-up, the Story-6.7 concentration/diversify-by-size
//! read, and the Story-6.8 per-currency exposure read (all journal-wide `app::state` reads).
//! This module is deliberately tiny — one checked multiply — so the rule stays auditable: grep
//! [`convert`]'s callers; outside tests they all live in those enumerated consolidation reads. The CALLER picks the dated, source-aware rate (from the journal's
//! `fx_rates` rows) and remains accountable for showing its date and source; nothing here (or
//! anywhere else) selects, inverts or interpolates a rate implicitly.
//!
//! Like the rest of `core::risk`, a decoupled overlay: nothing imported by `core::ssg`; exact
//! [`Decimal`] only; checked arithmetic — no panic on any input.

use rust_decimal::Decimal;

/// Convert `amount` (in the rate's BASE currency) into the rate's QUOTE currency:
/// `amount × rate`, checked — `None` on `Decimal` overflow (the caller refuses honestly rather
/// than showing a wrong figure). A **negative** amount passes through with its sign (a signed
/// consolidation delta is legal — the caller decides what a negative aggregate means); the rate
/// itself is the caller's responsibility (a stored rate is validated positive at entry).
pub fn convert(amount: Decimal, rate: Decimal) -> Option<Decimal> {
    amount.checked_mul(rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    #[test]
    fn converts_exactly() {
        // 150 USD at 0.8850 USD→CHF = 132.75 CHF — exact, no float drift.
        assert_eq!(convert(dec("150"), dec("0.8850")), Some(dec("132.7500")));
    }

    #[test]
    fn a_negative_amount_passes_through_signed() {
        assert_eq!(convert(dec("-10"), dec("2")), Some(dec("-20")));
    }

    #[test]
    fn overflow_is_none_never_a_panic() {
        assert_eq!(
            convert(dec("79228162514264337593543950335"), dec("2")),
            None,
            "beyond Decimal range → the caller refuses honestly"
        );
    }
}
