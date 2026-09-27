//! Story 2.13 — the runtime "verify engine" (FR9) + the read-only demonstration study (FR62).
//!
//! **Verify** REPLAYS the bundled golden fixtures (Epic 1; byte-identical to `core/tests/golden/`,
//! enforced by the golden-gate drift test) through the public `core::golden::check_all` comparator
//! and folds the result into a UI-ready report carrying the method identity + per-fixture
//! pass/deviation lines. **Demo** converts the `g01-worked-example` fixture into an in-memory
//! `contract::Study` for a look-only walkthrough.
//!
//! Nothing here calculates (Cardinal Rule) — the SSG math and the comparison live in `core`; this
//! module only bundles the fixtures and shapes data across the `core::golden` ⇄ `contract` boundary.

use steadyinvest_contract::{
    Cell, Coverage, ForecastLowOption, Freshness, Judgment, Money, Provenance, Review, Source,
    Study, Timestamp, YearData,
};
use steadyinvest_core::golden::{
    FixtureAmount, FixtureForecastLowOption, FixtureJudgment, FixtureYear, GoldenStudy, check_all,
};
use steadyinvest_core::{METHOD_VERSION, method};
use uuid::Uuid;

use crate::viewmodel::entry::tofill_cell;

/// The 11 bundled golden fixtures, embedded at compile time — byte-identical to `core/tests/golden/`
/// (the golden-gate drift test enforces the copy). `(id, json)`. `include_str!` paths are checked at
/// build time; no runtime filesystem, no new dependency.
const GOLDEN_FIXTURES: &[(&str, &str)] = &[
    (
        "g01-worked-example",
        include_str!("../../assets/golden/g01-worked-example.json"),
    ),
    (
        "g02-split-grossup-pipeline",
        include_str!("../../assets/golden/g02-split-grossup-pipeline.json"),
    ),
    (
        "g03-candidate-avg-low-price-option",
        include_str!("../../assets/golden/g03-candidate-avg-low-price-option.json"),
    ),
    (
        "g04-price-on-buy-top-boundary",
        include_str!("../../assets/golden/g04-price-on-buy-top-boundary.json"),
    ),
    (
        "g05-ud-exactly-at-target",
        include_str!("../../assets/golden/g05-ud-exactly-at-target.json"),
    ),
    (
        "g06-relative-value-at-ceiling",
        include_str!("../../assets/golden/g06-relative-value-at-ceiling.json"),
    ),
    (
        "g07-sell-zone-flag-cluster",
        include_str!("../../assets/golden/g07-sell-zone-flag-cluster.json"),
    ),
    (
        "g08-low-confidence-four-years",
        include_str!("../../assets/golden/g08-low-confidence-four-years.json"),
    ),
    (
        "g09-unknown-rich-degenerate",
        include_str!("../../assets/golden/g09-unknown-rich-degenerate.json"),
    ),
    (
        "g10-recent-severe-low-option",
        include_str!("../../assets/golden/g10-recent-severe-low-option.json"),
    ),
    (
        "g11-dividend-supported-option",
        include_str!("../../assets/golden/g11-dividend-supported-option.json"),
    ),
];

/// The fixture chosen as the read-only demonstration study (FR62) — the tutorial-style worked example.
const DEMO_FIXTURE_ID: &str = "g01-worked-example";

// ── User-facing neutral prose rendered in the verify panel / demo notices (FR13). Extracted to
// consts so the posture gate (`app::posture`) scans them via `USER_FACING_TEMPLATES`; the
// interpolated `{…}` values (fixture ids, engine deviation numbers) are data, never
// scanned. The deviation values themselves are `core::golden::GoldenDeviation` strings, gated in
// `core`'s own posture test.
const MSG_FIXTURE_UNREADABLE: &str = "fixture illisible";
const MSG_DEMO_MISSING: &str = "étude de démonstration introuvable";
const MSG_DEMO_UNREADABLE: &str = "étude de démonstration illisible";
/// Scan-only template mirroring the deviation `format!` in [`run`] (the `{}` holes are data).
#[cfg(test)]
const MSG_DEVIATION_TEMPLATE: &str = "{path} : attendu {expected}, obtenu {actual}";

/// The verify panel's user-facing prose, for the FR13 posture gate. Keep in sync with the
/// `format!`/const sites in [`run`] and [`demo_study`].
#[cfg(test)]
pub(crate) const USER_FACING_TEMPLATES: &[&str] = &[
    MSG_FIXTURE_UNREADABLE,
    MSG_DEMO_MISSING,
    MSG_DEMO_UNREADABLE,
    MSG_DEVIATION_TEMPLATE,
];

/// One fixture's runtime verification line (FR9).
#[derive(Debug, Clone)]
pub struct FixtureResult {
    pub id: String,
    pub passed: bool,
    /// One already-human-readable line per deviation (or a parse-error note). Empty when passed.
    pub deviations: Vec<String>,
}

/// The "verify engine" report (FR9): the method identity + a per-fixture pass/deviation roll-up.
#[derive(Debug, Clone)]
pub struct VerifyReport {
    pub method_version: String,
    pub method_fingerprint: String,
    pub results: Vec<FixtureResult>,
    pub passed_count: usize,
    pub total: usize,
}

impl VerifyReport {
    pub fn all_passed(&self) -> bool {
        self.passed_count == self.total
    }
}

/// Replay every bundled golden fixture through `core::golden::check_all` and fold into a report.
/// A fixture that fails to parse is itself reported as a non-passing result (never a panic / silent
/// skip), so the gate can never appear green by losing a fixture.
pub fn run() -> VerifyReport {
    let mut parsed: Vec<GoldenStudy> = Vec::new();
    let mut results: Vec<FixtureResult> = Vec::new();
    for (id, json) in GOLDEN_FIXTURES {
        match serde_json::from_str::<GoldenStudy>(json) {
            Ok(study) => parsed.push(study),
            Err(error) => {
                // The serde text is logged, never shown (2026-09-26: no raw error Display in
                // a user-visible string).
                tracing::warn!(fixture = %id, %error, "golden fixture unreadable");
                results.push(FixtureResult {
                    id: (*id).to_string(),
                    passed: false,
                    deviations: vec![MSG_FIXTURE_UNREADABLE.to_string()],
                });
            }
        }
    }
    for report in check_all(&parsed) {
        results.push(FixtureResult {
            id: report.id,
            passed: report.passed,
            deviations: report
                .deviations
                .into_iter()
                .map(|d| format!("{} : attendu {}, obtenu {}", d.path, d.expected, d.actual))
                .collect(),
        });
    }
    // Deterministic order by fixture id (parse-failures and check-results merged).
    results.sort_by(|a, b| a.id.cmp(&b.id));
    let passed_count = results.iter().filter(|r| r.passed).count();
    let total = results.len();
    VerifyReport {
        method_version: METHOD_VERSION.to_string(),
        method_fingerprint: method::method_fingerprint(),
        results,
        passed_count,
        total,
    }
}

/// Build the read-only demonstration `Study` (FR62) from the bundled worked-example fixture. The
/// returned study is **never persisted** — the caller renders it in memory with `current_study ==
/// None`, so every edit rail no-ops and the journal is untouched. `Err` only if the bundled fixture
/// somehow fails to parse (a build/packaging error, surfaced as a neutral notice, never a panic).
pub fn demo_study() -> Result<Study, String> {
    let json = GOLDEN_FIXTURES
        .iter()
        .find(|(id, _)| *id == DEMO_FIXTURE_ID)
        .map(|(_, json)| *json)
        .ok_or_else(|| MSG_DEMO_MISSING.to_string())?;
    // A fixed, deterministic identity — the demo is in-memory only, so its id need not be unique in
    // any journal.
    study_from_golden(json, Uuid::from_u128(0x0DE_0000), Uuid::nil(), "DÉMO").map_err(|error| {
        tracing::warn!(%error, "demo study unreadable");
        MSG_DEMO_UNREADABLE.to_string()
    })
}

/// Convert one bundled golden fixture into a contract [`Study`] with the given identity — the
/// demo's conversion, and the one the Story 8.3 metamorphic suite runs over every fixture. Cells
/// carry a provider provenance (reference data), Present coverage, Current.
fn study_from_golden(
    json: &str,
    id: Uuid,
    journal_id: Uuid,
    ticker: &str,
) -> Result<Study, serde_json::Error> {
    let fixture: GoldenStudy = serde_json::from_str(json)?;
    let provenance = Provenance {
        ai_origin: None,
        source: Source::Provider,
        logical_version: 0,
        timestamp: Timestamp("2026-01-01T00:00:00Z".to_string()),
        hash_of_dependencies: "demo".to_string(),
    };
    let years = fixture
        .input
        .years
        .iter()
        .map(|y| year_to_data(y, &provenance))
        .collect();
    let mut study = Study::new(
        id,
        journal_id,
        ticker,
        fixture.input.native_currency.clone(),
        judgment_from(&fixture.input.judgment),
        Timestamp("2026-01-01T00:00:00Z".to_string()),
    );
    study.years = years;
    Ok(study)
}

/// A present cell holding `value` (reference/provider provenance, `Present` coverage, `Current`).
fn present_cell(value: Money, provenance: &Provenance) -> Cell {
    Cell {
        value: Some(value),
        source: provenance.source,
        freshness: Freshness::Current,
        review: Review::None,
        coverage: Coverage::Present,
        provenance: provenance.clone(),
        pending: None,
    }
}

/// Map one fixture year → a `YearData`: each present `FixtureAmount` becomes a present cell, each
/// absent one a to-fill gap (the four load-bearing fields are always present cells; the three
/// optional columns are `Some(present)` / `None`).
fn year_to_data(y: &FixtureYear, provenance: &Provenance) -> YearData {
    let required = |amount: &Option<FixtureAmount>| match amount {
        Some(a) => present_cell(Money::from(a.value), provenance),
        None => tofill_cell(provenance.clone()),
    };
    let optional = |amount: &Option<FixtureAmount>| {
        amount
            .as_ref()
            .map(|a| present_cell(Money::from(a.value), provenance))
    };
    YearData {
        year: y.year,
        sales: required(&y.sales),
        eps: required(&y.eps),
        high_price: required(&y.high_price),
        low_price: required(&y.low_price),
        dividend_per_share: optional(&y.dividend_per_share),
        pre_tax_profit: optional(&y.pre_tax_profit),
        book_value_per_share: optional(&y.book_value_per_share),
    }
}

/// Map the fixture judgment → `contract::Judgment` (fields match one-for-one).
fn judgment_from(j: &FixtureJudgment) -> Judgment {
    Judgment {
        ai_placed: Default::default(),
        estimated_high_eps: j.estimated_high_eps.map(Money::from),
        estimated_low_eps: j.estimated_low_eps.map(Money::from),
        projected_sales_growth_pct: j.projected_sales_growth_pct.map(Money::from),
        projected_eps_growth_pct: j.projected_eps_growth_pct.map(Money::from),
        judged_avg_high_pe: j.judged_avg_high_pe.map(Money::from),
        judged_avg_low_pe: j.judged_avg_low_pe.map(Money::from),
        forecast_low_option: match j.forecast_low_option {
            FixtureForecastLowOption::AvgLowPeTimesEps => ForecastLowOption::AvgLowPeTimesEps,
            FixtureForecastLowOption::AvgLowPriceLast5y => ForecastLowOption::AvgLowPriceLast5y,
            FixtureForecastLowOption::RecentSevereLow => ForecastLowOption::RecentSevereLow,
            FixtureForecastLowOption::DividendSupported => ForecastLowOption::DividendSupported,
        },
        recent_severe_low: j.recent_severe_low.map(Money::from),
        current_price: j.current_price.map(Money::from),
        present_full_year_dividend: j.present_full_year_dividend.map(Money::from),
        // Issue #113: golden fixtures carry no TTM EPS — current P/E stays unknown, as before.
        ttm_eps: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_run_passes_every_bundled_fixture_at_runtime() {
        let report = run();
        assert_eq!(report.total, 11, "all 11 bundled fixtures are replayed");
        assert!(
            report.all_passed(),
            "the app's runtime verify path passes every golden (mirrors the golden gate); failures: {:?}",
            report
                .results
                .iter()
                .filter(|r| !r.passed)
                .collect::<Vec<_>>()
        );
        assert_eq!(report.passed_count, 11);
        assert_eq!(report.method_version, METHOD_VERSION);
        assert!(
            !report.method_fingerprint.is_empty(),
            "the method fingerprint is surfaced"
        );
    }

    #[test]
    fn demo_study_converts_the_worked_example_faithfully_and_yields_a_frame() {
        let study = demo_study().expect("the bundled demo fixture converts");
        assert_eq!(study.security_ticker, "DÉMO");
        assert!(
            !study.years.is_empty(),
            "the demo carries the worked-example years"
        );
        // Every load-bearing cell that the fixture filled is Present (not a gap) — the conversion is
        // faithful; and the engine builds a coherent frame from it (the demo renders a real verdict).
        let present_eps = study
            .years
            .iter()
            .filter(|y| y.eps.coverage == Coverage::Present)
            .count();
        assert!(present_eps > 0, "the worked example has filled EPS years");
        let snapshot = crate::viewmodel::engine::build_snapshot(&study);
        assert!(
            snapshot.is_ok(),
            "the demo study normalizes + computes a coherent frame"
        );
    }

    /// Story 8.3 AC 12 (FR72, "a pending draft changes no computed output"): for EVERY golden
    /// fixture, the study read through the MCP access surface yields the same snapshot — through
    /// the app's own `report::form::build_snapshot` — with and without pending drafts of every kind.
    #[test]
    fn pending_drafts_of_every_kind_change_no_computed_output_of_any_golden_study() {
        use steadyinvest_contract::{DraftKind, DraftOrigin, DraftTarget};
        use steadyinvest_persistence::{DraftSubmission, Journal, McpAccess, Page};

        let journal_id = Uuid::from_u128(0x83_6000);
        let now = Timestamp("2026-09-28T12:00:00Z".to_string());
        for (n, (fixture_id, json)) in GOLDEN_FIXTURES.iter().enumerate() {
            let n = n as u128;
            let dir = tempfile::TempDir::new().expect("tempdir");
            let path = dir.path().join("dossier.db");
            let study_id = Uuid::from_u128(0x83_7000 + n);
            let study = study_from_golden(json, study_id, journal_id, "GOLD")
                .unwrap_or_else(|e| panic!("{fixture_id} converts: {e}"));
            {
                let mut j = Journal::create(&path, journal_id, &now).expect("create");
                j.put_study_with_history(&study, &now).expect("store");
            }
            let access = McpAccess::at(&path);
            let snapshot = |label: &str| {
                let read = access
                    .read_study(study_id)
                    .expect("read")
                    .unwrap_or_else(|| panic!("{fixture_id}: {label} study present"));
                format!(
                    "{:?}",
                    crate::viewmodel::engine::build_snapshot(&read.study)
                )
            };
            let without = snapshot("before");

            let dossier = access.identity().expect("identity");
            let base = |k: u128, kind: DraftKind| DraftSubmission {
                id: Uuid::from_u128(0x83_8000 + n * 16 + k),
                created_at: now.clone(),
                kind,
                study_id: Some(study_id),
                security_ticker: None,
                native_currency: None,
                company_name: None,
                target: None,
                proposed_value: None,
                note_text: None,
                comment: "proposition de test".to_string(),
                origin: DraftOrigin {
                    client: "test".to_string(),
                    model: "test".to_string(),
                },
                dossier: dossier.clone(),
                method_version: METHOD_VERSION.to_string(),
            };
            let year = study.years.first().map(|y| y.year).expect("a year");
            let drafts = [
                DraftSubmission {
                    note_text: Some("note".to_string()),
                    ..base(1, DraftKind::Note)
                },
                DraftSubmission {
                    target: Some(DraftTarget::Cell {
                        fiscal_year: year,
                        field: "eps".to_string(),
                    }),
                    proposed_value: Some("999.5".to_string()),
                    ..base(2, DraftKind::Cell)
                },
                DraftSubmission {
                    target: Some(DraftTarget::Cell {
                        fiscal_year: year,
                        field: "high_price".to_string(),
                    }),
                    proposed_value: Some("0.01".to_string()),
                    ..base(3, DraftKind::Cell)
                },
                DraftSubmission {
                    target: Some(DraftTarget::Judgment {
                        field: "judged_avg_high_pe".to_string(),
                    }),
                    proposed_value: Some("80".to_string()),
                    ..base(4, DraftKind::Judgment)
                },
                DraftSubmission {
                    target: Some(DraftTarget::Judgment {
                        field: "estimated_high_eps".to_string(),
                    }),
                    proposed_value: Some("-3".to_string()),
                    ..base(5, DraftKind::Judgment)
                },
                DraftSubmission {
                    target: Some(DraftTarget::Judgment {
                        field: "forecast_low_option".to_string(),
                    }),
                    proposed_value: Some("dividend_supported".to_string()),
                    ..base(6, DraftKind::Judgment)
                },
                DraftSubmission {
                    study_id: None,
                    security_ticker: Some("NEWCO".to_string()),
                    native_currency: Some("USD".to_string()),
                    ..base(7, DraftKind::Study)
                },
            ];
            for d in &drafts {
                access
                    .submit_draft(d)
                    .unwrap_or_else(|e| panic!("{fixture_id}: {:?} accepted: {e:?}", d.kind));
            }
            assert_eq!(
                access
                    .list_drafts(Default::default(), Page::first(50))
                    .expect("drafts")
                    .total,
                drafts.len() as u64
            );
            assert_eq!(
                snapshot("after"),
                without,
                "{fixture_id}: a pending draft changed a computed output"
            );
        }
    }
}
