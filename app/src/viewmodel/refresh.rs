//! Refresh-cause classification (Story 3.3, FR29). When a manual refresh re-fetches provider data
//! and the engine recomputes, the app reports **why** it recomputed — a classification of what the
//! re-fetch actually changed: a yearly **price** moved, a **fundamental input** moved, or (reserved)
//! an **FX** rate moved. The recompute itself stays the single deterministic `engine::build_frame`
//! (the Cardinal Rule); the cause is a label on the diff, **never a different calculation**.
//!
//! `fx` is a declared slot for FR28 (FX acquisition, **P2** — not built); it is always `false` in
//! this story so FR29's "distinguishing the cause" is structurally honoured without speculative FX
//! machinery.

/// The yearly fields whose change is a **price** cause (FR29). The provider refresh writes only the
/// per-year grid; `current_price` is a user-set judgment input, not a refreshed cell, so it is not a
/// refresh cause. Kept here as the single source so the rail never hardcodes a parallel list.
pub const PRICE_FIELDS: &[&str] = &["high_price", "low_price"];

/// The yearly fields whose change is a **fundamental input** cause (FR29).
pub const INPUT_FIELDS: &[&str] = &[
    "sales",
    "eps",
    "pre_tax_profit",
    "book_value_per_share",
    "dividend_per_share",
];

/// Why a refresh recomputed (FR29). A pure classification of the refresh diff — bits are OR-merged
/// across every changed cell. Never carries a calculation, only the cause label the notice states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RefreshCause {
    /// A yearly high/low price moved.
    pub price: bool,
    /// A fundamental (sales / eps / pre-tax profit / book value / dividend) moved.
    pub input: bool,
    /// Reserved for FR28 (FX acquisition, P2) — always `false` in this story.
    pub fx: bool,
}

impl RefreshCause {
    /// OR-merge two causes (accumulating across changed cells).
    pub fn merge(self, other: RefreshCause) -> RefreshCause {
        RefreshCause {
            price: self.price || other.price,
            input: self.input || other.input,
            fx: self.fx || other.fx,
        }
    }
}

/// Classify a single changed yearly field into its recompute cause. An unknown field contributes no
/// cause bit (defensive — the rail only passes the seven canonical field names).
pub fn classify_field(field: &str) -> RefreshCause {
    RefreshCause {
        price: PRICE_FIELDS.contains(&field),
        input: INPUT_FIELDS.contains(&field),
        fx: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_fields_classify_as_price_only() {
        for field in PRICE_FIELDS {
            let c = classify_field(field);
            assert!(c.price && !c.input && !c.fx, "{field} is a price cause");
        }
    }

    #[test]
    fn input_fields_classify_as_input_only() {
        for field in INPUT_FIELDS {
            let c = classify_field(field);
            assert!(c.input && !c.price && !c.fx, "{field} is an input cause");
        }
    }

    #[test]
    fn unknown_field_classifies_as_nothing() {
        let c = classify_field("current_price");
        assert!(
            !c.price && !c.input && !c.fx,
            "current_price is not a refreshed cell → no cause"
        );
    }

    #[test]
    fn merge_is_an_or_over_the_bits() {
        let price = classify_field("high_price");
        let input = classify_field("sales");
        let both = price.merge(input);
        assert!(both.price && both.input && !both.fx);
        // merge is idempotent / commutative on the bits
        assert_eq!(both.merge(price), both);
        assert_eq!(input.merge(price), both);
    }
}
