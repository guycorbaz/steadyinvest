//! The §5 **input gates**: per-load-bearing-input gate states in core's own vocabulary
//! (FR11/FR12/FR13). [`GateState`] names the four neutral §5 facts for ONE input;
//! [`YearGates`]/[`InputGates`] tie the states to exactly the pinned catalogs
//! ([`LOAD_BEARING_YEAR_FIELDS`] / [`LOAD_BEARING_JUDGMENT_INPUTS`]); [`OpenGate`] is the
//! queryable evidence a gate is not validated-and-fresh. Mapping `contract::Cell`
//! review/freshness onto these states is the caller's job — `core` does not depend on
//! `contract`.

use crate::method::{LOAD_BEARING_JUDGMENT_INPUTS, LOAD_BEARING_YEAR_FIELDS};

/// Gate state of ONE load-bearing input (spec §5) — four neutral, fact-stating cases (FR13).
///
/// The verdict is `Full` only when every gate is [`GateState::ValidatedFresh`]; the other three
/// states each name the §5 degradation fact they stand for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateState {
    /// The input is absent (§5 "missing").
    Missing,
    /// The input is present but its review state is not ✓ (§5 "not validated (review ≠ ✓)").
    NotValidated,
    /// The input is present and validated but stale (§5 "stale").
    Stale,
    /// The input is present, validated (✓) and not stale — the only verdict-green state.
    ValidatedFresh,
}

impl GateState {
    /// `true` iff this is the verdict-green state ([`GateState::ValidatedFresh`]).
    pub const fn is_validated_fresh(self) -> bool {
        matches!(self, GateState::ValidatedFresh)
    }
}

/// Gate states of the four load-bearing fields of ONE usable year, aligned by index with
/// [`LOAD_BEARING_YEAR_FIELDS`] (the array length is tied to the pinned catalog mechanically).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YearGates {
    year: i32,
    states: [GateState; LOAD_BEARING_YEAR_FIELDS.len()],
}

impl YearGates {
    /// Gates of one year; `states[i]` is the gate of `LOAD_BEARING_YEAR_FIELDS[i]`.
    pub fn new(year: i32, states: [GateState; LOAD_BEARING_YEAR_FIELDS.len()]) -> Self {
        YearGates { year, states }
    }

    /// The reported year these gates belong to.
    pub fn year(&self) -> i32 {
        self.year
    }

    /// All four states, in pinned-catalog order.
    pub fn states(&self) -> &[GateState; LOAD_BEARING_YEAR_FIELDS.len()] {
        &self.states
    }

    /// The gate of a load-bearing year field looked up by its catalog name; `None` for an
    /// unknown name (same safe-direction contract as `normalize::canonical_field_present`).
    pub fn gate(&self, field: &str) -> Option<GateState> {
        LOAD_BEARING_YEAR_FIELDS
            .iter()
            .position(|f| *f == field)
            .map(|i| self.states[i])
    }
}

/// The §5 gates collection: the four load-bearing fields **of each usable year** plus the five
/// load-bearing judgment inputs — exactly the pinned catalogs, nothing else.
///
/// **Caller's mapping duty (spec §5 scoping):** the per-year gates apply to the load-bearing
/// fields *of the usable years* — callers MUST pass one [`YearGates`] entry per usable year of
/// the study and none for unusable years (a year missing a load-bearing field is simply not
/// usable, §4; the [`GateState::Missing`] case is still representable per field as defense in
/// depth). `core` cannot check this scoping itself: usability lives in
/// [`crate::normalize::CanonicalFinancials`], review/freshness live in the caller's vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputGates {
    year_gates: Vec<YearGates>,
    judgment_gates: [GateState; LOAD_BEARING_JUDGMENT_INPUTS.len()],
}

impl InputGates {
    /// Build the gates collection; `judgment_gates[i]` is the gate of
    /// `LOAD_BEARING_JUDGMENT_INPUTS[i]`.
    ///
    /// Part of the caller's mapping duty (see the type docs): supply at most ONE
    /// [`YearGates`] entry per year — on duplicates, [`Self::year_gate`] reads the first
    /// entry while [`crate::verdict::Verdict::open_gates`] reports every entry.
    pub fn new(
        year_gates: Vec<YearGates>,
        judgment_gates: [GateState; LOAD_BEARING_JUDGMENT_INPUTS.len()],
    ) -> Self {
        InputGates {
            year_gates,
            judgment_gates,
        }
    }

    /// Per-usable-year gates, in the order supplied by the caller.
    pub fn year_gates(&self) -> &[YearGates] {
        &self.year_gates
    }

    /// The five judgment gates, in pinned-catalog order.
    pub fn judgment_gates(&self) -> &[GateState; LOAD_BEARING_JUDGMENT_INPUTS.len()] {
        &self.judgment_gates
    }

    /// The gate of a load-bearing judgment input looked up by its catalog name; `None` for an
    /// unknown name.
    pub fn judgment_gate(&self, name: &str) -> Option<GateState> {
        LOAD_BEARING_JUDGMENT_INPUTS
            .iter()
            .position(|n| *n == name)
            .map(|i| self.judgment_gates[i])
    }

    /// The gate of a load-bearing year field looked up by year + catalog name; `None` when the
    /// year is not gated (not usable / not supplied) or the name is unknown.
    pub fn year_gate(&self, year: i32, field: &str) -> Option<GateState> {
        self.year_gates
            .iter()
            .find(|y| y.year == year)
            .and_then(|y| y.gate(field))
    }

    /// Every gate that is NOT validated-and-fresh, with the input it names — the queryable
    /// degradation evidence (FR11/FR12), in catalog order (years in supplied order, fields in
    /// catalog order, then judgment inputs in catalog order).
    pub(super) fn open_gates(&self) -> Vec<OpenGate> {
        let mut open = Vec::new();
        for yg in &self.year_gates {
            for (i, state) in yg.states.iter().enumerate() {
                if !state.is_validated_fresh() {
                    open.push(OpenGate {
                        input: GatedInput::YearField {
                            year: yg.year,
                            field: LOAD_BEARING_YEAR_FIELDS[i],
                        },
                        state: *state,
                    });
                }
            }
        }
        for (i, state) in self.judgment_gates.iter().enumerate() {
            if !state.is_validated_fresh() {
                open.push(OpenGate {
                    input: GatedInput::JudgmentInput {
                        name: LOAD_BEARING_JUDGMENT_INPUTS[i],
                    },
                    state: *state,
                });
            }
        }
        open
    }
}

/// Identifies ONE load-bearing input by its pinned catalog name (FR11 traceability).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatedInput {
    /// A per-year load-bearing field ([`LOAD_BEARING_YEAR_FIELDS`]).
    YearField {
        /// The reported year the field belongs to.
        year: i32,
        /// The field's pinned catalog name.
        field: &'static str,
    },
    /// A load-bearing judgment input ([`LOAD_BEARING_JUDGMENT_INPUTS`]).
    JudgmentInput {
        /// The input's pinned catalog name.
        name: &'static str,
    },
}

/// One queryable reason a verdict is not `Full`: a load-bearing input whose gate is not
/// validated-and-fresh (FR12 "testably/queryable degraded"). `state` is never
/// [`GateState::ValidatedFresh`] (by construction in `InputGates::open_gates`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenGate {
    /// The load-bearing input this gate names.
    pub input: GatedInput,
    /// The not-validated-and-fresh state the gate is in.
    pub state: GateState,
}
