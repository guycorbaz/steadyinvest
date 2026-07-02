//! Weighted-average cost basis over a buy/sell ledger (Story 6.3, FR39 / PRD Appendix A).
//!
//! Pure, IO-free position derivation: fold an ordered list of [`LedgerEvent`]s (buys and sells,
//! oldest first — the caller supplies them in `occurred_at, id` order) into a [`PositionBasis`]
//! `(quantity, avg_cost)`. The Appendix-A rule, **fees included**:
//!
//! - a **buy** of `q` at `p` with fees `f` re-averages the cost:
//!   `avg_cost' = (quantity × avg_cost + q × p + f) / (quantity + q)`;
//! - a **sell** reduces `quantity` only — the weighted-average cost of what remains is unchanged
//!   (sell fees affect realized proceeds, not the basis of the remainder).
//!
//! Like the rest of `core::risk`, this is a decoupled overlay: nothing here is imported by
//! `core::ssg`, so the method fingerprint / golden corpus / determinism gates are unaffected.
//! All arithmetic is exact [`Decimal`] and **checked** — no panic on any input; an impossible
//! history (an over-sell, a negative amount) is a typed [`LedgerError`], never a wrong number.

use rust_decimal::Decimal;

/// What a ledger row does to the position (mirrors the persistence `kind` column: `"buy"`/`"sell"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerEventKind {
    /// Adds `quantity` at `unit_price` (+ `fees`) and re-averages the cost basis.
    Buy,
    /// Removes `quantity`; the remaining basis is unchanged.
    Sell,
}

/// One buy/sell event, already parsed to exact decimals by the caller (the app validates and
/// parses the canonical TEXT spellings; nothing here touches strings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerEvent {
    /// Buy or sell.
    pub kind: LedgerEventKind,
    /// The transacted quantity — must be strictly positive.
    pub quantity: Decimal,
    /// The per-unit price — must be non-negative. Ignored for the basis on a sell.
    pub unit_price: Decimal,
    /// Transaction fees — must be non-negative. Folded into the basis on a buy (Appendix A);
    /// ignored for the basis on a sell.
    pub fees: Decimal,
}

/// A derived position: how much is held and at what weighted-average cost (fees included).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionBasis {
    /// The held quantity after replaying the ledger (`0` = the position is closed).
    pub quantity: Decimal,
    /// The weighted-average unit cost of the held quantity. When the position closes, the last
    /// basis is kept (informational — there is nothing left to cost).
    pub avg_cost: Decimal,
}

/// Why a ledger could not be replayed into a position. Neutral, fact-stating messages (FR13) —
/// the app maps them to its own user-facing copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerError {
    /// An event's quantity is zero or negative — not a recordable transaction.
    NonPositiveQuantity,
    /// A price, fee or opening value is negative.
    NegativeAmount,
    /// A sell exceeds the quantity held at that point in the history — the position would go
    /// negative, which weighted-average cost cannot represent.
    OverSell,
    /// A checked decimal operation overflowed (values beyond `Decimal` range).
    Overflow,
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            LedgerError::NonPositiveQuantity => "a transaction quantity is zero or negative",
            LedgerError::NegativeAmount => "a price, fee or opening value is negative",
            LedgerError::OverSell => {
                "a transaction quantity exceeds the quantity held at that point in the history"
            }
            LedgerError::Overflow => "a decimal computation exceeded the representable range",
        };
        f.write_str(text)
    }
}

impl std::error::Error for LedgerError {}

/// Replay a holding's ledger into its current [`PositionBasis`] (Story 6.3, FR39).
///
/// `opening` is the position **before** the first event (`None` = an empty position — the normal
/// case once the opening buy row is materialized in the ledger itself); `events` are the buy/sell
/// rows **oldest first** (the persistence read order, `occurred_at` then `id`). Deterministic:
/// same inputs, same output, no ambient state. Checked arithmetic throughout — invalid inputs and
/// impossible histories return a typed [`LedgerError`], never a panic or a negative position.
pub fn derive_position(
    opening: Option<PositionBasis>,
    events: &[LedgerEvent],
) -> Result<PositionBasis, LedgerError> {
    let mut position = opening.unwrap_or(PositionBasis {
        quantity: Decimal::ZERO,
        avg_cost: Decimal::ZERO,
    });
    if position.quantity.is_sign_negative() || position.avg_cost.is_sign_negative() {
        return Err(LedgerError::NegativeAmount);
    }
    for event in events {
        if !event.quantity.is_sign_positive() || event.quantity.is_zero() {
            return Err(LedgerError::NonPositiveQuantity);
        }
        if event.unit_price.is_sign_negative() || event.fees.is_sign_negative() {
            return Err(LedgerError::NegativeAmount);
        }
        match event.kind {
            LedgerEventKind::Buy => {
                // avg' = (held × avg + q × p + fees) / (held + q), every step checked.
                let held_cost = position
                    .quantity
                    .checked_mul(position.avg_cost)
                    .ok_or(LedgerError::Overflow)?;
                let event_cost = event
                    .quantity
                    .checked_mul(event.unit_price)
                    .and_then(|c| c.checked_add(event.fees))
                    .ok_or(LedgerError::Overflow)?;
                let total_cost = held_cost
                    .checked_add(event_cost)
                    .ok_or(LedgerError::Overflow)?;
                let total_quantity = position
                    .quantity
                    .checked_add(event.quantity)
                    .ok_or(LedgerError::Overflow)?;
                // total_quantity > 0: position.quantity ≥ 0 and event.quantity > 0.
                position.avg_cost = total_cost
                    .checked_div(total_quantity)
                    .ok_or(LedgerError::Overflow)?;
                position.quantity = total_quantity;
            }
            LedgerEventKind::Sell => {
                if event.quantity > position.quantity {
                    return Err(LedgerError::OverSell);
                }
                // Subtraction of a smaller-or-equal positive quantity cannot overflow.
                position.quantity -= event.quantity;
                // avg_cost unchanged: a sell removes units at the running average; the basis of
                // what remains is the same weighted average (Appendix A).
            }
        }
    }
    Ok(position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    fn buy(q: &str, p: &str, f: &str) -> LedgerEvent {
        LedgerEvent {
            kind: LedgerEventKind::Buy,
            quantity: dec(q),
            unit_price: dec(p),
            fees: dec(f),
        }
    }

    fn sell(q: &str) -> LedgerEvent {
        LedgerEvent {
            kind: LedgerEventKind::Sell,
            quantity: dec(q),
            unit_price: dec("0"),
            fees: dec("0"),
        }
    }

    #[test]
    fn a_single_buy_is_its_own_basis_with_fees_folded_in() {
        // 10 @ 100 with 5 fees → basis 100.5 (Appendix A: fees INCLUDED).
        let p = derive_position(None, &[buy("10", "100", "5")]).unwrap();
        assert_eq!(p.quantity, dec("10"));
        assert_eq!(p.avg_cost, dec("100.5"));
    }

    #[test]
    fn two_buys_re_average_per_appendix_a() {
        // 10 @ 100 (fees 0) then 10 @ 200 (fees 0) → 20 @ 150.
        let p = derive_position(None, &[buy("10", "100", "0"), buy("10", "200", "0")]).unwrap();
        assert_eq!(p.quantity, dec("20"));
        assert_eq!(p.avg_cost, dec("150"));
    }

    #[test]
    fn a_partial_sell_reduces_quantity_and_keeps_the_average() {
        let p = derive_position(
            None,
            &[buy("10", "100", "0"), buy("10", "200", "0"), sell("5")],
        )
        .unwrap();
        assert_eq!(p.quantity, dec("15"));
        assert_eq!(p.avg_cost, dec("150"), "a sell never re-averages");
    }

    #[test]
    fn a_sell_to_zero_closes_the_position_and_keeps_the_last_basis() {
        let p = derive_position(None, &[buy("10", "100", "2"), sell("10")]).unwrap();
        assert_eq!(p.quantity, Decimal::ZERO);
        assert_eq!(
            p.avg_cost,
            dec("100.2"),
            "informational last basis survives"
        );
    }

    #[test]
    fn an_over_sell_is_a_typed_error_never_a_negative_position() {
        assert_eq!(
            derive_position(None, &[buy("10", "100", "0"), sell("11")]),
            Err(LedgerError::OverSell)
        );
        // Selling from an empty ledger is the same impossibility.
        assert_eq!(
            derive_position(None, &[sell("1")]),
            Err(LedgerError::OverSell)
        );
    }

    #[test]
    fn an_opening_position_seeds_the_replay() {
        // The pre-6.3 holding (10 @ 95) then a buy re-averages against the opening.
        let opening = PositionBasis {
            quantity: dec("10"),
            avg_cost: dec("95"),
        };
        let p = derive_position(Some(opening), &[buy("10", "105", "0")]).unwrap();
        assert_eq!(p.quantity, dec("20"));
        assert_eq!(p.avg_cost, dec("100"));
    }

    #[test]
    fn invalid_inputs_are_typed_errors() {
        assert_eq!(
            derive_position(None, &[buy("0", "100", "0")]),
            Err(LedgerError::NonPositiveQuantity)
        );
        assert_eq!(
            derive_position(None, &[buy("-1", "100", "0")]),
            Err(LedgerError::NonPositiveQuantity)
        );
        assert_eq!(
            derive_position(None, &[buy("1", "-100", "0")]),
            Err(LedgerError::NegativeAmount)
        );
        assert_eq!(
            derive_position(None, &[buy("1", "100", "-0.01")]),
            Err(LedgerError::NegativeAmount)
        );
        let negative_opening = PositionBasis {
            quantity: dec("-1"),
            avg_cost: dec("0"),
        };
        assert_eq!(
            derive_position(Some(negative_opening), &[]),
            Err(LedgerError::NegativeAmount)
        );
    }

    #[test]
    fn extreme_magnitudes_overflow_as_a_typed_error_not_a_panic() {
        let p = derive_position(None, &[buy("79228162514264337593543950335", "2", "0")]);
        assert_eq!(p, Err(LedgerError::Overflow), "q × p beyond Decimal range");
    }

    proptest! {
        /// The derivation is total (never panics), deterministic, and any Ok position is
        /// non-negative in quantity — for arbitrary small-magnitude decimal event streams.
        #[test]
        fn derivation_is_total_deterministic_and_never_negative(
            events in proptest::collection::vec(
                (any::<bool>(), 1u64..10_000, 0u64..1_000_000, 0u64..10_000),
                0..12,
            )
        ) {
            let events: Vec<LedgerEvent> = events
                .into_iter()
                .map(|(is_buy, q, p, f)| LedgerEvent {
                    kind: if is_buy { LedgerEventKind::Buy } else { LedgerEventKind::Sell },
                    // Scale to fractional spellings so division exercises non-integers.
                    quantity: Decimal::new(q as i64, 2),
                    unit_price: Decimal::new(p as i64, 2),
                    fees: Decimal::new(f as i64, 2),
                })
                .collect();
            let a = derive_position(None, &events);
            let b = derive_position(None, &events);
            prop_assert_eq!(a, b, "deterministic");
            if let Ok(p) = a {
                prop_assert!(!p.quantity.is_sign_negative(), "never a negative position");
                prop_assert!(!p.avg_cost.is_sign_negative(), "never a negative basis");
            }
        }
    }
}
