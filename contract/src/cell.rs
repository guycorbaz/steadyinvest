//! The per-cell data-state model (FR17–FR20): every data point carries, independently queryable,
//! its **source** × **freshness** × **review** × **coverage**, plus its [`Provenance`].
//!
//! A missing value is `value: None` — a genuine gap, **never coerced to 0** (`unknown/insufficient`
//! is first-class).

use crate::money::Money;
use crate::provenance::Provenance;
use serde::{Deserialize, Serialize};

/// Where a cell's value came from (FR17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Provider,
    Manual,
    Derived,
}

/// Freshness of a cell's value (FR23).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Current,
    Stale,
}

/// User-set review tag (FR20) — tri-state, NEVER `0/1/2`. `none` → `to_review` (?) → `validated` (✓).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Review {
    None,
    ToReview,
    Validated,
}

/// Per-cell coverage state (FR19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    Present,
    ToFill,
    NotAvailableAccepted,
}

/// One data cell: an optional exact value plus its full, independently-queryable data state and
/// provenance. Forward-compatible: `value` defaults to `None` if absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    #[serde(default)]
    pub value: Option<Money>,
    pub source: Source,
    pub freshness: Freshness,
    pub review: Review,
    pub coverage: Coverage,
    pub provenance: Provenance,
}

impl Cell {
    /// The manual-mutation rail (Story 1.11, invariant 2b) — returns a NEW cell, snapshot
    /// semantics, never in-place. Epic 2's manual edits and Epic 3's provider reconciliation
    /// both go through this one primitive; it is load-bearing-agnostic (which fields gate the
    /// verdict is method knowledge, owned by `core`).
    ///
    /// Semantics (normative):
    /// - `value` ← `new_value`; `source` ← `provenance.source`; `freshness` ← [`Freshness::Current`]
    ///   (a fresh edit is current); `provenance` ← the caller-supplied one verbatim (`contract`
    ///   never calls a clock, UUID or hash — the ADD15 injected discipline);
    /// - `review`: [`Review::Validated`] → [`Review::ToReview`] **iff the value actually
    ///   differs** — [`Money`] equality is value-based, so re-entering `"3"` over `"3.0"` is NOT
    ///   a divergence and keeps ✓ (the Epic 3 reconcile rule: divergence → auto-?). A
    ///   non-divergent edit keeps `Validated` (provenance still updates). `None`/`ToReview` are
    ///   never promoted nor demoted — validation is an explicit user act (FR20);
    /// - `coverage`: a `Some` value ⇒ [`Coverage::Present`]; `None` ⇒ [`Coverage::ToFill`] (an
    ///   explicit clear reopens the gap — recorded interpretation).
    pub fn edited(&self, new_value: Option<Money>, provenance: Provenance) -> Cell {
        let diverges = self.value != new_value;
        let review = match self.review {
            Review::Validated if diverges => Review::ToReview,
            unchanged => unchanged,
        };
        let coverage = if new_value.is_some() {
            Coverage::Present
        } else {
            Coverage::ToFill
        };
        Cell {
            value: new_value,
            source: provenance.source,
            freshness: Freshness::Current,
            review,
            coverage,
            provenance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provenance::{Provenance, Timestamp};
    use rust_decimal::Decimal;

    fn sample_provenance() -> Provenance {
        Provenance {
            source: Source::Manual,
            logical_version: 1,
            timestamp: Timestamp("2026-06-09T00:00:00Z".to_string()),
            hash_of_dependencies: "deadbeef".to_string(),
        }
    }

    #[test]
    fn enum_wire_formats_are_snake_case() {
        assert_eq!(
            serde_json::to_string(&Source::Provider).unwrap(),
            "\"provider\""
        );
        assert_eq!(
            serde_json::to_string(&Freshness::Stale).unwrap(),
            "\"stale\""
        );
        assert_eq!(
            serde_json::to_string(&Review::ToReview).unwrap(),
            "\"to_review\""
        );
        assert_eq!(
            serde_json::to_string(&Coverage::NotAvailableAccepted).unwrap(),
            "\"not_available_accepted\""
        );
    }

    #[test]
    fn cell_value_defaults_to_none_when_absent() {
        // Forward-compat: a cell JSON missing "value" deserializes to None (serde default).
        let json = r#"{"source":"manual","freshness":"current","review":"none",
            "coverage":"to_fill","provenance":{"source":"manual","logical_version":1,
            "timestamp":"2026-06-09T00:00:00Z","hash_of_dependencies":"x"}}"#;
        let cell: Cell = serde_json::from_str(json).unwrap();
        assert_eq!(cell.value, None);
    }

    #[test]
    fn unknown_extra_field_is_tolerated() {
        // Forward-compat: NO deny_unknown_fields — an older build tolerates a newer file's field.
        let json = r#"{"value":"12.34","source":"provider","freshness":"current",
            "review":"validated","coverage":"present","provenance":{"source":"provider",
            "logical_version":2,"timestamp":"2026-06-09T00:00:00Z","hash_of_dependencies":"h"},
            "future_field_from_a_newer_build":42}"#;
        let cell: Cell = serde_json::from_str(json).unwrap();
        assert_eq!(cell.value, Some(Money::from(Decimal::new(1234, 2))));
        assert_eq!(cell.review, Review::Validated);
    }

    #[test]
    fn cell_round_trips() {
        let cell = Cell {
            value: Some(Money::from(Decimal::new(-7, 2))), // -0.07
            source: Source::Derived,
            freshness: Freshness::Stale,
            review: Review::ToReview,
            coverage: Coverage::Present,
            provenance: sample_provenance(),
        };
        let json = serde_json::to_string(&cell).unwrap();
        assert_eq!(serde_json::from_str::<Cell>(&json).unwrap(), cell);
    }

    // ── The Story-1.11 edit rail (invariant 2b) ──

    fn money(s: &str) -> Money {
        Money::from(Decimal::from_str_exact(s).expect("test literal must parse"))
    }

    fn edit_provenance() -> Provenance {
        Provenance {
            source: Source::Manual,
            logical_version: 7,
            timestamp: Timestamp("2026-06-12T08:00:00Z".to_string()),
            hash_of_dependencies: "cafe0011".to_string(),
        }
    }

    fn validated_cell(value: &str) -> Cell {
        Cell {
            value: Some(money(value)),
            source: Source::Provider,
            freshness: Freshness::Stale,
            review: Review::Validated,
            coverage: Coverage::Present,
            provenance: sample_provenance(),
        }
    }

    #[test]
    fn divergent_edit_demotes_validated_to_to_review_in_the_same_frame() {
        let original = validated_cell("3");
        let edited = original.edited(Some(money("4")), edit_provenance());
        assert_eq!(
            edited.review,
            Review::ToReview,
            "invariant 2b violated: a divergent edit of a ✓ cell must read ? in the new frame"
        );
        assert_eq!(edited.value, Some(money("4")));
        assert_eq!(
            edited.freshness,
            Freshness::Current,
            "a fresh edit is current"
        );
        assert_eq!(
            edited.source,
            Source::Manual,
            "source follows provenance.source"
        );
        assert_eq!(edited.coverage, Coverage::Present);
        assert_eq!(
            edited.provenance,
            edit_provenance(),
            "provenance replaced verbatim"
        );
        // Snapshot semantics: the original frame is untouched.
        assert_eq!(original, validated_cell("3"));
    }

    #[test]
    fn equal_value_edit_keeps_validated_even_across_scales() {
        // Money equality is value-based: re-entering "3.0" over "3" is NOT a divergence.
        let original = validated_cell("3");
        let edited = original.edited(Some(money("3.0")), edit_provenance());
        assert_eq!(
            edited.review,
            Review::Validated,
            "a non-divergent edit must keep ✓ (Money(\"3.0\") == Money(\"3\"))"
        );
        // Everything else still updates.
        assert_eq!(edited.provenance, edit_provenance());
        assert_eq!(edited.freshness, Freshness::Current);
        assert_eq!(edited.source, Source::Manual);
    }

    #[test]
    fn edit_never_promotes_review_and_clearing_reopens_the_gap() {
        // Validation is an explicit user act (FR20): an edit never promotes None/ToReview.
        for review in [Review::None, Review::ToReview] {
            let cell = Cell {
                review,
                ..validated_cell("3")
            };
            let divergent = cell.edited(Some(money("9")), edit_provenance());
            assert_eq!(
                divergent.review, review,
                "an edit must never change a non-✓ review"
            );
            let equal = cell.edited(Some(money("3")), edit_provenance());
            assert_eq!(equal.review, review);
        }
        // An explicit clear reopens the gap (recorded interpretation) — and None ≠ Some
        // diverges, so a ✓ cell cleared reads ? in the same frame.
        let cleared = validated_cell("3").edited(None, edit_provenance());
        assert_eq!(cleared.value, None);
        assert_eq!(cleared.coverage, Coverage::ToFill);
        assert_eq!(cleared.review, Review::ToReview);
    }
}

#[cfg(test)]
mod edit_rail_properties {
    use super::*;
    use crate::provenance::{Provenance, Timestamp};
    use proptest::prelude::*;
    use rust_decimal::Decimal;

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
    }
}
