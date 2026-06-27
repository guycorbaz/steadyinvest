//! End-to-end coherence test for Story 1.11 — invariant 2b proper, tested ON the
//! `contract::Cell::edited` rail through the real path: `contract::Study` → glue mapping →
//! [`normalize`] → snapshot ([`StudySnapshot::new`] computes + derives) → [`Verdict`].
//!
//! `steadyinvest-contract` is a **dev-dependency of core ONLY** (the 1.9 `serde_json`
//! precedent): the glue below is the Epic-2 production mapping *previewed* in tests —
//! `app/state.rs` will own the real one. Glue rules (recorded interpretations):
//! - `Cell` → [`GateState`]: value `None` ⇒ `Missing`; else review ≠ ✓ ⇒ `NotValidated`;
//!   else stale ⇒ `Stale`; else `ValidatedFresh` (precedence missing > not-validated > stale);
//! - judgment inputs: present ⇒ `ValidatedFresh`, absent ⇒ `Missing` — judgments are the
//!   user's own current assertion and carry no review state in contract v1 (issue #14);
//! - per-year gates only for usable years (§4: every load-bearing cell value present).

use proptest::prelude::*;
use rust_decimal::Decimal;
use steadyinvest_contract::{
    Cell, Coverage, Freshness, Judgment, Money, Provenance, Review, Source, Study, Timestamp,
    YearData,
};
use steadyinvest_core::method::{LOAD_BEARING_JUDGMENT_INPUTS, LOAD_BEARING_YEAR_FIELDS};
use steadyinvest_core::normalize::{normalize, RawAmount, RawFinancials, RawYear};
use steadyinvest_core::ssg::{ForecastLowOption, JudgmentInputs, QuarterlyObservations};
use steadyinvest_core::verdict::{
    GateState, GatedInput, InputGates, StudySnapshot, Verdict, YearGates,
};

fn d(s: &str) -> Decimal {
    Decimal::from_str_exact(s).expect("test literal must parse")
}

fn money(s: &str) -> Money {
    Money::from(d(s))
}

fn manual_provenance(logical_version: u64) -> Provenance {
    Provenance {
        source: Source::Manual,
        logical_version,
        timestamp: Timestamp("2026-06-12T08:00:00Z".to_string()),
        hash_of_dependencies: "edit0011".to_string(),
    }
}

fn validated_cell(value: &str) -> Cell {
    Cell {
        value: Some(money(value)),
        source: Source::Manual,
        freshness: Freshness::Current,
        review: Review::Validated,
        coverage: Coverage::Present,
        provenance: Provenance {
            source: Source::Manual,
            logical_version: 1,
            timestamp: Timestamp("2026-06-10T00:00:00Z".to_string()),
            hash_of_dependencies: "seed0001".to_string(),
        },
        pending: None,
    }
}

/// A `Study` whose load-bearing cells are ALL ✓ + current (5 usable years 2021–2025) with a
/// complete judgment. Built via serde (core's existing dev-dep) so `core` needs no `uuid`
/// dev-edge just to mint the two ids — the cells and judgment are then set programmatically.
fn all_green_study() -> Study {
    let mut study: Study = serde_json::from_value(serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "journal_id": "00000000-0000-0000-0000-000000000002",
        "security_ticker": "DEMO",
        "native_currency": "USD",
        "years": [],
        "judgment": { "forecast_low_option": "avg_low_pe_times_eps" },
        "created_at": "2026-06-12T00:00:00Z",
        "schema_version": 1
    }))
    .expect("study skeleton must deserialize");

    study.years = (0u32..5)
        .map(|i| {
            let step = Decimal::from(i);
            YearData {
                year: 2021 + i as i32,
                sales: validated_cell(&(d("100") + step * d("10")).to_string()),
                eps: validated_cell(&(d("1.00") + step * d("0.10")).to_string()),
                high_price: validated_cell(&(d("20") + step * d("2")).to_string()),
                low_price: validated_cell(&(d("10") + step).to_string()),
                dividend_per_share: None,
                pre_tax_profit: None,
                book_value_per_share: None,
            }
        })
        .collect();
    study.judgment = Judgment {
        estimated_high_eps: Some(money("2.00")),
        estimated_low_eps: Some(money("1.50")),
        // The four fields added to `contract::Judgment` in Story 2.2 (issue #14). This glue
        // preview maps only the load-bearing inputs it already exercised, so they stay `None`
        // here — the mechanical update keeps this construction compiling after the additive
        // contract change (the four fields' engine mapping is Story 2.6's job).
        projected_sales_growth_pct: None,
        projected_eps_growth_pct: None,
        judged_avg_high_pe: Some(money("20")),
        judged_avg_low_pe: Some(money("10")),
        forecast_low_option: study.judgment.forecast_low_option,
        recent_severe_low: None,
        current_price: Some(money("25")),
        present_full_year_dividend: None,
    };
    study
}

// ── The Epic-2-preview glue: contract vocabulary → core vocabulary ──

/// `Cell` data state → gate state (precedence: missing > not-validated > stale).
fn gate_of(cell: &Cell) -> GateState {
    if cell.value.is_none() {
        GateState::Missing
    } else if cell.review != Review::Validated {
        GateState::NotValidated
    } else if cell.freshness == Freshness::Stale {
        GateState::Stale
    } else {
        GateState::ValidatedFresh
    }
}

/// The load-bearing cells of one year, in pinned-catalog order
/// (`LOAD_BEARING_YEAR_FIELDS` = sales, eps, high_price, low_price).
fn load_bearing_cells(year: &YearData) -> [&Cell; 4] {
    [&year.sales, &year.eps, &year.high_price, &year.low_price]
}

/// §4 usability in contract terms: every load-bearing cell has a value.
fn year_is_usable(year: &YearData) -> bool {
    load_bearing_cells(year).iter().all(|c| c.value.is_some())
}

/// The §5 gates of a study: the load-bearing fields of the USABLE years (the caller's
/// scoping duty documented on `InputGates`) + the five judgment inputs
/// (present ⇒ validated-and-fresh, absent ⇒ missing — issue #14).
fn study_gates(study: &Study) -> InputGates {
    let year_gates = study
        .years
        .iter()
        .filter(|y| year_is_usable(y))
        .map(|y| YearGates::new(y.year, load_bearing_cells(y).map(gate_of)))
        .collect();
    let j = &study.judgment;
    let judgment_gate = |present: bool| {
        if present {
            GateState::ValidatedFresh
        } else {
            GateState::Missing
        }
    };
    // Pinned-catalog order: estimated_high_eps, estimated_low_eps, judged_avg_high_pe,
    // judged_avg_low_pe, current_price.
    let judgment_gates = [
        judgment_gate(j.estimated_high_eps.is_some()),
        judgment_gate(j.estimated_low_eps.is_some()),
        judgment_gate(j.judged_avg_high_pe.is_some()),
        judgment_gate(j.judged_avg_low_pe.is_some()),
        judgment_gate(j.current_price.is_some()),
    ];
    InputGates::new(year_gates, judgment_gates)
}

/// Study cells → the raw input of the real engine path (raw → canonical → compute).
fn raw_from_study(study: &Study) -> RawFinancials {
    let amount = |cell: &Cell| {
        cell.value.map(|m| RawAmount {
            value: m.as_decimal(),
            currency: study.native_currency.clone(),
        })
    };
    RawFinancials {
        native_currency: study.native_currency.clone(),
        years: study
            .years
            .iter()
            .map(|y| {
                let mut raw = RawYear::empty(y.year);
                raw.sales = amount(&y.sales);
                raw.eps = amount(&y.eps);
                raw.high_price = amount(&y.high_price);
                raw.low_price = amount(&y.low_price);
                raw
            })
            .collect(),
        splits: vec![],
    }
}

/// `contract::Judgment` (`Option<Money>`) → engine `JudgmentInputs` (`Option<Decimal>`).
fn judgment_inputs(judgment: &Judgment) -> JudgmentInputs {
    JudgmentInputs {
        estimated_high_eps: judgment.estimated_high_eps.map(Money::as_decimal),
        estimated_low_eps: judgment.estimated_low_eps.map(Money::as_decimal),
        judged_avg_high_pe: judgment.judged_avg_high_pe.map(Money::as_decimal),
        judged_avg_low_pe: judgment.judged_avg_low_pe.map(Money::as_decimal),
        current_price: judgment.current_price.map(Money::as_decimal),
        forecast_low_option: match judgment.forecast_low_option {
            steadyinvest_contract::ForecastLowOption::AvgLowPeTimesEps => {
                ForecastLowOption::AvgLowPeTimesEps
            }
            steadyinvest_contract::ForecastLowOption::AvgLowPriceLast5y => {
                ForecastLowOption::AvgLowPriceLast5y
            }
            steadyinvest_contract::ForecastLowOption::RecentSevereLow => {
                ForecastLowOption::RecentSevereLow
            }
            steadyinvest_contract::ForecastLowOption::DividendSupported => {
                ForecastLowOption::DividendSupported
            }
        },
        ..JudgmentInputs::empty()
    }
}

/// THE single rebuild path: study → glue → normalize → snapshot. Both the all-green and the
/// post-mutation frames go through exactly this function — "same coherence frame" is
/// structural, not a convention.
fn snapshot_of(study: &Study) -> StudySnapshot {
    let financials = normalize(raw_from_study(study)).expect("fixture must normalize");
    StudySnapshot::new(
        &financials,
        &judgment_inputs(&study.judgment),
        &QuarterlyObservations::empty(),
        study_gates(study),
    )
}

#[test]
fn all_green_study_derives_full_through_the_real_path() {
    let snap = snapshot_of(&all_green_study());
    assert!(
        matches!(snap.verdict(), Verdict::Full(_)),
        "all load-bearing cells ✓+current with a complete judgment over 5 usable years must \
         derive Full, got {:?}",
        snap.verdict()
    );
    assert!(!snap.outputs().low_confidence);
}

/// Invariant 2b, end to end: mutating ONE load-bearing cell through the rail and rebuilding
/// through the single path yields ONE new frame in which the cell reads `ToReview` AND the
/// verdict is degraded and names that input — never one without the other, because both are
/// reads of the same immutable snapshot. The old Full verdict keeps the OLD content address
/// (detectably orphaned, ADD9).
#[test]
fn mutating_one_load_bearing_cell_degrades_verdict_in_the_same_frame() {
    let study = all_green_study();
    let before = snapshot_of(&study);
    let Verdict::Full(old_full) = before.verdict() else {
        panic!("precondition: the untouched study must be Full");
    };

    // Mutate ONE load-bearing cell through the rail (divergent value: 1.20 → 9.99).
    let mut edited_study = study.clone();
    edited_study.years[2].eps = edited_study.years[2]
        .eps
        .edited(Some(money("9.99")), manual_provenance(2));

    let after = snapshot_of(&edited_study);

    // Read 1 of the new frame: the cell's review is ? …
    assert_eq!(
        edited_study.years[2].eps.review,
        Review::ToReview,
        "invariant 2b violated: the divergent edit must demote ✓ → ?"
    );
    // … and read 2 of the SAME frame: the dependent verdict is degraded and names the input.
    let Verdict::Provisional(degraded) = after.verdict() else {
        panic!(
            "invariant 2b violated: the verdict over an edited, unvalidated input must be \
             Provisional, got {:?}",
            after.verdict()
        );
    };
    assert_eq!(
        degraded.open_gates(),
        &[steadyinvest_core::verdict::OpenGate {
            input: GatedInput::YearField {
                year: 2023,
                field: "eps"
            },
            state: GateState::NotValidated,
        }],
        "the degradation evidence must name exactly the mutated input"
    );

    // ADD9: the prior verdict is detectably orphaned — it carries the OLD address.
    assert_ne!(
        old_full.inputs_hash(),
        after.inputs_hash(),
        "the input changed, so the old verdict's content address must no longer match"
    );
    assert_eq!(old_full.inputs_hash(), before.inputs_hash());
}

/// A withdrawn judgment input is MISSING (§5) ⇒ the verdict is Withheld and names it.
#[test]
fn withdrawing_a_judgment_input_withholds_the_verdict() {
    let mut study = all_green_study();
    study.judgment.current_price = None;
    let snap = snapshot_of(&study);
    let Verdict::Withheld(degraded) = snap.verdict() else {
        panic!(
            "a missing load-bearing judgment input must withhold the verdict, got {:?}",
            snap.verdict()
        );
    };
    assert!(
        degraded
            .open_gates()
            .contains(&steadyinvest_core::verdict::OpenGate {
                input: GatedInput::JudgmentInput {
                    name: "current_price"
                },
                state: GateState::Missing,
            }),
        "the missing input must be named: {:?}",
        degraded.open_gates()
    );
}

// ── QA workflow additions (qa-generate-e2e-tests, 2026-06-12): end-to-end user
//    workflows not covered by the dev suites — non-divergent refresh, stale cells,
//    re-validation recovery, §4 usability interplay, multi-input evidence. ──

fn provider_provenance(logical_version: u64) -> Provenance {
    Provenance {
        source: Source::Provider,
        logical_version,
        timestamp: Timestamp("2026-06-12T09:00:00Z".to_string()),
        hash_of_dependencies: "refresh01".to_string(),
    }
}

/// Epic-3 preview, the non-divergent annual refresh: re-entering a value-equal number
/// (here at a DIFFERENT Decimal scale, `"100.0"` over `"100"`) through the rail keeps ✓,
/// keeps the verdict `Full`, and keeps the SAME content address — a no-op refresh must
/// never orphan the prior verdict (ADD9 invalidation fires on divergence only).
#[test]
fn non_divergent_refresh_keeps_full_and_the_same_content_address() {
    let study = all_green_study();
    let before = snapshot_of(&study);
    assert!(matches!(before.verdict(), Verdict::Full(_)));

    let mut refreshed = study.clone();
    refreshed.years[0].sales = refreshed.years[0]
        .sales
        .edited(Some(money("100.0")), provider_provenance(2));
    assert_eq!(
        refreshed.years[0].sales.review,
        Review::Validated,
        "a value-equal refresh is NOT a divergence and must keep ✓ (Money(\"100.0\") == Money(\"100\"))"
    );

    let after = snapshot_of(&refreshed);
    assert!(
        matches!(after.verdict(), Verdict::Full(_)),
        "a non-divergent refresh must keep the verdict Full, got {:?}",
        after.verdict()
    );
    assert_eq!(
        before.inputs_hash(),
        after.inputs_hash(),
        "value-equal inputs must keep the SAME content address through the rail \
         (Decimal::normalize before encoding) — a no-op refresh must not orphan the verdict"
    );
}

/// A validated-but-STALE load-bearing cell degrades the verdict and is named with the
/// `Stale` gate state — the third §5 degradation cause, end to end through the glue.
#[test]
fn stale_load_bearing_cell_derives_provisional_naming_stale() {
    let mut study = all_green_study();
    study.years[4].high_price.freshness = Freshness::Stale;

    let snap = snapshot_of(&study);
    let Verdict::Provisional(degraded) = snap.verdict() else {
        panic!(
            "a stale load-bearing input (nothing missing) must derive Provisional, got {:?}",
            snap.verdict()
        );
    };
    assert_eq!(
        degraded.open_gates(),
        &[steadyinvest_core::verdict::OpenGate {
            input: GatedInput::YearField {
                year: 2025,
                field: "high_price"
            },
            state: GateState::Stale,
        }],
        "the stale input must be named with the Stale state, queryably"
    );
    assert!(!degraded.low_confidence());
}

/// Recovery workflow: divergent edit → `Provisional`; the user re-validates the cell (an
/// explicit act, FR20 — Epic 2 will own the UI for it) → rebuilding through the same single
/// path derives `Full` again. The content address follows the VALUES only: the divergent
/// edit changes it, the re-validation alone does not (gates are not content).
#[test]
fn revalidation_restores_full_with_the_new_content_address() {
    let study = all_green_study();
    let before = snapshot_of(&study);

    let mut edited_study = study.clone();
    edited_study.years[1].low_price = edited_study.years[1]
        .low_price
        .edited(Some(money("12.50")), manual_provenance(2));
    let degraded_frame = snapshot_of(&edited_study);
    assert!(
        matches!(degraded_frame.verdict(), Verdict::Provisional(_)),
        "precondition: the divergent edit must degrade the verdict"
    );

    // The explicit user act (FR20): review ? → ✓ on the edited cell.
    edited_study.years[1].low_price.review = Review::Validated;
    let recovered = snapshot_of(&edited_study);
    assert!(
        matches!(recovered.verdict(), Verdict::Full(_)),
        "re-validating the one open input must derive Full again, got {:?}",
        recovered.verdict()
    );
    assert_ne!(
        recovered.inputs_hash(),
        before.inputs_hash(),
        "the recovered Full verdict rests on the NEW values — new content address (ADD9)"
    );
    assert_eq!(
        recovered.inputs_hash(),
        degraded_frame.inputs_hash(),
        "re-validation alone changes no value, so the content address is unchanged \
         (the digest covers the engine inputs, not the gates)"
    );
}

/// §4/§5 interplay: CLEARING a load-bearing year cell through the rail makes the year
/// unusable (§4 — it is not gated at all), drops usable years below the floor, and the
/// degradation arrives via the queryable low-confidence state — not via `Withheld`.
#[test]
fn clearing_a_load_bearing_year_cell_degrades_via_usability_and_low_confidence() {
    let study = all_green_study();
    let before = snapshot_of(&study);

    let mut edited_study = study.clone();
    edited_study.years[0].eps = edited_study.years[0].eps.edited(None, manual_provenance(2));
    assert_eq!(edited_study.years[0].eps.review, Review::ToReview);
    assert_eq!(
        edited_study.years[0].eps.coverage,
        Coverage::ToFill,
        "an explicit clear reopens the gap (recorded interpretation)"
    );

    let after = snapshot_of(&edited_study);
    assert!(
        after.gates().year_gate(2021, "eps").is_none(),
        "a year missing a load-bearing field is not usable (§4) and must not be gated"
    );
    assert!(
        after.outputs().low_confidence,
        "4 usable years < floor of 5"
    );
    let Verdict::Provisional(degraded) = after.verdict() else {
        panic!(
            "the degradation must arrive via low confidence (Provisional), got {:?}",
            after.verdict()
        );
    };
    assert!(degraded.low_confidence());
    assert!(
        degraded.open_gates().is_empty(),
        "the unusable year is out of the §5 scope — low confidence is the queryable \
         cause, distinct from the gate evidence (FR8): {:?}",
        degraded.open_gates()
    );
    assert_ne!(before.inputs_hash(), after.inputs_hash());
}

/// The degradation evidence accumulates: one divergent year edit + one withdrawn judgment
/// input in the same frame ⇒ `Withheld` (missing has precedence over not-validated for the
/// state split) and BOTH inputs are named.
#[test]
fn multiple_degraded_inputs_are_all_named_and_missing_wins_the_split() {
    let mut study = all_green_study();
    study.years[3].sales = study.years[3]
        .sales
        .edited(Some(money("999")), manual_provenance(2));
    study.judgment.judged_avg_low_pe = None;

    let snap = snapshot_of(&study);
    let Verdict::Withheld(degraded) = snap.verdict() else {
        panic!(
            "a missing input anywhere in the frame must derive Withheld, got {:?}",
            snap.verdict()
        );
    };
    assert_eq!(
        degraded.open_gates(),
        &[
            steadyinvest_core::verdict::OpenGate {
                input: GatedInput::YearField {
                    year: 2024,
                    field: "sales"
                },
                state: GateState::NotValidated,
            },
            steadyinvest_core::verdict::OpenGate {
                input: GatedInput::JudgmentInput {
                    name: "judged_avg_low_pe"
                },
                state: GateState::Missing,
            },
        ],
        "EVERY degraded input must be named, not just the one that decided the state"
    );
}

proptest! {
    /// AC 5 property: degradation holds for EVERY choice of mutated load-bearing input —
    /// any of the 4 year fields on any of the 5 usable years (divergent rail edit ⇒
    /// not-validated ⇒ Provisional), or any of the 5 judgment inputs withdrawn (missing ⇒
    /// Withheld). In every case the new frame's verdict is NOT Full, the evidence names the
    /// input, and the old Full verdict is orphaned (old hash ≠ new hash).
    #[test]
    fn degradation_holds_for_every_load_bearing_input(slot in 0..25usize) {
        let study = all_green_study();
        let before = snapshot_of(&study);
        prop_assert!(matches!(before.verdict(), Verdict::Full(_)));

        let mut edited_study = study.clone();
        let expected: steadyinvest_core::verdict::OpenGate;
        if slot < 20 {
            let (year_idx, field_idx) = (slot / 4, slot % 4);
            let year = &mut edited_study.years[year_idx];
            let cell = match field_idx {
                0 => &mut year.sales,
                1 => &mut year.eps,
                2 => &mut year.high_price,
                _ => &mut year.low_price,
            };
            // Divergent rail edit: +1 on the current value (all fixture values are positive).
            let bumped = cell.value.expect("fixture cells are populated").as_decimal()
                + Decimal::ONE;
            *cell = cell.edited(Some(Money::from(bumped)), manual_provenance(2));
            prop_assert_eq!(cell.review, Review::ToReview, "the rail must demote ✓ → ?");
            expected = steadyinvest_core::verdict::OpenGate {
                input: GatedInput::YearField {
                    year: 2021 + year_idx as i32,
                    field: LOAD_BEARING_YEAR_FIELDS[field_idx],
                },
                state: GateState::NotValidated,
            };
        } else {
            let j = &mut edited_study.judgment;
            let field = match slot - 20 {
                0 => &mut j.estimated_high_eps,
                1 => &mut j.estimated_low_eps,
                2 => &mut j.judged_avg_high_pe,
                3 => &mut j.judged_avg_low_pe,
                _ => &mut j.current_price,
            };
            *field = None;
            expected = steadyinvest_core::verdict::OpenGate {
                input: GatedInput::JudgmentInput {
                    name: LOAD_BEARING_JUDGMENT_INPUTS[slot - 20],
                },
                state: GateState::Missing,
            };
        }

        let after = snapshot_of(&edited_study);
        prop_assert!(
            !matches!(after.verdict(), Verdict::Full(_)),
            "invariant 2b violated for {expected:?}: verdict stayed Full after the mutation"
        );
        if slot < 20 {
            prop_assert!(matches!(after.verdict(), Verdict::Provisional(_)));
        } else {
            prop_assert!(matches!(after.verdict(), Verdict::Withheld(_)));
        }
        prop_assert!(
            after.verdict().open_gates().contains(&expected),
            "the evidence must name the mutated input {:?}, got {:?}",
            expected,
            after.verdict().open_gates()
        );
        prop_assert_ne!(
            before.inputs_hash(),
            after.inputs_hash(),
            "ADD9: the prior verdict must be detectably orphaned"
        );
    }
}
