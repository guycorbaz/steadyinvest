//! Portfolio **risk overlay** (FR42–48) — a deliberately **decoupled** subsystem, separate from the
//! SSG engine (`core::ssg`). The PRD keeps risk management as an optional overlay that never weighs
//! down the pure SSG calc: nothing here is imported by `core::ssg`, and the SSG method fingerprint /
//! golden corpus / determinism gates are unaffected by this module.
//!
//! Story 4.5 (FR42) opens it with the **trailing-stop** primitive: a stop level that **ratchets up
//! only**; Story 6.3 (FR39) adds the [`ledger`] weighted-average cost-basis derivation; Story 6.5
//! (FR28) adds the [`fx`] consolidation-only conversion primitive. All math is exact [`Decimal`]
//! — never `f64`.

use rust_decimal::Decimal;

mod fx;
mod ledger;

pub use fx::convert;
pub use ledger::{
    derive_position, net_dividend_cash, LedgerError, LedgerEvent, LedgerEventKind, PositionBasis,
};

/// The ratcheted trailing-stop **level** (a price) after observing `reference_price` (Story 4.5,
/// FR42). The candidate level is `reference_price × (1 − pct/100)`; the returned level is the
/// **maximum** of the prior level and that candidate, so it **ratchets up only** — a falling price
/// (or a looser `pct`) never lowers it. With no prior level (`None`), it seeds from the candidate.
///
/// `pct` is the trailing-stop percentage (e.g. `15` for 15 %), assumed already validated to `(0,
/// 100)` by the caller. Exact decimal throughout, all arithmetic checked — no panic on any input:
/// should the candidate computation overflow `Decimal` (only reachable on inputs outside the
/// validated range), the prior level is kept (the ratchet holds), or `Decimal::ZERO` is returned
/// when there is no prior level (a floor no positive price breaches).
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
    let candidate = pct
        .checked_div(Decimal::ONE_HUNDRED)
        .and_then(|fraction| Decimal::ONE.checked_sub(fraction))
        .and_then(|factor| reference_price.checked_mul(factor));
    match (prior_level, candidate) {
        (Some(prior), Some(candidate)) => prior.max(candidate),
        // Overflowed candidate: the level holds (ratchet-up only, see the doc note).
        (Some(prior), None) => prior,
        (None, Some(candidate)) => candidate,
        (None, None) => Decimal::ZERO,
    }
}

/// Whether the current price has reached or fallen through the trailing-stop level (Story 4.5) — a
/// pure **state**, surfaced as a neutral fact (the user arbitrates; the app never acts). `true` when
/// `current_price ≤ stop_level`.
pub fn stop_breached(stop_level: Decimal, current_price: Decimal) -> bool {
    current_price <= stop_level
}

/// One position's inputs to the portfolio risk sums (Story 4.6, FR43): its average cost, its
/// trailing-stop level (`None` when no stop is set), and its quantity. All exact [`Decimal`].
#[derive(Debug, Clone, Copy)]
pub struct PositionRisk {
    /// Average acquisition cost per unit.
    pub avg_cost: Decimal,
    /// The trailing-stop level; `None` when no stop is set.
    pub stop: Option<Decimal>,
    /// Held quantity.
    pub quantity: Decimal,
}

/// The portfolio's **capital-at-risk** (Story 4.6, FR43 — the Appendix-A formula):
/// `Σ (avg_cost − stop) × quantity` over positions whose stop is **set** and **≤ avg_cost**. A
/// position with no stop, or whose stop has ratcheted **above** cost, contributes **0** (its
/// capital-loss risk is gone). `≥ 0` by construction (the `stop ≤ avg_cost` guard means every summed
/// term is non-negative). Single-currency — the caller sums one reference currency (no FX, Epic 4).
///
/// All arithmetic is saturating — no panic on any input: an out-of-`Decimal`-range term or sum
/// clamps at the `Decimal` bounds instead of overflowing (unreachable for realistic portfolios;
/// defense in depth only).
pub fn capital_at_risk(positions: &[PositionRisk]) -> Decimal {
    positions
        .iter()
        .filter_map(|p| {
            let stop = p.stop?;
            (stop <= p.avg_cost).then(|| p.avg_cost.saturating_sub(stop).saturating_mul(p.quantity))
        })
        .fold(Decimal::ZERO, |acc, term| acc.saturating_add(term))
}

/// Total invested capital `Σ avg_cost × quantity` (Story 4.6) — the denominator for capital-at-risk
/// as a percentage. `0` for an empty portfolio (the caller omits the percent then). Saturating
/// arithmetic, like [`capital_at_risk`] — an out-of-range term or sum clamps at the `Decimal`
/// bounds instead of panicking.
pub fn total_invested(positions: &[PositionRisk]) -> Decimal {
    positions
        .iter()
        .map(|p| p.avg_cost.saturating_mul(p.quantity))
        .fold(Decimal::ZERO, |acc, term| acc.saturating_add(term))
}

/// Which neutral trigger a holding fires (Story 4.7, FR46/FR47). The app surfaces the fact and
/// offers manual actions — it **never acts on its own**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
    /// The current price has reached or fallen through the trailing stop (Story 4.5).
    Stop,
    /// The matched study's present price is in its Sell zone (Story 4.4 / `core::ssg` §4).
    Sell,
}

/// The neutral trigger state for one holding (Story 4.7, FR46/FR47): given whether its trailing
/// stop is breached and whether it is in its Sell zone, return which trigger (if any) fires. **The
/// stop takes priority over the Sell zone** — when both hold, the result is [`TriggerKind::Stop`].
/// This is the **isolated, testable FR47 business rule** (stop-loss priority); keeping it a pure
/// function of the two booleans (→ `Option<TriggerKind>`) means the priority never leaks into UI
/// conditionals. Neither condition → `None`.
pub fn trigger_state(stop_breached: bool, in_sell_zone: bool) -> Option<TriggerKind> {
    if stop_breached {
        Some(TriggerKind::Stop) // FR47: the stop wins, even when also in the Sell zone.
    } else if in_sell_zone {
        Some(TriggerKind::Sell)
    } else {
        None
    }
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

    fn pos(cost: i64, stop: Option<i64>, qty: i64) -> PositionRisk {
        PositionRisk {
            avg_cost: d(cost),
            stop: stop.map(d),
            quantity: d(qty),
        }
    }

    #[test]
    fn capital_at_risk_sums_only_below_cost_stops() {
        let portfolio = [
            pos(100, Some(85), 10), // stop below cost → (100−85)×10 = 150
            pos(50, Some(60), 20),  // stop ABOVE cost → 0 (risk gone)
            pos(30, None, 100),     // no stop → 0
            pos(40, Some(40), 5),   // stop == cost → (40−40)×5 = 0
        ];
        assert_eq!(capital_at_risk(&portfolio), d(150));
    }

    #[test]
    fn capital_at_risk_is_non_negative_and_zero_when_empty_or_all_protected() {
        assert_eq!(capital_at_risk(&[]), d(0));
        assert_eq!(
            capital_at_risk(&[pos(10, Some(20), 5), pos(10, None, 5)]),
            d(0),
            "all stops above cost (or absent) → 0, never negative"
        );
    }

    #[test]
    fn total_invested_sums_cost_times_quantity() {
        assert_eq!(
            total_invested(&[pos(100, Some(85), 10), pos(50, None, 4)]),
            d(1200) // 100×10 + 50×4
        );
        assert_eq!(total_invested(&[]), d(0));
    }

    #[test]
    fn capital_at_risk_preserves_exact_decimal_scale() {
        // (100.50 − 90.25) × 3 = 30.75 — exact.
        let p = PositionRisk {
            avg_cost: Decimal::from_str_exact("100.50").unwrap(),
            stop: Some(Decimal::from_str_exact("90.25").unwrap()),
            quantity: d(3),
        };
        assert_eq!(
            capital_at_risk(&[p]),
            Decimal::from_str_exact("30.75").unwrap()
        );
    }

    // ── Story 4.7 — the neutral trigger state + the FR47 stop-priority rule ──

    #[test]
    fn no_trigger_when_neither_breached_nor_in_sell_zone() {
        assert_eq!(trigger_state(false, false), None);
    }

    #[test]
    fn a_breached_stop_fires_the_stop_trigger() {
        assert_eq!(trigger_state(true, false), Some(TriggerKind::Stop));
    }

    #[test]
    fn the_sell_zone_alone_fires_the_sell_trigger() {
        assert_eq!(trigger_state(false, true), Some(TriggerKind::Sell));
    }

    #[test]
    fn stop_priority_over_sell_zone() {
        // FR47 — when both conditions hold, the stop-loss takes priority over the Sell zone.
        assert_eq!(trigger_state(true, true), Some(TriggerKind::Stop));
    }
}
