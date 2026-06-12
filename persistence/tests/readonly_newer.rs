//! Integration tests — AC 5: a newer-than-app journal opens read-only (NFR-R3), at both gates:
//! the file's `PRAGMA user_version` (open path) and a row's `schema_version` (read path).

use rusqlite::Connection;
use steadyinvest_contract::{ForecastLowOption, Judgment, Money, Study, Timestamp, SCHEMA_VERSION};
use steadyinvest_persistence::{Error, Journal};
use tempfile::TempDir;
use uuid::Uuid;

fn money(s: &str) -> Money {
    serde_json::from_str(&format!("\"{s}\"")).expect("canonical decimal string parses as Money")
}

fn ts(s: &str) -> Timestamp {
    Timestamp(s.to_string())
}

fn minimal_study(id: u128, journal_id: Uuid) -> Study {
    Study {
        id: Uuid::from_u128(id),
        journal_id,
        security_ticker: "NESN".to_string(),
        native_currency: "CHF".to_string(),
        years: Vec::new(),
        judgment: Judgment {
            estimated_high_eps: Some(money("5.20")),
            estimated_low_eps: None,
            judged_avg_high_pe: None,
            judged_avg_low_pe: None,
            forecast_low_option: ForecastLowOption::AvgLowPriceLast5y,
            current_price: None,
        },
        rationale: None,
        created_at: ts("2026-06-12T08:30:00Z"),
        schema_version: SCHEMA_VERSION,
    }
}

/// A journal file with one study, then `user_version` bumped by hand to simulate a newer build's
/// file.
fn newer_file_with_one_study(dir: &TempDir, jid: Uuid) -> (std::path::PathBuf, Study) {
    let path = dir.path().join("journal.db");
    let s = minimal_study(1, jid);
    {
        let mut journal = Journal::create(&path, jid, &ts("2026-06-12T00:00:00Z"))
            .expect("a fresh journal creates");
        journal.put_study(&s).expect("study writes");
    }
    let conn = Connection::open(&path).expect("raw open");
    conn.pragma_update(None, "user_version", 9)
        .expect("user_version bumps");
    drop(conn);
    (path, s)
}

// ── Gate 1: file user_version > latest known migration ──

#[test]
fn newer_file_opens_read_only_reads_work_writes_fail() {
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0xA);
    let (path, s) = newer_file_with_one_study(&dir, jid);

    let mut journal = Journal::open(&path).expect("a newer file still opens (read-only)");
    assert!(journal.is_read_only());
    assert_eq!(journal.id(), jid, "identity stays readable");
    assert_eq!(
        journal.logical_version().expect("version reads"),
        1,
        "the heartbeat stays readable"
    );

    // Reads work for rows whose schema_version <= the app's SCHEMA_VERSION.
    let back = journal
        .get_study(s.id)
        .expect("read works in read-only mode")
        .expect("the study is there");
    assert_eq!(back, s);
    assert_eq!(journal.list_studies().expect("list works").len(), 1);

    // Writes are gated at the API level (defense in depth) with a cause-named, neutral error.
    let err = journal
        .put_study(&minimal_study(2, jid))
        .expect_err("writes are refused on a newer file");
    assert!(
        matches!(
            err,
            Error::NewerJournalSchema {
                file_user_version: 9,
                supported: 1,
            }
        ),
        "got {err:?}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("user_version 9") && msg.contains("read-only"),
        "the message states the facts: {msg:?}"
    );

    // The refused write changed nothing.
    assert_eq!(journal.logical_version().expect("version reads"), 1);
}

#[test]
fn newer_file_open_does_not_run_migrations_or_mutate() {
    let dir = TempDir::new().expect("tempdir");
    let (path, _) = newer_file_with_one_study(&dir, Uuid::from_u128(0xB));
    drop(Journal::open(&path).expect("read-only open"));
    let conn = Connection::open(&path).expect("raw open");
    let v: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .expect("pragma reads");
    assert_eq!(v, 9, "the read-only open left user_version untouched");
}

// ── Gate 2: a single row's schema_version > contract::SCHEMA_VERSION ──

#[test]
fn newer_row_schema_version_fails_its_read_with_a_typed_error() {
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0xC);
    let path = dir.path().join("journal.db");
    let ok_study = minimal_study(1, jid);
    {
        let mut journal = Journal::create(&path, jid, &ts("2026-06-12T00:00:00Z"))
            .expect("a fresh journal creates");
        journal.put_study(&ok_study).expect("study writes");
    }

    // Hand-insert a future row (schema_version = SCHEMA_VERSION + 1) as a newer build would.
    let future_id = Uuid::from_u128(0xF);
    let conn = Connection::open(&path).expect("raw open");
    conn.execute(
        "INSERT INTO studies (id, journal_id, security_ticker, created_at, status,
                              schema_version, method_version, payload)
         VALUES (?1, ?2, 'FUT', '2026-06-12T09:00:00Z', 'active', ?3, NULL,
                 '{\"a_future_shape\": true}')",
        rusqlite::params![future_id.to_string(), jid.to_string(), SCHEMA_VERSION + 1],
    )
    .expect("future row inserts");
    drop(conn);

    let journal = Journal::open(&path).expect("file user_version is still 1 — normal open");
    assert!(!journal.is_read_only());

    let err = journal
        .get_study(future_id)
        .expect_err("a newer row fails its read, never a silent partial parse");
    assert!(
        matches!(
            err,
            Error::NewerRowSchema {
                row_schema_version,
                supported,
            } if row_schema_version == i64::from(SCHEMA_VERSION + 1) && supported == SCHEMA_VERSION
        ),
        "got {err:?}"
    );

    // Rows at a known schema_version keep reading fine next to the future one.
    assert_eq!(
        journal
            .get_study(ok_study.id)
            .expect("read works")
            .expect("study exists"),
        ok_study
    );
}

#[test]
fn unknown_fields_within_a_known_version_are_tolerated() {
    // The version gate — not field-set divergence — is the contract: serde tolerates unknown
    // *fields* by design (forward-compat rail, no deny_unknown_fields).
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0xD);
    let path = dir.path().join("journal.db");
    let s = minimal_study(1, jid);
    {
        let mut journal = Journal::create(&path, jid, &ts("2026-06-12T00:00:00Z"))
            .expect("a fresh journal creates");
        journal.put_study(&s).expect("study writes");
    }

    // Graft an extra top-level field into the stored payload, keeping schema_version = 1.
    let conn = Connection::open(&path).expect("raw open");
    let payload: String = conn
        .query_row(
            "SELECT payload FROM studies WHERE id = ?1",
            [s.id.to_string()],
            |r| r.get(0),
        )
        .expect("payload reads");
    let mut v: serde_json::Value = serde_json::from_str(&payload).expect("payload is JSON");
    v.as_object_mut().expect("payload is an object").insert(
        "field_from_a_newer_build".to_string(),
        serde_json::json!(42),
    );
    conn.execute(
        "UPDATE studies SET payload = ?1 WHERE id = ?2",
        rusqlite::params![v.to_string(), s.id.to_string()],
    )
    .expect("payload updates");
    drop(conn);

    let journal = Journal::open(&path).expect("normal open");
    let back = journal
        .get_study(s.id)
        .expect("the unknown field is tolerated")
        .expect("study exists");
    assert_eq!(back, s);
}

#[test]
fn corrupt_payload_fails_with_the_cause_named_error() {
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0xE);
    let path = dir.path().join("journal.db");
    {
        Journal::create(&path, jid, &ts("2026-06-12T00:00:00Z")).expect("creates");
    }
    let bad_id = Uuid::from_u128(0xBAD);
    let conn = Connection::open(&path).expect("raw open");
    conn.execute(
        "INSERT INTO studies (id, journal_id, security_ticker, created_at, status,
                              schema_version, method_version, payload)
         VALUES (?1, ?2, 'BAD', '2026-06-12T09:00:00Z', 'active', 1, NULL, 'not json at all')",
        rusqlite::params![bad_id.to_string(), jid.to_string()],
    )
    .expect("bad row inserts");
    drop(conn);

    let journal = Journal::open(&path).expect("normal open");
    let err = journal
        .get_study(bad_id)
        .expect_err("an unparseable payload is a loud, typed failure");
    assert!(matches!(err, Error::CorruptPayload { .. }), "got {err:?}");
}
