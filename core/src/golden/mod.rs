//! Golden reference studies & self-check (Story 1.9, FR9 / NFR-C1/C2): a serde-strict
//! fixture format ([`GoldenStudy`]) plus a pure self-check ([`check`] / [`check_all`]) that
//! replays a fixture through the real pipeline `normalize` → `ssg::compute` and compares the
//! actual outputs against independently hand-computed expected values.
//!
//! Pure calculation (Cardinal Rule): **no file I/O in `core/src`** — parsing is `serde`
//! derives on golden-local structs, comparison is pure; callers own file reading and JSON
//! parsing (the CI runner `core/tests/golden_gate.rs` now, the Story-2.13 "verify engine" UI
//! later). `serde_json` is a dev-dependency only — the shipped engine's dependency surface is
//! unchanged.
//!
//! **Fixtures are oracles, not user data** — the journal's forward-compat rule is deliberately
//! INVERTED here: every struct is `deny_unknown_fields`, required fields carry no
//! `serde(default)`, and required-but-nullable fields are presence-enforced (an *omitted*
//! field is a parse error; explicit `null` means "expected unknown"). Only the optional
//! per-year tables and `normalize_findings` use plain `Option` omission semantics.
//!
//! Comparison semantics (spec §7): **exact for everything categorical** (zones, trends,
//! verdict facts, flags, findings, `low_confidence`, the U/D state, every unknown —
//! expected `null` ⇔ actual `None`, both directions); **derived numerics within the method
//! tolerance** `|actual − expected| ≤ golden_relative_tolerance() × |expected|` (relative to
//! the EXPECTED value — `expected == 0` therefore demands exact equality). A fixture whose
//! `meta.method_version ≠ METHOD_VERSION` fails its check: a stale golden must be
//! re-validated at a method bump, never silently replayed.

mod compare;
mod schema;

pub use compare::{GoldenDeviation, GoldenReport, check, check_all};
pub use schema::{
    ExpectedCalcFinding, ExpectedCriterion, ExpectedGrowth, ExpectedManagement,
    ExpectedNormalizeFinding, ExpectedOutputs, ExpectedReturns, ExpectedRiskReward, ExpectedTrend,
    ExpectedUpsideDownside, ExpectedValuation, ExpectedVerdictFacts, ExpectedYearRatios,
    ExpectedYearValuation, ExpectedZone, ExpectedZoneBounds, FIXTURE_FORMAT_VERSION, FixtureAmount,
    FixtureForecastLowOption, FixtureJudgment, FixtureQuarterly, FixtureSplit, FixtureYear,
    GoldenInput, GoldenMeta, GoldenStudy,
};
