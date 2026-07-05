//! AC 6 — the two CI gates over the persisted shapes:
//!
//! 1. **Pinned JSON snapshot**: the canonical `Study` serializes to a byte-pinned expected string
//!    committed below. Any change to a persisted struct breaks it, forcing a conscious
//!    `SCHEMA_VERSION` bump + migration + new corpus file — never silent drift.
//! 2. **Frozen binary corpus** `tests/corpus/v1.db`: generated ONCE by the `#[ignore]`d generator
//!    test, committed, then append-only forever (see `tests/corpus/README.md`). The gate test
//!    opens a copy, asserts `user_version == 1` and reads back the canonical study exactly.

use rusqlite::Connection;
use std::path::PathBuf;
use steadyinvest_contract::{
    Cell, Coverage, ForecastLowOption, Freshness, Judgment, Money, Provenance, Review, Source,
    Study, Timestamp, YearData, SCHEMA_VERSION,
};
use steadyinvest_persistence::Journal;
use tempfile::TempDir;
use uuid::Uuid;

// ── The canonical study: fixed UUIDs/timestamps, full YearData incl. a None-valued cell,
//    a scale-bearing Money ("3.0"), a rationale ──

const CANONICAL_JOURNAL_ID: &str = "11111111-1111-4111-8111-111111111111";
const CANONICAL_STUDY_ID: &str = "22222222-2222-4222-8222-222222222222";
const CANONICAL_JOURNAL_CREATED_AT: &str = "2026-06-12T00:00:00Z";

fn money(s: &str) -> Money {
    serde_json::from_str(&format!("\"{s}\"")).expect("canonical decimal string parses as Money")
}

fn ts(s: &str) -> Timestamp {
    Timestamp(s.to_string())
}

fn cell(value: Option<&str>, review: Review) -> Cell {
    Cell {
        value: value.map(money),
        source: Source::Manual,
        freshness: Freshness::Current,
        review,
        coverage: if value.is_some() {
            Coverage::Present
        } else {
            Coverage::ToFill
        },
        provenance: Provenance {
            source: Source::Manual,
            logical_version: 0,
            timestamp: ts("2026-06-12T08:00:00Z"),
            hash_of_dependencies: "deadbeef".to_string(),
        },
        pending: None,
    }
}

fn canonical_study() -> Study {
    Study {
        id: Uuid::parse_str(CANONICAL_STUDY_ID).expect("canonical study UUID parses"),
        journal_id: Uuid::parse_str(CANONICAL_JOURNAL_ID).expect("canonical journal UUID parses"),
        security_ticker: "NESN".to_string(),
        native_currency: "CHF".to_string(),
        years: vec![
            YearData {
                year: 2021,
                sales: cell(Some("3.0"), Review::Validated), // scale-bearing: "3.0", not "3"
                eps: cell(Some("-0.07"), Review::Validated),
                high_price: cell(Some("141.50"), Review::ToReview),
                low_price: cell(Some("98"), Review::None),
                dividend_per_share: Some(cell(None, Review::None)), // the None-valued cell
                pre_tax_profit: Some(cell(Some("200"), Review::Validated)),
                book_value_per_share: Some(cell(Some("12.34"), Review::Validated)),
            },
            YearData {
                year: 2022,
                sales: cell(Some("1322.500000"), Review::Validated),
                eps: cell(Some("3.15"), Review::Validated),
                high_price: cell(Some("150"), Review::Validated),
                low_price: cell(Some("101.25"), Review::Validated),
                dividend_per_share: None,
                pre_tax_profit: None,
                book_value_per_share: None,
            },
        ],
        judgment: Judgment {
            estimated_high_eps: Some(money("5.20")),
            estimated_low_eps: Some(money("2.10")),
            // The four issue-#14 fields stay `None` in the frozen corpus: the committed v1.db
            // payload predates them, so `serde(default)` deserializes them to `None` and the
            // frozen read-back below still compares equal — the live backward-compat proof. The
            // populated round-trip is exercised elsewhere (journal_roundtrip.rs).
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: Some(money("18")),
            judged_avg_low_pe: Some(money("11.5")),
            forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
            recent_severe_low: None,
            current_price: Some(money("104.00")),
            present_full_year_dividend: None,
            ttm_eps: None,
        },
        rationale: Some("Margin trend noted; demand steady.".to_string()),
        created_at: ts("2026-06-12T08:30:00Z"),
        schema_version: SCHEMA_VERSION,
    }
}

fn corpus_path(version: u32) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus")
        .join(format!("v{version}.db"))
}

// ── Gate 1: the pinned JSON snapshot ──

/// The byte-pinned serde JSON of [`canonical_study`], captured once and committed. Field order is
/// struct declaration order (serde), `Money` strings preserve scale.
///
/// Story 2.2 (issue #14) added four `#[serde(default)] Option<Money>` fields to `Judgment`
/// (`projected_sales_growth_pct`, `projected_eps_growth_pct`, `recent_severe_low`,
/// `present_full_year_dividend`). They render as `"…":null` here (they are `None` in the canonical
/// study, and `Option` without `skip_serializing_if` serializes `None` as `null` — consistent with
/// `dividend_per_share: None` above). This is an **additive, optional** change: by the contract's
/// own forward-compat policy (`contract/src/lib.rs`: new *fields* tolerated in both directions) it
/// is NOT a `SCHEMA_VERSION` bump and needs no migration or new corpus file — the frozen v1.db (its
/// payload predates the fields) still reads back equal via `serde(default)`. Only this pinned string
/// was re-captured to the new canonical shape.
///
/// Story 3.4 (FR22) added a `#[serde(default)] Option<PendingProvider>` field `pending` to `Cell`
/// (a divergent provider value preserved alongside a manual one). Same additive treatment: each cell
/// gains a trailing `"pending":null` (None in the canonical study), NOT a `SCHEMA_VERSION` bump; the
/// frozen v1.db still reads back equal via `serde(default)`; only this pinned string was re-captured.
const PINNED_CANONICAL_STUDY_JSON: &str = r#"{"id":"22222222-2222-4222-8222-222222222222","journal_id":"11111111-1111-4111-8111-111111111111","security_ticker":"NESN","native_currency":"CHF","years":[{"year":2021,"sales":{"value":"3.0","source":"manual","freshness":"current","review":"validated","coverage":"present","provenance":{"source":"manual","logical_version":0,"timestamp":"2026-06-12T08:00:00Z","hash_of_dependencies":"deadbeef"},"pending":null},"eps":{"value":"-0.07","source":"manual","freshness":"current","review":"validated","coverage":"present","provenance":{"source":"manual","logical_version":0,"timestamp":"2026-06-12T08:00:00Z","hash_of_dependencies":"deadbeef"},"pending":null},"high_price":{"value":"141.50","source":"manual","freshness":"current","review":"to_review","coverage":"present","provenance":{"source":"manual","logical_version":0,"timestamp":"2026-06-12T08:00:00Z","hash_of_dependencies":"deadbeef"},"pending":null},"low_price":{"value":"98","source":"manual","freshness":"current","review":"none","coverage":"present","provenance":{"source":"manual","logical_version":0,"timestamp":"2026-06-12T08:00:00Z","hash_of_dependencies":"deadbeef"},"pending":null},"dividend_per_share":{"value":null,"source":"manual","freshness":"current","review":"none","coverage":"to_fill","provenance":{"source":"manual","logical_version":0,"timestamp":"2026-06-12T08:00:00Z","hash_of_dependencies":"deadbeef"},"pending":null},"pre_tax_profit":{"value":"200","source":"manual","freshness":"current","review":"validated","coverage":"present","provenance":{"source":"manual","logical_version":0,"timestamp":"2026-06-12T08:00:00Z","hash_of_dependencies":"deadbeef"},"pending":null},"book_value_per_share":{"value":"12.34","source":"manual","freshness":"current","review":"validated","coverage":"present","provenance":{"source":"manual","logical_version":0,"timestamp":"2026-06-12T08:00:00Z","hash_of_dependencies":"deadbeef"},"pending":null}},{"year":2022,"sales":{"value":"1322.500000","source":"manual","freshness":"current","review":"validated","coverage":"present","provenance":{"source":"manual","logical_version":0,"timestamp":"2026-06-12T08:00:00Z","hash_of_dependencies":"deadbeef"},"pending":null},"eps":{"value":"3.15","source":"manual","freshness":"current","review":"validated","coverage":"present","provenance":{"source":"manual","logical_version":0,"timestamp":"2026-06-12T08:00:00Z","hash_of_dependencies":"deadbeef"},"pending":null},"high_price":{"value":"150","source":"manual","freshness":"current","review":"validated","coverage":"present","provenance":{"source":"manual","logical_version":0,"timestamp":"2026-06-12T08:00:00Z","hash_of_dependencies":"deadbeef"},"pending":null},"low_price":{"value":"101.25","source":"manual","freshness":"current","review":"validated","coverage":"present","provenance":{"source":"manual","logical_version":0,"timestamp":"2026-06-12T08:00:00Z","hash_of_dependencies":"deadbeef"},"pending":null},"dividend_per_share":null,"pre_tax_profit":null,"book_value_per_share":null}],"judgment":{"estimated_high_eps":"5.20","estimated_low_eps":"2.10","projected_sales_growth_pct":null,"projected_eps_growth_pct":null,"judged_avg_high_pe":"18","judged_avg_low_pe":"11.5","forecast_low_option":"avg_low_pe_times_eps","recent_severe_low":null,"current_price":"104.00","ttm_eps":null,"present_full_year_dividend":null},"rationale":"Margin trend noted; demand steady.","created_at":"2026-06-12T08:30:00Z","schema_version":1}"#;

#[test]
fn persisted_study_shape_is_byte_pinned() {
    let json = serde_json::to_string(&canonical_study()).expect("the canonical study serializes");
    assert_eq!(
        json, PINNED_CANONICAL_STUDY_JSON,
        "persisted Study shape changed — this requires a SCHEMA_VERSION bump + a migration + a \
         new corpus file v{{N+1}}.db, see persistence/tests/corpus/README.md (never edit or \
         regenerate existing corpus files)"
    );
}

// ── Generator (run once, commit the file, never again) ──

/// Generates `tests/corpus/v1.db` from the canonical study. `#[ignore]`d: run explicitly with
/// `cargo test -p steadyinvest-persistence --test corpus_gate -- --ignored`, commit the file,
/// never run again (it refuses to overwrite — corpus files are append-only forever).
#[test]
#[ignore = "one-shot corpus generator — the committed v1.db is frozen"]
fn generate_corpus_v1() {
    let dest = corpus_path(1);
    assert!(
        !dest.exists(),
        "{} already exists — corpus files are append-only, never regenerated; a schema change \
         adds v{{N+1}}.db beside it (see tests/corpus/README.md)",
        dest.display()
    );

    // Build in a TempDir (never write a live DB inside the sync-watched repo tree), close
    // cleanly, then copy the closed file.
    let dir = TempDir::new().expect("tempdir");
    let tmp_db = dir.path().join("v1.db");
    {
        let mut journal = Journal::create(
            &tmp_db,
            Uuid::parse_str(CANONICAL_JOURNAL_ID).expect("canonical journal UUID parses"),
            &ts(CANONICAL_JOURNAL_CREATED_AT),
        )
        .expect("corpus journal creates");
        journal
            .put_study(&canonical_study())
            .expect("canonical study writes");
    } // drop = last connection closes; WAL checkpoints and removes -wal/-shm sidecars

    std::fs::create_dir_all(dest.parent().expect("corpus dir has a parent"))
        .expect("corpus dir exists");
    std::fs::copy(&tmp_db, &dest).expect("closed corpus file copies into the repo");
    eprintln!("corpus written: {} — commit it now", dest.display());
}

// ── Gate 2: the frozen corpus opens and reads back exactly ──

#[test]
fn frozen_corpus_v1_opens_and_reads_back_the_canonical_study() {
    let src = corpus_path(1);
    assert!(
        src.exists(),
        "{} is absent — if it exists locally but not in git, the .gitignore `*.db` rule is \
         swallowing it (the `!persistence/tests/corpus/*.db` exception is load-bearing)",
        src.display()
    );

    // Copy out first: keeps the frozen fixture untouched and the -wal/-shm sidecars out of the
    // repo tree.
    let dir = TempDir::new().expect("tempdir");
    let work = dir.path().join("v1.db");
    std::fs::copy(&src, &work).expect("corpus copies to a TempDir");

    let v: i64 = {
        let conn = Connection::open(&work).expect("raw open");
        conn.query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("user_version reads")
    };
    assert_eq!(v, 1, "corpus v1.db carries user_version 1");

    let journal = Journal::open(&work).expect("the frozen v1 journal opens");
    assert!(!journal.is_read_only());
    assert_eq!(
        journal.id(),
        Uuid::parse_str(CANONICAL_JOURNAL_ID).expect("canonical journal UUID parses")
    );
    assert_eq!(
        journal.logical_version().expect("version reads"),
        1,
        "one mutation (the canonical put_study) is recorded"
    );

    let back = journal
        .get_study(Uuid::parse_str(CANONICAL_STUDY_ID).expect("canonical study UUID parses"))
        .expect("the canonical study reads")
        .expect("the canonical study is present");
    assert_eq!(
        back,
        canonical_study(),
        "the journal written at v1 no longer reads back equal — a persisted shape changed \
         without its SCHEMA_VERSION bump + migration (see tests/corpus/README.md)"
    );
}
