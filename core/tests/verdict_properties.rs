//! Property tests for Story 1.11 — invariant 2a (`Full` ⟺ all gates green ∧ ¬low_confidence)
//! and the ADD9 content address (equal inputs ⇒ equal digest; any load-bearing change ⇒
//! different digest), driven through the ONLY construction path: [`StudySnapshot::new`].

use proptest::prelude::*;
use rust_decimal::Decimal;
use steadyinvest_core::normalize::{CanonicalFinancials, CanonicalYear, YearUsability};
use steadyinvest_core::ssg::{JudgmentInputs, QuarterlyObservations};
use steadyinvest_core::verdict::{GateState, InputGates, StudySnapshot, Verdict, YearGates};
use steadyinvest_core::METHOD_VERSION;

fn d(s: &str) -> Decimal {
    Decimal::from_str_exact(s).expect("test literal must parse")
}

/// One fully-populated usable canonical year (values vary by index so the series is plausible).
fn usable_year(year: i32, i: u32) -> CanonicalYear {
    let step = Decimal::from(i);
    CanonicalYear {
        year,
        sales: Some(d("100") + step * d("10")),
        eps: Some(d("1.00") + step * d("0.10")),
        high_price: Some(d("20") + step * d("2")),
        low_price: Some(d("10") + step),
        dividend_per_share: None,
        pre_tax_profit: None,
        book_value_per_share: None,
        usability: YearUsability::Usable,
    }
}

/// Five usable years 2021–2025 — full confidence by construction.
fn five_year_financials() -> CanonicalFinancials {
    CanonicalFinancials {
        years: (0..5).map(|i| usable_year(2021 + i as i32, i)).collect(),
        findings: vec![],
        usable_years: 5,
    }
}

/// A complete judgment: every load-bearing judgment input present.
fn complete_judgment() -> JudgmentInputs {
    JudgmentInputs {
        estimated_high_eps: Some(d("2.00")),
        estimated_low_eps: Some(d("1.50")),
        judged_avg_high_pe: Some(d("20")),
        judged_avg_low_pe: Some(d("10")),
        current_price: Some(d("25")),
        ..JudgmentInputs::empty()
    }
}

/// All-green gates matching `five_year_financials()`'s usable years.
fn green_gates() -> InputGates {
    InputGates::new(
        (2021..=2025)
            .map(|y| YearGates::new(y, [GateState::ValidatedFresh; 4]))
            .collect(),
        [GateState::ValidatedFresh; 5],
    )
}

fn snapshot_of(
    financials: &CanonicalFinancials,
    judgment: &JudgmentInputs,
    gates: InputGates,
) -> StudySnapshot {
    StudySnapshot::new(financials, judgment, &QuarterlyObservations::empty(), gates)
}

fn gate_state() -> impl Strategy<Value = GateState> {
    prop_oneof![
        Just(GateState::Missing),
        Just(GateState::NotValidated),
        Just(GateState::Stale),
        Just(GateState::ValidatedFresh),
    ]
}

proptest! {
    /// Invariant 2a, the full equivalence: `Full` ⟺ (every gate validated-and-fresh ∧
    /// ¬low_confidence), over arbitrary gate vectors × low_confidence. Withheld ⟺ any gate
    /// Missing; Provisional covers every other degraded case.
    #[test]
    fn full_iff_all_gates_green_and_not_low_confidence(
        year_states in prop::collection::vec(prop::array::uniform4(gate_state()), 0..6),
        judgment_states in prop::array::uniform5(gate_state()),
        low_confidence in any::<bool>(),
    ) {
        let year_count = year_states.len() as u32;
        let financials = CanonicalFinancials {
            years: vec![], // gate derivation reads only usable_years; spec-§9 engine handles it
            findings: vec![],
            usable_years: if low_confidence { 0 } else { 5.max(year_count) },
        };
        let gates = InputGates::new(
            year_states
                .iter()
                .enumerate()
                .map(|(i, s)| YearGates::new(2020 + i as i32, *s))
                .collect(),
            judgment_states,
        );
        let all_green = year_states
            .iter()
            .flatten()
            .chain(judgment_states.iter())
            .all(|s| s.is_validated_fresh());
        let any_missing = year_states
            .iter()
            .flatten()
            .chain(judgment_states.iter())
            .any(|s| matches!(s, GateState::Missing));

        let snap = snapshot_of(&financials, &JudgmentInputs::empty(), gates);
        match snap.verdict() {
            Verdict::Full(full) => {
                prop_assert!(
                    all_green && !low_confidence,
                    "invariant 2a violated: Full derived while a gate is non-green or the \
                     study is low-confidence"
                );
                prop_assert_eq!(full.inputs_hash(), snap.inputs_hash());
                prop_assert_eq!(full.method_version(), METHOD_VERSION);
            }
            Verdict::Withheld(deg) => {
                prop_assert!(any_missing, "Withheld requires a missing input (§5)");
                prop_assert!(!deg.open_gates().is_empty());
            }
            Verdict::Provisional(deg) => {
                prop_assert!(
                    !any_missing,
                    "Provisional must never carry a missing input — that is Withheld"
                );
                prop_assert!(
                    !all_green || low_confidence,
                    "Provisional requires a degradation cause"
                );
                prop_assert!(!deg.open_gates().is_empty() || deg.low_confidence());
            }
        }
        // Stamped on every state (ADD9).
        prop_assert_eq!(snap.verdict().inputs_hash(), snap.inputs_hash());
        prop_assert_eq!(snap.verdict().method_version(), METHOD_VERSION);
    }

    /// Any SINGLE non-green gate ⇒ never Full — and the open gates name it, queryably.
    #[test]
    fn any_single_non_green_gate_is_never_full(
        slot in 0..25usize, // 5 years × 4 fields + 5 judgment inputs
        state in prop_oneof![
            Just(GateState::Missing),
            Just(GateState::NotValidated),
            Just(GateState::Stale),
        ],
    ) {
        let mut year_states = [[GateState::ValidatedFresh; 4]; 5];
        let mut judgment_states = [GateState::ValidatedFresh; 5];
        if slot < 20 {
            year_states[slot / 4][slot % 4] = state;
        } else {
            judgment_states[slot - 20] = state;
        }
        let gates = InputGates::new(
            year_states
                .iter()
                .enumerate()
                .map(|(i, s)| YearGates::new(2021 + i as i32, *s))
                .collect(),
            judgment_states,
        );
        let snap = snapshot_of(&five_year_financials(), &complete_judgment(), gates);
        prop_assert!(
            !matches!(snap.verdict(), Verdict::Full(_)),
            "invariant 2a violated: FullVerdict derived while one input is {state:?}"
        );
        prop_assert_eq!(
            snap.verdict().open_gates().len(),
            1,
            "exactly the one non-green input must be named"
        );
        prop_assert_eq!(snap.verdict().open_gates()[0].state, state);
    }

    /// Derivation is deterministic: the same inputs + gates always build an identical
    /// snapshot (same verdict, same digest).
    #[test]
    fn derivation_is_deterministic(
        judgment_states in prop::array::uniform5(gate_state()),
    ) {
        let gates = InputGates::new(
            (2021..=2025)
                .map(|y| YearGates::new(y, [GateState::ValidatedFresh; 4]))
                .collect(),
            judgment_states,
        );
        let a = snapshot_of(&five_year_financials(), &complete_judgment(), gates.clone());
        let b = snapshot_of(&five_year_financials(), &complete_judgment(), gates);
        prop_assert_eq!(a.verdict(), b.verdict());
        prop_assert_eq!(a.inputs_hash(), b.inputs_hash());
        prop_assert_eq!(a, b);
    }

    /// ADD9: changing ANY load-bearing value changes the digest — the prior verdict is
    /// detectably orphaned (invalidation, never a silent overwrite).
    #[test]
    fn changing_any_load_bearing_value_changes_the_digest(
        slot in 0..25usize, // 5 years × 4 fields + 5 judgment inputs
        delta in 1..1000i64,
    ) {
        let base_financials = five_year_financials();
        let base_judgment = complete_judgment();
        let bump = Decimal::new(delta, 2); // 0.01 .. 9.99 — never zero
        let mut financials = base_financials.clone();
        let mut judgment = base_judgment.clone();
        if slot < 20 {
            let year = &mut financials.years[slot / 4];
            let field = match slot % 4 {
                0 => &mut year.sales,
                1 => &mut year.eps,
                2 => &mut year.high_price,
                _ => &mut year.low_price,
            };
            *field = Some(field.expect("load-bearing fields are populated") + bump);
        } else {
            let field = match slot - 20 {
                0 => &mut judgment.estimated_high_eps,
                1 => &mut judgment.estimated_low_eps,
                2 => &mut judgment.judged_avg_high_pe,
                3 => &mut judgment.judged_avg_low_pe,
                _ => &mut judgment.current_price,
            };
            *field = Some(field.expect("judgment inputs are populated") + bump);
        }
        let before = snapshot_of(&base_financials, &base_judgment, green_gates());
        let after = snapshot_of(&financials, &judgment, green_gates());
        prop_assert_ne!(
            before.inputs_hash(),
            after.inputs_hash(),
            "ADD9 violated: a load-bearing value changed but the content address did not"
        );
    }
}

/// Equal inputs ⇒ equal digest, including across `Decimal` SCALE differences: `3.0` and `3`
/// are the same value and must produce the same content address (`Decimal::normalize` before
/// encoding — the `contract::provenance` NOTE).
#[test]
fn value_equal_inputs_digest_equal_across_scales() {
    let scaled = JudgmentInputs {
        estimated_high_eps: Some(d("2.0")),
        estimated_low_eps: Some(d("1.50")),
        judged_avg_high_pe: Some(d("20.00")),
        judged_avg_low_pe: Some(d("10.0")),
        current_price: Some(d("25.000")),
        ..JudgmentInputs::empty()
    };
    let plain = JudgmentInputs {
        estimated_high_eps: Some(d("2")),
        estimated_low_eps: Some(d("1.5")),
        judged_avg_high_pe: Some(d("20")),
        judged_avg_low_pe: Some(d("10")),
        current_price: Some(d("25")),
        ..JudgmentInputs::empty()
    };
    let mut scaled_financials = five_year_financials();
    scaled_financials.years[0].sales = Some(d("100.000")); // value-equal to the plain "100"
    let a = snapshot_of(&scaled_financials, &scaled, green_gates());
    let b = snapshot_of(&five_year_financials(), &plain, green_gates());
    assert_eq!(
        a.inputs_hash(),
        b.inputs_hash(),
        "value-equal inputs must digest equal regardless of Decimal scale"
    );
    assert_eq!(a.verdict(), b.verdict());
}

/// `Full` is ORTHOGONAL to `quality_value_candidate` (AC 2 pin): a degenerate forecast range
/// (`forecast_high ≤ forecast_low`) with ALL inputs validated-and-fresh stays `Full` — the
/// facts state the insufficiency queryably; integrity gates trust in the inputs, not the
/// computability of every number.
#[test]
fn full_is_orthogonal_to_quality_value_candidate() {
    // forecast_high = 20 × 1 = 20; forecast_low (option a) = 30 × 1.5 = 45 ≥ 20 — degenerate.
    let degenerate_judgment = JudgmentInputs {
        estimated_high_eps: Some(d("1")),
        estimated_low_eps: Some(d("1.5")),
        judged_avg_high_pe: Some(d("20")),
        judged_avg_low_pe: Some(d("30")),
        current_price: Some(d("25")),
        ..JudgmentInputs::empty()
    };
    let snap = snapshot_of(&five_year_financials(), &degenerate_judgment, green_gates());
    let Verdict::Full(full) = snap.verdict() else {
        panic!(
            "a degenerate forecast range with all-validated inputs must stay Full — \
             integrity is about trust in the inputs, got {:?}",
            snap.verdict()
        );
    };
    assert!(
        snap.outputs().risk_reward.zones.is_none(),
        "the degenerate range must yield no zones (engine precondition of this pin)"
    );
    assert!(
        !full.facts().quality_value_candidate,
        "the degenerate range cannot be a quality-value candidate — Full carries the \
         unmet facts queryably instead of being blocked"
    );
}
