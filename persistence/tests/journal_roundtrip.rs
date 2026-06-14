//! Integration tests — journal identity, atomic Study round-trip, logical-version heartbeat.
//!
//! File-level behaviours need real files: everything runs in a `tempfile::TempDir` (`/tmp`,
//! outside the Synology Drive sync watch — never create test DBs inside the repo tree).

use rusqlite::Connection;
use steadyinvest_contract::{
    Cell, Coverage, ForecastLowOption, Freshness, Judgment, Money, Provenance, Review, Source,
    Study, Timestamp, YearData,
};
use steadyinvest_persistence::{Error, Journal};
use tempfile::TempDir;
use uuid::Uuid;

// ── Builders (fixed values — identity and time are caller-supplied, full determinism) ──

fn money(s: &str) -> Money {
    serde_json::from_str(&format!("\"{s}\"")).expect("canonical decimal string parses as Money")
}

fn ts(s: &str) -> Timestamp {
    Timestamp(s.to_string())
}

fn cell(value: Option<&str>) -> Cell {
    Cell {
        value: value.map(money),
        source: Source::Manual,
        freshness: Freshness::Current,
        review: Review::Validated,
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
    }
}

fn judgment() -> Judgment {
    Judgment {
        estimated_high_eps: Some(money("5.20")),
        estimated_low_eps: Some(money("2.10")),
        // The four issue-#14 fields, all populated — so the round-trip tests below prove they
        // survive save/reload instead of being silently lost (the exact harm #14 documents).
        projected_sales_growth_pct: Some(money("8.5")),
        projected_eps_growth_pct: Some(money("11.0")),
        judged_avg_high_pe: Some(money("18")),
        judged_avg_low_pe: Some(money("11.5")),
        forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
        recent_severe_low: Some(money("72.40")),
        current_price: Some(money("104.00")),
        present_full_year_dividend: Some(money("2.95")),
    }
}

/// A varied, fully-exercising study: full year (incl. a `None`-valued cell), lean year,
/// rationale, scale-bearing Money values.
fn study(id: u128, journal_id: Uuid, ticker: &str) -> Study {
    Study {
        id: Uuid::from_u128(id),
        journal_id,
        security_ticker: ticker.to_string(),
        native_currency: "CHF".to_string(),
        years: vec![
            YearData {
                year: 2021,
                sales: cell(Some("3.0")),
                eps: cell(Some("-0.07")),
                high_price: cell(Some("141.50")),
                low_price: cell(Some("98")),
                dividend_per_share: Some(cell(None)), // unknown stays unknown — never 0
                pre_tax_profit: Some(cell(Some("200"))),
                book_value_per_share: Some(cell(Some("12.34"))),
            },
            YearData {
                year: 2022,
                sales: cell(Some("1322.500000")),
                eps: cell(Some("3.15")),
                high_price: cell(Some("150")),
                low_price: cell(Some("101.25")),
                dividend_per_share: None,
                pre_tax_profit: None,
                book_value_per_share: None,
            },
        ],
        judgment: judgment(),
        rationale: Some("Margin trend noted; demand steady.".to_string()),
        created_at: ts("2026-06-12T08:30:00Z"),
        schema_version: steadyinvest_contract::SCHEMA_VERSION,
    }
}

const JOURNAL_TS: &str = "2026-06-12T00:00:00Z";

fn new_journal(dir: &TempDir, journal_id: Uuid) -> Journal {
    Journal::create(dir.path().join("journal.db"), journal_id, &ts(JOURNAL_TS))
        .expect("a fresh journal creates")
}

// ── Identity & lifecycle ──

#[test]
fn create_sets_identity_and_logical_version_zero() {
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0xA);
    let journal = new_journal(&dir, jid);
    assert_eq!(journal.id(), jid, "identity is the caller-supplied UUID");
    assert_eq!(
        journal.logical_version().expect("version reads"),
        0,
        "logical_version starts at 0 = created, never mutated"
    );
    assert!(!journal.is_read_only());
}

#[test]
fn create_refuses_an_existing_path() {
    let dir = TempDir::new().expect("tempdir");
    let _journal = new_journal(&dir, Uuid::from_u128(0xA));
    let err = Journal::create(
        dir.path().join("journal.db"),
        Uuid::from_u128(0xB),
        &ts(JOURNAL_TS),
    )
    .expect_err("create never overwrites an existing file");
    assert!(matches!(err, Error::JournalExists(_)), "got {err:?}");
}

#[test]
fn open_missing_file_is_an_error_never_a_silent_empty_journal() {
    let dir = TempDir::new().expect("tempdir");
    let err = Journal::open(dir.path().join("absent.db")).expect_err("open requires the file");
    assert!(matches!(err, Error::Sqlite(_)), "got {err:?}");
}

#[test]
fn reopen_preserves_identity_and_version() {
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0xBEEF);
    {
        let mut journal = new_journal(&dir, jid);
        journal
            .put_study(&study(1, jid, "NESN"))
            .expect("study writes");
    }
    let journal = Journal::open(dir.path().join("journal.db")).expect("reopen");
    assert_eq!(journal.id(), jid);
    assert_eq!(journal.logical_version().expect("version reads"), 1);
}

#[test]
fn read_write_open_applies_wal_and_pragmas() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("journal.db");
    drop(Journal::create(&path, Uuid::from_u128(0xA), &ts(JOURNAL_TS)).expect("create"));
    // journal_mode=WAL is persistent in the file — visible from a fresh raw connection.
    let conn = Connection::open(&path).expect("raw open");
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .expect("journal_mode reads");
    assert_eq!(mode.to_lowercase(), "wal");
}

// ── Atomic round-trip (AC 3) ──

#[test]
fn varied_studies_round_trip_exactly() {
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0xC0FFEE);
    let mut journal = new_journal(&dir, jid);

    let full = study(1, jid, "NESN");
    let mut no_years = study(2, jid, "ROG");
    no_years.years.clear();
    no_years.rationale = None;
    let mut bare_judgment = study(3, jid, "AAPL");
    bare_judgment.native_currency = "USD".to_string();
    bare_judgment.judgment = Judgment {
        estimated_high_eps: None,
        estimated_low_eps: None,
        projected_sales_growth_pct: None,
        projected_eps_growth_pct: None,
        judged_avg_high_pe: None,
        judged_avg_low_pe: None,
        forecast_low_option: ForecastLowOption::DividendSupported,
        recent_severe_low: None,
        current_price: None,
        present_full_year_dividend: None,
    };

    for s in [&full, &no_years, &bare_judgment] {
        journal.put_study(s).expect("study writes");
        let back = journal
            .get_study(s.id)
            .expect("study reads")
            .expect("study exists");
        assert_eq!(back, *s, "get(put(s)) == s for {}", s.security_ticker);
    }
}

#[test]
fn money_scale_survives_byte_for_byte_in_the_stored_payload() {
    // `Money` Eq is value-based (Money("3.0") == Money("3")) — struct equality cannot prove
    // scale preservation. The proof is the stored payload string itself.
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0xD);
    let mut journal = new_journal(&dir, jid);
    let s = study(1, jid, "NESN");
    journal.put_study(&s).expect("study writes");
    drop(journal);

    let conn = Connection::open(dir.path().join("journal.db")).expect("raw open");
    let payload: String = conn
        .query_row(
            "SELECT payload FROM studies WHERE id = ?1",
            [s.id.to_string()],
            |r| r.get(0),
        )
        .expect("payload reads");
    assert!(
        payload.contains("\"3.0\""),
        "scale \"3.0\" did not survive byte-for-byte in the stored payload"
    );
    assert!(
        payload.contains("\"1322.500000\""),
        "scale \"1322.500000\" did not survive byte-for-byte in the stored payload"
    );
    assert!(
        payload.contains("\"-0.07\""),
        "negative scale-2 value did not survive byte-for-byte"
    );
}

#[test]
fn unknown_cell_round_trips_as_unknown_never_zero() {
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0xE);
    let mut journal = new_journal(&dir, jid);
    let s = study(1, jid, "NESN");
    journal.put_study(&s).expect("study writes");
    let back = journal
        .get_study(s.id)
        .expect("study reads")
        .expect("study exists");
    let dividend = back.years[0]
        .dividend_per_share
        .as_ref()
        .expect("the dividend cell exists");
    assert_eq!(dividend.value, None, "unknown came back as unknown, not 0");
    assert_eq!(dividend.coverage, Coverage::ToFill);
}

#[test]
fn resave_updates_in_place_and_bumps_version() {
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0xF);
    let mut journal = new_journal(&dir, jid);
    let mut s = study(1, jid, "NESN");
    journal.put_study(&s).expect("first write");
    s.rationale = Some("Revised note after the annual report.".to_string());
    journal.put_study(&s).expect("re-save upserts");

    let listed = journal.list_studies().expect("list reads");
    assert_eq!(listed.len(), 1, "re-save updated in place, no second row");
    let back = journal
        .get_study(s.id)
        .expect("study reads")
        .expect("study exists");
    assert_eq!(back.rationale, s.rationale);
    assert_eq!(
        journal.logical_version().expect("version reads"),
        2,
        "each mutation bumps the heartbeat exactly once"
    );
}

#[test]
fn logical_version_strictly_increments_per_mutation() {
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0x10);
    let mut journal = new_journal(&dir, jid);
    for (i, expected) in [(1u128, 1u64), (2, 2), (3, 3)] {
        journal.put_study(&study(i, jid, "NESN")).expect("writes");
        assert_eq!(journal.logical_version().expect("version reads"), expected);
    }
}

#[test]
fn wrong_journal_id_write_is_rejected_and_writes_nothing() {
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0x11);
    let mut journal = new_journal(&dir, jid);
    let foreign = study(1, Uuid::from_u128(0xBAD), "NESN");
    let err = journal
        .put_study(&foreign)
        .expect_err("a study from journal A is never written into journal B");
    assert!(
        matches!(
            err,
            Error::JournalIdentityMismatch { study_journal_id, journal_id }
                if study_journal_id == Uuid::from_u128(0xBAD) && journal_id == jid
        ),
        "got {err:?}"
    );
    assert_eq!(journal.get_study(foreign.id).expect("read works"), None);
    assert_eq!(
        journal.logical_version().expect("version reads"),
        0,
        "the rejected write did not bump the heartbeat"
    );
}

#[test]
fn interrupted_write_leaves_prior_version_and_no_partial_row() {
    // Simulates an interrupted mutation at the SQLite level: the same two statements
    // put_study runs, in a transaction dropped without commit (rusqlite rolls back on drop).
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0x12);
    let path = dir.path().join("journal.db");
    {
        let mut journal = new_journal(&dir, jid);
        journal.put_study(&study(1, jid, "NESN")).expect("writes");
    }

    let mut conn = Connection::open(&path).expect("raw open");
    {
        let tx = conn.transaction().expect("tx begins");
        tx.execute(
            "INSERT INTO studies (id, journal_id, security_ticker, created_at, status,
                                  schema_version, method_version, payload)
             VALUES (?1, ?2, 'GHOST', '2026-06-12T09:00:00Z', 'active', 1, NULL, '{}')",
            [Uuid::from_u128(0xDEAD).to_string(), jid.to_string()],
        )
        .expect("insert inside tx");
        tx.execute(
            "UPDATE journal_meta SET logical_version = logical_version + 1 WHERE id = 1",
            [],
        )
        .expect("bump inside tx");
        // Dropped here without commit — the interruption.
    }
    drop(conn);

    let journal = Journal::open(&path).expect("reopen");
    assert_eq!(
        journal.logical_version().expect("version reads"),
        1,
        "the journal stays at its prior logical_version"
    );
    assert_eq!(
        journal
            .get_study(Uuid::from_u128(0xDEAD))
            .expect("read works"),
        None,
        "no partial row survives the interrupted write"
    );
}

// ── Listing (the Epic 2 dashboard's building block) ──

#[test]
fn list_studies_returns_indexed_columns_without_payload_parse() {
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0x13);
    let mut journal = new_journal(&dir, jid);
    let mut a = study(1, jid, "NESN");
    a.created_at = ts("2026-06-10T00:00:00Z");
    let mut b = study(2, jid, "ROG");
    b.created_at = ts("2026-06-11T00:00:00Z");
    journal.put_study(&b).expect("writes");
    journal.put_study(&a).expect("writes");

    let listed = journal.list_studies().expect("list reads");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, a.id, "ordered by created_at");
    assert_eq!(listed[0].security_ticker, "NESN");
    assert_eq!(listed[0].created_at, a.created_at);
    assert_eq!(listed[0].status, "active");
    assert_eq!(listed[1].id, b.id);
    assert_eq!(listed[1].security_ticker, "ROG");
}

// ── Issue #14: the four judgment fields survive save → close → reopen (AC 3, AC 4) ──

#[test]
fn issue_14_judgment_fields_round_trip_through_a_reopened_journal() {
    // The harm issue #14 documents: a study's FR6 growth judgments and the §4 option (c)/(d)
    // inputs were silently lost on save/reload because `contract::Judgment` could not hold them.
    // Here all four are populated, written, the journal is CLOSED, then reopened in a fresh
    // `Journal::open` (not an in-memory clone) — and every field reads back exactly.
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("journal.db");
    let jid = Uuid::from_u128(0x14);

    let s = study(1, jid, "NESN");
    // Guard the test's own premise: the fixture really populates all four issue-#14 fields.
    assert!(s.judgment.projected_sales_growth_pct.is_some());
    assert!(s.judgment.projected_eps_growth_pct.is_some());
    assert!(s.judgment.recent_severe_low.is_some());
    assert!(s.judgment.present_full_year_dividend.is_some());

    {
        let mut journal =
            Journal::create(&path, jid, &ts(JOURNAL_TS)).expect("a fresh journal creates");
        journal.put_study(&s).expect("study writes");
    } // close: last connection drops, WAL checkpoints

    let journal = Journal::open(&path).expect("the journal reopens on disk");
    let back = journal
        .get_study(s.id)
        .expect("study reads")
        .expect("study exists");
    assert_eq!(
        back.judgment, s.judgment,
        "the full Judgment — including the four issue-#14 fields — survives a disk round-trip"
    );
    // Field-by-field, so a regression names the exact field that was dropped.
    assert_eq!(back.judgment.projected_sales_growth_pct, Some(money("8.5")));
    assert_eq!(back.judgment.projected_eps_growth_pct, Some(money("11.0")));
    assert_eq!(back.judgment.recent_severe_low, Some(money("72.40")));
    assert_eq!(
        back.judgment.present_full_year_dividend,
        Some(money("2.95"))
    );
    // And the whole study is value-equal (AC 3: nothing dropped, defaulted-away, or coerced).
    assert_eq!(
        back, s,
        "the reopened study is value-equal to what was saved"
    );
}

// ── Story 2.12: archive (soft status) & delete (hard, FK-safe time-series purge) ──

fn status_of(journal: &Journal, id: Uuid) -> Option<String> {
    journal
        .list_studies()
        .expect("list")
        .into_iter()
        .find(|s| s.id == id)
        .map(|s| s.status)
}

#[test]
fn set_study_status_archives_and_unarchives_reversibly() {
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0x21);
    let mut journal = new_journal(&dir, jid);
    let s = study(1, jid, "NESN");
    journal.put_study(&s).expect("study writes");
    assert_eq!(status_of(&journal, s.id).as_deref(), Some("active"));

    journal.set_study_status(s.id, "archived").expect("archive");
    assert_eq!(
        status_of(&journal, s.id).as_deref(),
        Some("archived"),
        "archive flips the status column"
    );
    // The blob payload is untouched — archive is a pure status change, fully reversible.
    assert_eq!(journal.get_study(s.id).expect("read").expect("exists"), s);

    journal.set_study_status(s.id, "active").expect("un-archive");
    assert_eq!(
        status_of(&journal, s.id).as_deref(),
        Some("active"),
        "un-archive restores active"
    );
}

#[test]
fn delete_study_removes_the_row_and_its_judgments_leaving_others_intact() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("journal.db");
    let jid = Uuid::from_u128(0x22);
    let keep = study(1, jid, "KEEP");
    let drop_it = study(2, jid, "DROPME");
    {
        let mut journal = Journal::create(&path, jid, &ts(JOURNAL_TS)).expect("create");
        journal.put_study(&keep).expect("keep writes");
        journal.put_study(&drop_it).expect("dropme writes");
    }
    // Seed an FR51 `judgments` row for each study via a raw connection (the app doesn't write the
    // time-series yet — issue #34) — proves the delete is FK-safe and purges only the target's rows.
    {
        let conn = Connection::open(&path).expect("raw conn");
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        for (jrow, study_id) in [(0x100u128, keep.id), (0x101, drop_it.id), (0x102, drop_it.id)] {
            conn.execute(
                "INSERT INTO judgments (id, study_id, created_at, schema_version, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    Uuid::from_u128(jrow).to_string(),
                    study_id.to_string(),
                    "2026-06-12T09:00:00Z",
                    steadyinvest_contract::SCHEMA_VERSION,
                    "{}"
                ],
            )
            .expect("judgment row inserts");
        }
    }

    let mut journal = Journal::open(&path).expect("reopen writable");
    let v_before = journal.logical_version().expect("version reads");
    journal.delete_study(drop_it.id).expect("delete is FK-safe with judgments present");

    // The deleted study is gone; the kept study remains and is intact.
    let ids: Vec<Uuid> = journal
        .list_studies()
        .expect("list")
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(ids, vec![keep.id], "only the kept study remains in the list");
    assert!(
        journal.get_study(drop_it.id).expect("read").is_none(),
        "the deleted study is unreadable"
    );
    assert_eq!(
        journal.get_study(keep.id).expect("read").expect("exists"),
        keep,
        "the other study is value-intact"
    );
    assert!(
        journal.logical_version().expect("version reads") > v_before,
        "delete bumps the logical version"
    );

    // FR55: the deleted study's time-series rows are purged (no orphan); the kept study's are untouched.
    let conn = Connection::open(&path).expect("raw conn");
    let dropped: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM judgments WHERE study_id = ?1",
            rusqlite::params![drop_it.id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(dropped, 0, "the deleted study's judgments rows are purged (FR55, no orphan)");
    let kept: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM judgments WHERE study_id = ?1",
            rusqlite::params![keep.id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(kept, 1, "the kept study's time-series is untouched");
}

#[test]
fn delete_study_of_an_absent_id_is_a_noop_success() {
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0x23);
    let mut journal = new_journal(&dir, jid);
    journal
        .delete_study(Uuid::from_u128(0xDEAD))
        .expect("deleting an absent id is a no-op success");
    assert!(journal.list_studies().expect("list").is_empty());
}
