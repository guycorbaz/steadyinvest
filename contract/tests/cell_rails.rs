//! Property tests of the two `Cell` mutation rails — `edited` (Story 1.11, invariant 2b) and
//! `reconcile` (Story 3.4, FR22 / NFR-R4) — over every representable cell state. Everything
//! exercised here is public API; the example-based rail tests live with the type in
//! `contract/src/cell.rs`.

use proptest::prelude::*;
use rust_decimal::Decimal;
use steadyinvest_contract::{
    Cell, Coverage, Freshness, Money, Provenance, Review, Source, Timestamp,
};

fn source() -> impl Strategy<Value = Source> {
    prop_oneof![
        Just(Source::Provider),
        Just(Source::Manual),
        Just(Source::Derived)
    ]
}
fn freshness() -> impl Strategy<Value = Freshness> {
    prop_oneof![Just(Freshness::Current), Just(Freshness::Stale)]
}
fn review() -> impl Strategy<Value = Review> {
    prop_oneof![
        Just(Review::None),
        Just(Review::ToReview),
        Just(Review::Validated)
    ]
}
fn coverage() -> impl Strategy<Value = Coverage> {
    prop_oneof![
        Just(Coverage::Present),
        Just(Coverage::ToFill),
        Just(Coverage::NotAvailableAccepted),
    ]
}
/// Optional Money over a deliberately small value space (collisions WANTED, to exercise
/// the equal-value branch) with varying scale (value-equality across scales).
fn value() -> impl Strategy<Value = Option<Money>> {
    proptest::option::of((0..50i64, 0..3u32).prop_map(|(mantissa, extra_zeros)| {
        Money::from(Decimal::new(mantissa * 10i64.pow(extra_zeros), extra_zeros))
    }))
}
fn cell() -> impl Strategy<Value = Cell> {
    (value(), source(), freshness(), review(), coverage()).prop_map(
        |(value, source, freshness, review, coverage)| Cell {
            value,
            source,
            freshness,
            review,
            coverage,
            provenance: Provenance {
                source,
                logical_version: 1,
                timestamp: Timestamp("2026-06-09T00:00:00Z".to_string()),
                hash_of_dependencies: "aa00".to_string(),
            },
            pending: None,
        },
    )
}
fn edit_provenance() -> impl Strategy<Value = Provenance> {
    (source(), 1..100u64).prop_map(|(source, logical_version)| Provenance {
        source,
        logical_version,
        timestamp: Timestamp("2026-06-12T08:00:00Z".to_string()),
        hash_of_dependencies: "bb11".to_string(),
    })
}

proptest! {
    /// Invariant 2b on the rail, for EVERY cell state: a divergent edit always demotes ✓,
    /// an equal-value edit never does, no edit ever promotes, and the rail never
    /// half-applies (freshness/source/coverage/provenance always follow the semantics).
    #[test]
    fn edit_rail_semantics_hold_for_every_cell_state(
        original in cell(),
        new_value in value(),
        provenance in edit_provenance(),
    ) {
        let before = original.clone();
        let edited = original.edited(new_value, provenance.clone());

        let diverges = before.value != new_value;
        let expected_review = match before.review {
            Review::Validated if diverges => Review::ToReview,
            unchanged => unchanged,
        };
        prop_assert_eq!(edited.review, expected_review,
            "✓ demotes iff the value diverges; None/ToReview never move");
        prop_assert_eq!(edited.value, new_value);
        prop_assert_eq!(edited.freshness, Freshness::Current, "a fresh edit is current");
        prop_assert_eq!(edited.source, provenance.source);
        prop_assert_eq!(
            edited.coverage,
            if new_value.is_some() { Coverage::Present } else { Coverage::ToFill }
        );
        prop_assert_eq!(edited.provenance, provenance, "provenance replaced verbatim");
        prop_assert_eq!(original, before, "snapshot semantics: the original is untouched");
    }

    /// The Story-3.4 reconcile rail, for EVERY cell state: the LIVE value/source/coverage/
    /// freshness are NEVER touched (manual wins); a divergence stores a pending and demotes ✓
    /// (only ✓), an agreement clears any pending and never moves the review.
    #[test]
    fn reconcile_rail_never_touches_the_live_value_and_only_pends_on_divergence(
        original in cell(),
        fetched in value(),
        provenance in edit_provenance(),
    ) {
        let before = original.clone();
        let reconciled = original.reconcile(fetched, provenance.clone());

        // The live value and its attributes are inviolate — reconciliation is non-destructive.
        prop_assert_eq!(reconciled.value, before.value, "manual value never overwritten");
        prop_assert_eq!(reconciled.source, before.source);
        prop_assert_eq!(reconciled.coverage, before.coverage);
        prop_assert_eq!(reconciled.freshness, before.freshness);

        let diverges = before.value != fetched;
        if diverges {
            let p = reconciled.pending.expect("a divergence stores a pending");
            prop_assert_eq!(p.value, fetched);
            prop_assert_eq!(p.provenance, provenance);
            let expected = match before.review {
                Review::Validated => Review::ToReview,
                unchanged => unchanged,
            };
            prop_assert_eq!(reconciled.review, expected, "✓ demotes on divergence; others don't move");
        } else {
            prop_assert_eq!(reconciled.pending, None, "agreement clears any pending");
            prop_assert_eq!(reconciled.review, before.review, "agreement never moves the review");
        }
        prop_assert_eq!(original, before, "snapshot semantics: the original is untouched");
    }
}
