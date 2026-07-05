//! Verdict integrity & coherence invariants (Story 1.11 — FR11/FR12/FR13, ADD9).
//!
//! This module makes the product's core promise structural: **a verdict can never silently
//! outrun the state of its inputs**. It wraps the pure engine ([`crate::ssg::compute`]) with:
//!
//! - a per-load-bearing-input **gate state** ([`GateState`]) in core's OWN vocabulary — `core`
//!   does not depend on `contract`; mapping `contract::Cell` review/freshness onto these states
//!   is the caller's job (Epic 2 glue, exercised here only in tests);
//! - a structured **gates collection** ([`InputGates`]) covering exactly the spec-§5 catalog
//!   ([`crate::method::LOAD_BEARING_YEAR_FIELDS`] per usable year plus
//!   [`crate::method::LOAD_BEARING_JUDGMENT_INPUTS`]);
//! - an immutable **computed-study snapshot** ([`StudySnapshot`]) binding the engine outputs
//!   and the gates in ONE constructor call — once built, neither can be swapped, so **no
//!   incoherent intermediate frame is representable** (epic AC 3): the verdict and the
//!   staleness/integrity states are both reads of this single frame;
//! - the **integrity-gated verdict** ([`Verdict`] / [`FullVerdict`]): a `Full` verdict is
//!   constructible ONLY when every load-bearing gate is validated-and-fresh and the study is
//!   not low-confidence — the compiler is the gate (invariant 2a);
//! - **content addressing** (ADD9): every derived verdict is stamped with
//!   `(inputs_hash, METHOD_VERSION)` — `verdict = f(hash(inputs), method_version)` — so a
//!   prior verdict is detectably orphaned by any input change (invalidation, never a silent
//!   overwrite). The digest addresses input **values only** — the gates (review/freshness)
//!   shape the verdict's integrity state but are deliberately NOT hashed: re-validating or
//!   refreshing an unchanged value must not orphan the verdict it supports (the `digest`
//!   submodule owns the frozen encoding).
//!
//! Pure calculation (Cardinal Rule): no I/O / UI / SQL / network; exact
//! [`rust_decimal::Decimal`] only.

mod digest;
mod gates;

pub use gates::{GateState, GatedInput, InputGates, OpenGate, YearGates};

use crate::method_version::METHOD_VERSION;
use crate::normalize::CanonicalFinancials;
use crate::ssg::VerdictFacts;
use crate::ssg::{self, JudgmentInputs, QuarterlyObservations, SsgOutputs};
use digest::inputs_digest;

/// The integrity-**Full** verdict (invariant 2a): the facts rest on all-validated-and-fresh
/// load-bearing inputs of a not-low-confidence study. **The compiler is the gate** — private
/// fields, no public constructor, no `Default`, no serde (nothing persists a verdict in
/// Epic 1); the ONLY way to obtain one is [`StudySnapshot::verdict`].
///
/// `Full` is ORTHOGONAL to [`VerdictFacts::quality_value_candidate`]: it means *the facts rest
/// on trusted inputs*, NOT that the company passes the four criteria — a `Full` verdict may
/// carry `Unmet`/`UnmetByInsufficiency` facts (e.g. a degenerate forecast range with
/// all-validated inputs stays `Full`; the facts state the insufficiency queryably).
///
/// The type is reachable at this path (so the `compile_fail` proof below fails for the right
/// reason, not a path typo):
///
/// ```
/// let _: Option<&steadyinvest_core::verdict::FullVerdict> = None;
/// ```
///
/// Constructing one outside `core` does not compile (invariant 2a):
///
/// ```compile_fail
/// steadyinvest_core::verdict::FullVerdict {
///     facts: unreachable!(),
///     inputs_hash: unreachable!(),
///     method_version: unreachable!(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullVerdict {
    facts: VerdictFacts,
    inputs_hash: String,
    method_version: &'static str,
}

impl FullVerdict {
    /// The neutral verdict facts (FR13) these trusted inputs support.
    pub fn facts(&self) -> &VerdictFacts {
        &self.facts
    }

    /// The content address of the inputs this verdict derives from (ADD9).
    pub fn inputs_hash(&self) -> &str {
        &self.inputs_hash
    }

    /// The method version this verdict was derived under.
    pub fn method_version(&self) -> &'static str {
        self.method_version
    }
}

/// A non-`Full` verdict (`Provisional` / `Withheld` payload): the facts where statable, plus
/// the queryable evidence — which inputs, which gate state, and the distinct low-confidence
/// state (FR8: queryable and separate from the gate evidence).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegradedVerdict {
    facts: VerdictFacts,
    open_gates: Vec<OpenGate>,
    low_confidence: bool,
    inputs_hash: String,
    method_version: &'static str,
}

impl DegradedVerdict {
    /// The neutral verdict facts (still statable on a degraded study — spec §9 typed unknowns).
    pub fn facts(&self) -> &VerdictFacts {
        &self.facts
    }

    /// Every load-bearing input whose gate is not validated-and-fresh (FR11/FR12).
    pub fn open_gates(&self) -> &[OpenGate] {
        &self.open_gates
    }

    /// The FR8 low-confidence state — queryable, distinct from the gate evidence.
    pub fn low_confidence(&self) -> bool {
        self.low_confidence
    }

    /// The content address of the inputs this verdict derives from (ADD9).
    pub fn inputs_hash(&self) -> &str {
        &self.inputs_hash
    }

    /// The method version this verdict was derived under.
    pub fn method_version(&self) -> &'static str {
        self.method_version
    }
}

/// The integrity-gated verdict, derived once at snapshot construction (spec §5, FR12). Three
/// states; every state is stamped `(inputs_hash, METHOD_VERSION)` (ADD9).
///
/// Recorded interpretation (Withheld/Provisional split): **Withheld** ⟺ at least one
/// load-bearing input is missing (§5 "missing"); **Provisional** ⟺ nothing missing but at
/// least one input not-validated or stale, OR the study is low-confidence (spec §1 + FR12:
/// low confidence degrades the verdict too). Epic 2's "degraded" badge is a *rendering* of
/// `Provisional`'s open gates, not a fourth core state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every load-bearing gate validated-and-fresh AND not low-confidence.
    Full(FullVerdict),
    /// Degraded but nothing missing: ≥ 1 not-validated/stale gate, or low confidence.
    Provisional(DegradedVerdict),
    /// At least one load-bearing input is missing.
    Withheld(DegradedVerdict),
}

impl Verdict {
    /// The neutral verdict facts, whatever the integrity state.
    pub fn facts(&self) -> &VerdictFacts {
        match self {
            Verdict::Full(v) => v.facts(),
            Verdict::Provisional(v) | Verdict::Withheld(v) => v.facts(),
        }
    }

    /// The content address of the inputs this verdict derives from (ADD9).
    pub fn inputs_hash(&self) -> &str {
        match self {
            Verdict::Full(v) => v.inputs_hash(),
            Verdict::Provisional(v) | Verdict::Withheld(v) => v.inputs_hash(),
        }
    }

    /// The method version this verdict was derived under.
    pub fn method_version(&self) -> &'static str {
        match self {
            Verdict::Full(v) => v.method_version(),
            Verdict::Provisional(v) | Verdict::Withheld(v) => v.method_version(),
        }
    }

    /// The queryable degradation evidence — empty for `Full` (FR11/FR12).
    pub fn open_gates(&self) -> &[OpenGate] {
        match self {
            Verdict::Full(_) => &[],
            Verdict::Provisional(v) | Verdict::Withheld(v) => v.open_gates(),
        }
    }

    /// The FR8 low-confidence state — `false` for `Full` (a low-confidence study is never
    /// `Full` by derivation).
    pub fn low_confidence(&self) -> bool {
        match self {
            Verdict::Full(_) => false,
            Verdict::Provisional(v) | Verdict::Withheld(v) => v.low_confidence(),
        }
    }
}

/// The immutable computed-study snapshot: engine outputs + input gates + content address +
/// derived verdict, **born together in one constructor call** (epic AC 3 "same coherence
/// frame"). Private fields, no `&mut` accessor, no setter — once built, neither the outputs
/// nor the gates can be swapped, so verdict and staleness/integrity are always reads of the
/// SAME frame. In headless Epic 1 the "transaction" of the epic AC *is* this snapshot
/// (recorded interpretation); nothing persists a verdict before Epic 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudySnapshot {
    outputs: SsgOutputs,
    gates: InputGates,
    inputs_hash: String,
    verdict: Verdict,
}

impl StudySnapshot {
    /// THE single construction path: takes the engine inputs + gates, computes the content
    /// address ([`Self::inputs_hash`]), runs [`crate::ssg::compute`] itself (`SsgOutputs` is
    /// never caller-supplied — a caller-supplied value would make a mismatched outputs/inputs
    /// frame representable), and derives the [`Verdict`] — all before anything is exposed.
    ///
    /// See [`InputGates`] for the caller's usable-years scoping duty.
    pub fn new(
        financials: &CanonicalFinancials,
        judgment: &JudgmentInputs,
        observations: &QuarterlyObservations,
        gates: InputGates,
    ) -> StudySnapshot {
        let inputs_hash = inputs_digest(financials, judgment, observations);
        let outputs = ssg::compute(financials, judgment, observations);
        let verdict = derive_verdict(&outputs, &gates, &inputs_hash);
        StudySnapshot {
            outputs,
            gates,
            inputs_hash,
            verdict,
        }
    }

    /// The full engine output set this frame was computed from.
    pub fn outputs(&self) -> &SsgOutputs {
        &self.outputs
    }

    /// The §5 input gates of this frame.
    pub fn gates(&self) -> &InputGates {
        &self.gates
    }

    /// The verdict derived from THIS frame — the only way a [`FullVerdict`] comes to exist.
    pub fn verdict(&self) -> &Verdict {
        &self.verdict
    }

    /// Deterministic SHA-256 hex content address of the engine inputs (ADD9). See
    /// [`inputs_digest`] for the documented encoding.
    pub fn inputs_hash(&self) -> &str {
        &self.inputs_hash
    }

    /// The method version this snapshot was computed under ([`METHOD_VERSION`]).
    pub fn method_version(&self) -> &'static str {
        METHOD_VERSION
    }
}

/// Derive the verdict from one coherence frame — called exactly once, at snapshot
/// construction. `Full` ⟺ every gate validated-and-fresh ∧ ¬low_confidence; `Withheld` ⟺
/// ≥ 1 input missing; `Provisional` ⟺ every other degraded case.
fn derive_verdict(outputs: &SsgOutputs, gates: &InputGates, inputs_hash: &str) -> Verdict {
    let open_gates = gates.open_gates();
    let low_confidence = outputs.low_confidence;
    if open_gates.is_empty() && !low_confidence {
        return Verdict::Full(FullVerdict {
            facts: outputs.verdict_facts.clone(),
            inputs_hash: inputs_hash.to_string(),
            method_version: METHOD_VERSION,
        });
    }
    let any_missing = open_gates
        .iter()
        .any(|g| matches!(g.state, GateState::Missing));
    let degraded = DegradedVerdict {
        facts: outputs.verdict_facts.clone(),
        open_gates,
        low_confidence,
        inputs_hash: inputs_hash.to_string(),
        method_version: METHOD_VERSION,
    };
    if any_missing {
        Verdict::Withheld(degraded)
    } else {
        Verdict::Provisional(degraded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::method::{LOAD_BEARING_JUDGMENT_INPUTS, LOAD_BEARING_YEAR_FIELDS};
    use rust_decimal::Decimal;

    /// All-green gates over `years` usable years.
    fn green_gates(years: &[i32]) -> InputGates {
        InputGates::new(
            years
                .iter()
                .map(|&y| YearGates::new(y, [GateState::ValidatedFresh; 4]))
                .collect(),
            [GateState::ValidatedFresh; 5],
        )
    }

    /// Minimal full-confidence financials: no year values needed — gate derivation reads only
    /// `usable_years` (the engine handles all-`None` inputs per spec §9).
    fn financials(usable_years: u32) -> CanonicalFinancials {
        CanonicalFinancials {
            years: vec![],
            findings: vec![],
            usable_years,
        }
    }

    fn snapshot(gates: InputGates, usable_years: u32) -> StudySnapshot {
        StudySnapshot::new(
            &financials(usable_years),
            &JudgmentInputs::empty(),
            &QuarterlyObservations::empty(),
            gates,
        )
    }

    /// AC 1: the gates type covers each pinned catalog entry — every load-bearing year field
    /// and judgment input resolves to a gate (mirrors 1.8's `judgment_field_present` glue).
    #[test]
    fn gates_cover_exactly_the_pinned_catalogs() {
        let gates = green_gates(&[2025]);
        for field in LOAD_BEARING_YEAR_FIELDS {
            assert!(
                gates.year_gate(2025, field).is_some(),
                "load-bearing year field {field:?} does not resolve to a gate"
            );
        }
        for name in LOAD_BEARING_JUDGMENT_INPUTS {
            assert!(
                gates.judgment_gate(name).is_some(),
                "load-bearing judgment input {name:?} does not resolve to a gate"
            );
        }
        // Exactly the catalogs — array lengths are tied to the catalog constants, and unknown
        // names do not resolve (safe direction).
        assert_eq!(
            gates.year_gates()[0].states().len(),
            LOAD_BEARING_YEAR_FIELDS.len()
        );
        assert_eq!(
            gates.judgment_gates().len(),
            LOAD_BEARING_JUDGMENT_INPUTS.len()
        );
        assert_eq!(gates.year_gate(2025, "dividend_per_share"), None);
        assert_eq!(gates.judgment_gate("recent_severe_low"), None);
        assert_eq!(gates.year_gate(1999, "sales"), None, "ungated year");
    }

    #[test]
    fn all_green_full_confidence_derives_full() {
        let snap = snapshot(green_gates(&[2021, 2022, 2023, 2024, 2025]), 5);
        match snap.verdict() {
            Verdict::Full(full) => {
                assert_eq!(full.inputs_hash(), snap.inputs_hash());
                assert_eq!(full.method_version(), METHOD_VERSION);
            }
            other => panic!(
                "invariant 2a violated: all gates validated-and-fresh and full confidence \
                 must derive Full, got {other:?}"
            ),
        }
        assert!(snap.verdict().open_gates().is_empty());
        assert!(!snap.verdict().low_confidence());
    }

    #[test]
    fn any_missing_input_derives_withheld_and_names_it() {
        let mut judgment_gates = [GateState::ValidatedFresh; 5];
        judgment_gates[4] = GateState::Missing; // current_price
        let gates = InputGates::new(
            vec![YearGates::new(2025, [GateState::ValidatedFresh; 4])],
            judgment_gates,
        );
        let snap = snapshot(gates, 5);
        match snap.verdict() {
            Verdict::Withheld(d) => {
                assert_eq!(
                    d.open_gates(),
                    &[OpenGate {
                        input: GatedInput::JudgmentInput {
                            name: "current_price"
                        },
                        state: GateState::Missing,
                    }],
                    "the missing input must be named, queryably"
                );
                assert!(!d.low_confidence());
            }
            other => panic!("a missing load-bearing input must derive Withheld, got {other:?}"),
        }
    }

    #[test]
    fn not_validated_or_stale_derives_provisional_and_names_them() {
        let mut states = [GateState::ValidatedFresh; 4];
        states[1] = GateState::NotValidated; // eps
        states[3] = GateState::Stale; // low_price
        let gates = InputGates::new(
            vec![YearGates::new(2024, states)],
            [GateState::ValidatedFresh; 5],
        );
        let snap = snapshot(gates, 5);
        match snap.verdict() {
            Verdict::Provisional(d) => {
                assert_eq!(
                    d.open_gates(),
                    &[
                        OpenGate {
                            input: GatedInput::YearField {
                                year: 2024,
                                field: "eps"
                            },
                            state: GateState::NotValidated,
                        },
                        OpenGate {
                            input: GatedInput::YearField {
                                year: 2024,
                                field: "low_price"
                            },
                            state: GateState::Stale,
                        },
                    ]
                );
            }
            other => panic!("degraded-but-nothing-missing must derive Provisional, got {other:?}"),
        }
    }

    /// Spec §1 + FR12: low confidence alone degrades the verdict — and stays queryable,
    /// distinct from the gate evidence (FR8).
    #[test]
    fn low_confidence_alone_derives_provisional_with_no_open_gates() {
        let snap = snapshot(green_gates(&[2024, 2025]), 2);
        match snap.verdict() {
            Verdict::Provisional(d) => {
                assert!(
                    d.open_gates().is_empty(),
                    "low confidence is NOT a gate hold — it must stay distinct"
                );
                assert!(d.low_confidence());
            }
            other => panic!("a low-confidence study must never be Full, got {other:?}"),
        }
    }

    #[test]
    fn snapshot_exposes_one_coherent_frame() {
        let snap = snapshot(green_gates(&[2025]), 5);
        // Verdict stamp and snapshot address are the same read of the same frame.
        assert_eq!(snap.verdict().inputs_hash(), snap.inputs_hash());
        assert_eq!(snap.verdict().method_version(), snap.method_version());
        assert_eq!(snap.method_version(), METHOD_VERSION);
        assert_eq!(
            snap.inputs_hash().len(),
            64,
            "SHA-256 hex digest must be 64 chars"
        );
        assert!(snap.inputs_hash().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn digest_is_deterministic_and_value_sensitive() {
        let j_a = JudgmentInputs {
            current_price: Some(Decimal::new(250, 1)), // 25.0
            ..JudgmentInputs::empty()
        };
        let j_b = JudgmentInputs {
            current_price: Some(Decimal::new(25, 0)), // 25 — value-equal, different scale
            ..JudgmentInputs::empty()
        };
        let j_c = JudgmentInputs {
            current_price: Some(Decimal::new(26, 0)), // different value
            ..JudgmentInputs::empty()
        };
        let f = financials(5);
        let obs = QuarterlyObservations::empty();
        assert_eq!(
            inputs_digest(&f, &j_a, &obs),
            inputs_digest(&f, &j_b, &obs),
            "value-equal inputs must digest equal (Decimal::normalize before encoding)"
        );
        assert_ne!(
            inputs_digest(&f, &j_a, &obs),
            inputs_digest(&f, &j_c, &obs),
            "a changed load-bearing value must change the digest (ADD9 orphaning)"
        );
    }

    /// FR13 posture gate, verdict-local (AC 6): no public type/variant/accessor name or
    /// emitted sentinel introduced by this story contains a banned imperative verb
    /// (case-insensitive, whole-word) — same recipe as the 1.8/1.9/1.10 local gates.
    #[test]
    fn verdict_vocabulary_contains_no_banned_verbs() {
        use crate::method::{BANNED_VERBS_EN, BANNED_VERBS_FR, contains_word};

        let vocabulary: [&str; 42] = [
            // Types
            "GateState",
            "YearGates",
            "InputGates",
            "GatedInput",
            "OpenGate",
            "FullVerdict",
            "DegradedVerdict",
            "Verdict",
            "StudySnapshot",
            // Variants
            "Missing",
            "NotValidated",
            "Stale",
            "ValidatedFresh",
            "YearField",
            "JudgmentInput",
            "Full",
            "Provisional",
            "Withheld",
            // Public accessors / constructors (as snake_case identifiers)
            "is_validated_fresh",
            "new",
            "year",
            "states",
            "gate",
            "year_gates",
            "judgment_gates",
            "judgment_gate",
            "year_gate",
            "open_gates",
            "facts",
            "inputs_hash",
            "method_version",
            "low_confidence",
            "outputs",
            "gates",
            "verdict",
            "input",
            "state",
            // Struct-variant field names of `GatedInput` (public via pattern matching)
            "field",
            "name",
            // Digest sentinel + section prefixes (the only emitted strings of this module)
            "absent",
            "judgment",
            "observations",
        ];

        for s in vocabulary {
            for banned in BANNED_VERBS_EN.iter().chain(BANNED_VERBS_FR.iter()) {
                assert!(
                    !contains_word(s, banned),
                    "verdict vocabulary {s:?} contains banned verb {banned:?} (FR13)"
                );
            }
        }
    }
}
