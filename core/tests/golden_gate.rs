//! The CI-gating golden runner (Story 1.9, AC 5–7): discovers EVERY `*.json` under
//! `core/tests/golden/`, fails when the bundle is thinner than the AC-4 minimum, fails on
//! any parse error, replays each fixture through `core::golden::check`, and fails with the
//! full deviation list on any mismatch. Picked up by `cargo test --all --locked` — no CI
//! workflow change needed for the gate to block merge.
//!
//! Negative controls (AC 6) are embedded as in-memory tampered JSON — NEVER files in the
//! fixtures directories — proving the gate cannot pass silently: a tampered categorical, a
//! numeric just beyond (and just inside) the ±0.5% tolerance, a stale `method_version`, a
//! typo'd field, an omitted required field, and the `null` ⇔ `None` rule in both directions.
//!
//! The drift test (AC 7, ADD12) asserts `app/assets/golden/` carries a byte-identical copy
//! of every fixture (READMEs excluded — they intentionally differ): the Story-2.13 "verify
//! engine" screen reads those assets and calls the same `check`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use steadyinvest_core::golden::{GoldenStudy, check};

/// AC-4 floor: an empty or thinned glob must never pass.
const MIN_GOLDEN_FIXTURES: usize = 10;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/assets/golden")
}

/// Every `*.json` in `dir`, keyed by file name, with its raw bytes. Sorted (BTreeMap) so
/// failures are deterministic and self-explaining.
fn json_files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read fixture directory {}: {e}", dir.display()));
    let mut files = BTreeMap::new();
    for entry in entries {
        let path = entry
            .unwrap_or_else(|e| panic!("cannot read a directory entry in {}: {e}", dir.display()))
            .path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let name = path
                .file_name()
                .expect("a .json path has a file name")
                .to_string_lossy()
                .into_owned();
            let bytes = fs::read(&path)
                .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
            files.insert(name, bytes);
        }
    }
    files
}

fn parse(name: &str, bytes: &[u8]) -> GoldenStudy {
    let text = std::str::from_utf8(bytes)
        .unwrap_or_else(|e| panic!("fixture {name} is not valid UTF-8: {e}"));
    serde_json::from_str(text).unwrap_or_else(|e| panic!("fixture {name} failed to parse: {e}"))
}

fn deviation_lines(report: &steadyinvest_core::golden::GoldenReport) -> String {
    report
        .deviations
        .iter()
        .map(|d| format!("  - {d}"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ── AC 5: the gate ──

/// The gate proper: ≥ MIN fixtures, every one parses, every one passes its check.
#[test]
fn every_bundled_golden_passes_the_self_check() {
    let files = json_files(&fixtures_dir());
    assert!(
        !files.is_empty(),
        "no golden fixtures found in {} - an empty glob must never pass",
        fixtures_dir().display()
    );
    assert!(
        files.len() >= MIN_GOLDEN_FIXTURES,
        "only {} golden fixtures found ({:?}) - the AC-4 minimum is {MIN_GOLDEN_FIXTURES}; \
         a thinned bundle must never pass",
        files.len(),
        files.keys().collect::<Vec<_>>()
    );

    for (name, bytes) in &files {
        let study = parse(name, bytes);
        assert_eq!(
            format!("{}.json", study.meta.id),
            *name,
            "fixture file name must carry the golden id (one naming convention)"
        );
        let report = check(&study);
        assert!(
            report.passed,
            "golden {name} deviates from the engine:\n{}\n\
             (re-derive the expected values by hand - a deviation means a fixture error or a \
             real engine bug, never a number to copy back)",
            deviation_lines(&report)
        );
    }
}

// ── AC 6: negative controls (embedded, in-memory only) ──

/// The trusted base for tampering: the worked-example fixture's on-disk content. Tampered
/// variants exist only in memory - never as files in the fixtures directories.
const BASE: &str = include_str!("golden/g01-worked-example.json");

/// Apply a targeted tamper, asserting it actually changed something.
fn tampered(from: &str, to: &str) -> String {
    let out = BASE.replace(from, to);
    assert_ne!(out, BASE, "tamper {from:?} -> {to:?} did not apply");
    out
}

fn check_str(json: &str) -> steadyinvest_core::golden::GoldenReport {
    let study: GoldenStudy = serde_json::from_str(json).expect("tampered control still parses");
    check(&study)
}

/// (a) A tampered categorical (wrong zone) is reported as a deviation.
#[test]
fn control_tampered_zone_is_reported() {
    let json = tampered(
        "\"present_price_zone\": \"buy\"",
        "\"present_price_zone\": \"sell\"",
    );
    let report = check_str(&json);
    assert!(!report.passed, "a wrong zone must never pass silently");
    assert!(
        report
            .deviations
            .iter()
            .any(|d| d.path == "risk_reward.present_price_zone"),
        "the zone deviation must be reported: {:?}",
        report.deviations
    );
}

/// (b) A numeric tampered just beyond +0.5% is reported, while the same value just inside
/// the tolerance passes — the tolerance boundary itself is tested at the gate level.
#[test]
fn control_numeric_tolerance_boundary() {
    // Actual sales CAGR is 10 (exact-root construction). Expected 10.06 puts the actual
    // 0.06 away with a tolerance of 0.005 x 10.06 = 0.0503 -> deviation.
    let beyond = tampered(
        "\"sales_cagr_pct\": \"10\"",
        "\"sales_cagr_pct\": \"10.06\"",
    );
    let report = check_str(&beyond);
    assert!(!report.passed);
    let deviation = report
        .deviations
        .iter()
        .find(|d| d.path == "growth.sales_cagr_pct")
        .unwrap_or_else(|| panic!("missing CAGR deviation: {:?}", report.deviations));
    assert!(
        deviation.relative_error.is_some(),
        "a numeric deviation carries its relative error"
    );

    // Expected 10.05: the actual 10 is 0.05 away, tolerance 0.005 x 10.05 = 0.05025 -> passes.
    let inside = tampered(
        "\"sales_cagr_pct\": \"10\"",
        "\"sales_cagr_pct\": \"10.05\"",
    );
    let report = check_str(&inside);
    assert!(
        report.passed,
        "a value just inside the tolerance must pass: {:?}",
        report.deviations
    );
}

/// (c) A stale `method_version` fails the check.
#[test]
fn control_stale_method_version_fails() {
    let json = tampered(
        "\"method_version\": \"ssg-1.0.0\"",
        "\"method_version\": \"ssg-0.9.9\"",
    );
    let report = check_str(&json);
    assert!(
        !report.passed,
        "a stale golden must never be silently replayed"
    );
    assert_eq!(report.deviations.len(), 1);
    assert_eq!(report.deviations[0].path, "meta.method_version");
}

/// (d) A typo'd field name fails to parse — `deny_unknown_fields` proven at the gate level.
#[test]
fn control_typoed_field_fails_to_parse() {
    let json = tampered("\"sales_cagr_pct\"", "\"sales_cagr_typo\"");
    let r: Result<GoldenStudy, _> = serde_json::from_str(&json);
    let err = r
        .expect_err("a typo'd field must fail the parse")
        .to_string();
    assert!(
        err.contains("sales_cagr_typo") || err.contains("unknown field"),
        "unexpected parse error: {err}"
    );
}

/// (e) A fixture OMITTING a required expected field fails to parse — the AC-1
/// presence-enforcement proven at the gate level.
#[test]
fn control_omitted_required_field_fails_to_parse() {
    let json = tampered("\"eps_cagr_pct\": \"10\",", "");
    let r: Result<GoldenStudy, _> = serde_json::from_str(&json);
    let err = r
        .expect_err("an omitted required field must fail the parse")
        .to_string();
    assert!(
        err.contains("eps_cagr_pct"),
        "the error must name the omitted field: {err}"
    );
}

/// (f) The `null` ⇔ `None` rule, both directions, through the full check.
#[test]
fn control_null_none_rule_both_directions() {
    // Direction 1: expected null, actual has a measured value -> deviation.
    let expected_null = tampered("\"eps_cagr_pct\": \"10\"", "\"eps_cagr_pct\": null");
    let report = check_str(&expected_null);
    assert!(!report.passed);
    let deviation = report
        .deviations
        .iter()
        .find(|d| d.path == "growth.eps_cagr_pct")
        .unwrap_or_else(|| panic!("missing deviation: {:?}", report.deviations));
    assert_eq!(deviation.expected, "null");

    // Direction 2: expected a value, actual is unknown (withhold the year-ago quarterly EPS
    // observation -> the engine yields None while the expected 25 stays) -> deviation.
    let actual_none = tampered(
        "\"year_ago_quarter_eps\": \"0.32\"",
        "\"year_ago_quarter_eps\": null",
    );
    let report = check_str(&actual_none);
    assert!(!report.passed);
    let deviation = report
        .deviations
        .iter()
        .find(|d| d.path == "growth.quarterly_eps_change_pct")
        .unwrap_or_else(|| panic!("missing deviation: {:?}", report.deviations));
    assert_eq!(deviation.expected, "25");
    assert_eq!(deviation.actual, "null");
}

// ── AC 7: app-asset drift test (ADD12) ──

/// `app/assets/golden/*.json` must be the same SET of files as `core/tests/golden/*.json`,
/// each pair byte-identical (READMEs excluded - they intentionally differ). The source of
/// truth is `core/tests/golden/`; regenerate by plain copy.
#[test]
fn app_assets_carry_a_byte_identical_copy_of_every_golden() {
    let source = json_files(&fixtures_dir());
    let assets = json_files(&assets_dir());
    assert_eq!(
        source.keys().collect::<Vec<_>>(),
        assets.keys().collect::<Vec<_>>(),
        "app/assets/golden must carry exactly the fixture set of core/tests/golden \
         (source of truth: core/tests/golden; regenerate by copying)"
    );
    for (name, bytes) in &source {
        assert_eq!(
            bytes, &assets[name],
            "asset copy of {name} drifted from the source fixture - re-copy from \
             core/tests/golden/"
        );
    }
}
