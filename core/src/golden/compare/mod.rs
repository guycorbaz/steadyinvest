//! The pure self-check (AC 2): [`check`] replays one [`GoldenStudy`] through the real
//! pipeline `normalize` → `ssg::compute` and compares actual vs expected — exact for
//! everything categorical, within `golden_relative_tolerance()` for derived numerics
//! (relative to the EXPECTED value; `expected == 0` therefore demands exact equality).
//!
//! Deviation wording is neutral (FR13): paths, values and the words "expected"/"actual" —
//! no imperative verbs. The zone-derived field-path nouns (`buy_top`,
//! `present_price_in_buy_zone`) and the zone labels themselves are exempt per spec §6,
//! gated by the posture test in `core/tests/golden_compare.rs` (NOT by the `ssg` inventory,
//! whose `contains_word` matcher treats `_` as a word boundary and would reject `buy_top`).
//!
//! Layout: this file owns the report types ([`GoldenDeviation`], [`GoldenReport`]) and the
//! two entry points ([`check`], [`check_all`]); `primitives` owns the renderers, the
//! `Expected*` → engine conversions and the three comparison primitives; `sections` owns the
//! per-section `compare_*` walkers.

mod primitives;
mod sections;

use crate::golden::schema::GoldenStudy;
use crate::method_version::METHOD_VERSION;
use crate::normalize::{RawFinancials, normalize};
use crate::ssg::{JudgmentInputs, QuarterlyObservations, compute};
use rust_decimal::Decimal;
use sections::{compare_normalize_findings, compare_outputs};
use std::fmt;

/// One observed deviation: where, what was expected, what the engine produced, and the
/// relative error when the comparison was numeric.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenDeviation {
    /// Dotted field path into the expected/actual output surface.
    pub path: String,
    /// The fixture's expected value, rendered neutrally (`null` for an expected unknown).
    pub expected: String,
    /// The engine's actual value, rendered neutrally (`null` for an actual unknown).
    pub actual: String,
    /// `|actual − expected| / |expected|` for numeric comparisons with a non-zero expected.
    pub relative_error: Option<Decimal>,
}

impl fmt::Display for GoldenDeviation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: expected {}, actual {}",
            self.path, self.expected, self.actual
        )?;
        if let Some(re) = self.relative_error {
            write!(f, " (relative error {re})")?;
        }
        Ok(())
    }
}

/// The result of replaying one golden study.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenReport {
    /// The golden's `meta.id`.
    pub id: String,
    /// `true` iff the replay produced no deviation.
    pub passed: bool,
    /// Every observed deviation, in output-surface order (empty when `passed`).
    pub deviations: Vec<GoldenDeviation>,
}

/// Replay one golden study through `normalize` → `compute` and compare. Pure — no I/O.
pub fn check(study: &GoldenStudy) -> GoldenReport {
    let mut deviations = Vec::new();

    // A stale golden must be re-validated at a method bump, never silently replayed.
    if study.meta.method_version != METHOD_VERSION {
        deviations.push(GoldenDeviation {
            path: "meta.method_version".to_string(),
            expected: study.meta.method_version.clone(),
            actual: METHOD_VERSION.to_string(),
            relative_error: None,
        });
        return GoldenReport {
            id: study.meta.id.clone(),
            passed: false,
            deviations,
        };
    }

    // A golden whose input fails normalization is a failing golden, not a panic.
    let canonical = match normalize(RawFinancials::from(&study.input)) {
        Ok(c) => c,
        Err(e) => {
            deviations.push(GoldenDeviation {
                path: "input".to_string(),
                expected: "structurally valid raw financials".to_string(),
                actual: e.to_string(),
                relative_error: None,
            });
            return GoldenReport {
                id: study.meta.id.clone(),
                passed: false,
                deviations,
            };
        }
    };

    if let Some(expected) = &study.expected.normalize_findings {
        compare_normalize_findings(expected, &canonical.findings, &mut deviations);
    }

    let judgment = JudgmentInputs::from(&study.input.judgment);
    let observations = QuarterlyObservations::from(&study.input.quarterly);
    let actual = compute(&canonical, &judgment, &observations);
    compare_outputs(&study.expected, &actual, &mut deviations);

    GoldenReport {
        id: study.meta.id.clone(),
        passed: deviations.is_empty(),
        deviations,
    }
}

/// [`check`] over a bundle — the exact pair of entry points Story 2.13 consumes.
pub fn check_all(studies: &[GoldenStudy]) -> Vec<GoldenReport> {
    studies.iter().map(check).collect()
}
