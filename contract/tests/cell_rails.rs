//! Property tests of the two `Cell` mutation rails — `edited` (Story 1.11, invariant 2b) and
//! `reconcile` (Story 3.4, FR22 / NFR-R4) — over every representable cell state. Everything
//! exercised here is public API; the example-based rail tests live with the type in
//! `contract/src/cell.rs`.

use proptest::prelude::*;
use rust_decimal::Decimal;
use steadyinvest_contract::{
    AiOrigin, Cell, Coverage, Freshness, Money, Provenance, Review, Source, Timestamp,
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
                ai_origin: None,
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
        ai_origin: None,
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

fn ai_origin() -> AiOrigin {
    AiOrigin {
        draft_id: uuid::Uuid::from_u128(42),
        client: "claude-code".to_string(),
        model: "claude-opus-5-5".to_string(),
        validated_at: Timestamp("2026-09-27T12:00:00Z".to_string()),
    }
}

fn ai_provenance() -> Provenance {
    Provenance {
        source: Source::Manual,
        logical_version: 9,
        timestamp: Timestamp("2026-09-27T12:00:00Z".to_string()),
        hash_of_dependencies: "cc22".to_string(),
        ai_origin: Some(ai_origin()),
    }
}

proptest! {
    /// Story 8.2b (O5 / D5): the AI-draft validation rail always lands on `?`, whatever the
    /// cell's review, value or pending state — the documented exception to invariant 2b — and
    /// otherwise behaves exactly like the manual edit rail.
    #[test]
    fn validation_from_a_draft_always_lands_on_to_review(
        original in cell(),
        new_value in value(),
    ) {
        let before = original.clone();
        let validated = original.validated_from_draft(new_value, ai_provenance());
        prop_assert_eq!(validated.review, Review::ToReview);
        let edited = before.edited(new_value, ai_provenance());
        prop_assert_eq!(&Cell { review: Review::ToReview, ..edited }, &validated);
        prop_assert_eq!(validated.source, Source::Manual);
        prop_assert_eq!(validated.pending, None);
        prop_assert_eq!(validated.provenance.ai_origin, Some(ai_origin()));
        prop_assert_eq!(original, before, "snapshot semantics");
    }
}

fn money(s: &str) -> Money {
    Money::from(Decimal::from_str_exact(s).unwrap())
}

fn plain(review: Review, v: &str) -> Cell {
    Cell {
        value: Some(money(v)),
        source: Source::Provider,
        freshness: Freshness::Current,
        review,
        coverage: Coverage::Present,
        provenance: Provenance {
            source: Source::Provider,
            logical_version: 1,
            timestamp: Timestamp("2026-09-01T00:00:00Z".to_string()),
            hash_of_dependencies: "aa".to_string(),
            ai_origin: None,
        },
        pending: None,
    }
}

#[test]
fn draft_validation_on_an_untagged_cell_is_to_review() {
    let c = plain(Review::None, "3").validated_from_draft(Some(money("4")), ai_provenance());
    assert_eq!(c.review, Review::ToReview);
    assert_eq!(c.coverage, Coverage::Present);
}

#[test]
fn draft_validation_on_a_validated_cell_with_the_same_value_is_to_review() {
    // The manual rail would keep ✓ here; the draft rail never does (D5).
    let c = plain(Review::Validated, "3").validated_from_draft(Some(money("3.0")), ai_provenance());
    assert_eq!(c.review, Review::ToReview);
}

#[test]
fn draft_validation_on_a_validated_cell_with_a_new_value_is_to_review() {
    let c = plain(Review::Validated, "3").validated_from_draft(Some(money("5")), ai_provenance());
    assert_eq!(c.review, Review::ToReview);
    assert_eq!(c.value, Some(money("5")));
}

#[test]
fn draft_validation_clears_a_pending_divergence() {
    let mut before = plain(Review::None, "3");
    before.pending = Some(steadyinvest_contract::PendingProvider {
        value: Some(money("9")),
        provenance: before.provenance.clone(),
    });
    let c = before.validated_from_draft(Some(money("4")), ai_provenance());
    assert_eq!(c.pending, None);
}

/// T4.3 — reconciliation of an AI-origin manual cell: manual wins, a divergent provider value is
/// parked, and the AI origin survives; an agreeing fetch clears the pending and keeps the origin.
#[test]
fn an_ai_origin_value_reconciles_as_a_manual_value() {
    let ai = plain(Review::None, "3").validated_from_draft(Some(money("4")), ai_provenance());
    let fetch = plain(Review::None, "0").provenance;
    let divergent = ai.reconcile(Some(money("5")), fetch.clone());
    assert_eq!(divergent.value, Some(money("4")), "manual wins");
    assert_eq!(divergent.source, Source::Manual);
    assert_eq!(divergent.pending.as_ref().unwrap().value, Some(money("5")));
    assert_eq!(divergent.provenance.ai_origin, Some(ai_origin()));
    let agreeing = divergent.reconcile(Some(money("4.00")), fetch);
    assert_eq!(agreeing.pending, None);
    assert_eq!(agreeing.provenance.ai_origin, Some(ai_origin()));
    assert_eq!(agreeing.value, Some(money("4")));
}

/// A review-only toggle keeps the provenance verbatim — the mark survives it; the owner's next
/// value edit replaces the provenance, so the mark is cleared.
#[test]
fn the_owner_s_next_edit_clears_the_ai_origin() {
    let ai = plain(Review::None, "3").validated_from_draft(Some(money("4")), ai_provenance());
    let toggled = Cell {
        review: Review::Validated,
        ..ai.clone()
    };
    assert_eq!(toggled.provenance.ai_origin, Some(ai_origin()));
    let owner = Provenance {
        ai_origin: None,
        ..ai_provenance()
    };
    assert_eq!(
        ai.edited(Some(money("6")), owner).provenance.ai_origin,
        None
    );
}
