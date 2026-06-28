//! Portfolio **risk overlay** (FR42–48) — a deliberately **decoupled** subsystem, separate from the
//! SSG engine (`core::ssg`). The PRD keeps risk management as an optional overlay that never weighs
//! down the pure SSG calc: nothing here is imported by `core::ssg`, and the SSG method fingerprint /
//! golden corpus / determinism gates are unaffected by this module.
//!
//! Story 4.5 (FR42) opens it with the **trailing-stop** primitive: a stop level that **ratchets up
//! only**. All math is exact [`Decimal`] — never `f64`.

use rust_decimal::Decimal;

/// The ratcheted trailing-stop **level** (a price) after observing `reference_price` (Story 4.5,
/// FR42). The candidate level is `reference_price × (1 − pct/100)`; the returned level is the
/// **maximum** of the prior level and that candidate, so it **ratchets up only** — a falling price
/// (or a looser `pct`) never lowers it. With no prior level (`None`), it seeds from the candidate.
///
/// `pct` is the trailing-stop percentage (e.g. `15` for 15 %), assumed already validated to `(0,
/// 100)` by the caller. Exact decimal throughout.
///
/// ```
/// use rust_decimal::Decimal;
/// use steadyinvest_core::risk::ratchet_trailing_stop;
/// let pct = Decimal::from(15);
/// // First set at price 100 → 85.
/// let level = ratchet_trailing_stop(None, Decimal::from(100), pct);
/// assert_eq!(level, Decimal::from(85));
/// // Price rises to 120 → ratchets up to 102.
/// let level = ratchet_trailing_stop(Some(level), Decimal::from(120), pct);
/// assert_eq!(level, Decimal::from(102));
/// // Price falls to 90 → level holds at 102 (ratchet-up only).
/// let level = ratchet_trailing_stop(Some(level), Decimal::from(90), pct);
/// assert_eq!(level, Decimal::from(102));
/// ```
pub fn ratchet_trailing_stop(
    prior_level: Option<Decimal>,
    reference_price: Decimal,
    pct: Decimal,
) -> Decimal {
    let candidate = reference_price * (Decimal::ONE - pct / Decimal::ONE_HUNDRED);
    match prior_level {
        Some(prior) => prior.max(candidate),
        None => candidate,
    }
}

/// Whether the current price has reached or fallen through the trailing-stop level (Story 4.5) — a
/// pure **state**, surfaced as a neutral fact (the user arbitrates; the app never acts). `true` when
/// `current_price ≤ stop_level`.
pub fn stop_breached(stop_level: Decimal, current_price: Decimal) -> bool {
    current_price <= stop_level
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(n: i64) -> Decimal {
        Decimal::from(n)
    }

    #[test]
    fn first_set_seeds_the_level_from_the_reference_price() {
        // 200 × (1 − 0.10) = 180.
        assert_eq!(ratchet_trailing_stop(None, d(200), d(10)), d(180));
    }

    #[test]
    fn a_rising_reference_price_ratchets_the_level_up() {
        let pct = d(20);
        let level = ratchet_trailing_stop(None, d(100), pct); // 80
        assert_eq!(level, d(80));
        let level = ratchet_trailing_stop(Some(level), d(150), pct); // 120 > 80 → 120
        assert_eq!(level, d(120));
    }

    #[test]
    fn a_falling_reference_price_never_lowers_the_level() {
        let pct = d(20);
        let level = ratchet_trailing_stop(Some(d(120)), d(80), pct); // candidate 64 < 120 → stays 120
        assert_eq!(level, d(120));
    }

    #[test]
    fn a_looser_pct_never_lowers_an_existing_level() {
        // Prior level 102 (from a 15% stop at 120). Raising pct to 30 → candidate 120×0.70 = 84 < 102.
        let level = ratchet_trailing_stop(Some(Decimal::from(102)), d(120), d(30));
        assert_eq!(level, Decimal::from(102), "the ratchet never drops");
    }

    #[test]
    fn exact_decimal_scale_is_preserved() {
        // 33.33 × (1 − 0.10) = 29.997 — exact, no f64 rounding.
        let level = ratchet_trailing_stop(None, Decimal::from_str_exact("33.33").unwrap(), d(10));
        assert_eq!(level, Decimal::from_str_exact("29.997").unwrap());
    }

    #[test]
    fn breach_is_at_or_below_the_stop() {
        assert!(
            stop_breached(d(100), d(100)),
            "exactly at the stop is a breach"
        );
        assert!(stop_breached(d(100), d(99)));
        assert!(!stop_breached(d(100), d(101)));
    }
}
