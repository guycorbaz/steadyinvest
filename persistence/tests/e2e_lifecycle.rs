//! End-to-end lifecycle tests — the journal's real "user journey" across app sessions, plus the
//! environment failure modes a user-selectable DB location makes reachable (wrong file picked,
//! foreign SQLite db, hand-damaged metadata).
//!
//! Complements `journal_roundtrip.rs` (single-session AC coverage): here every scenario spans
//! multiple open/close cycles or multiple journal files. All file work happens in a
//! `tempfile::TempDir` (`/tmp`, outside the Synology Drive sync watch).

use rusqlite::Connection;
use steadyinvest_contract::{
    Cell, Coverage, ForecastLowOption, Freshness, Judgment, Money, Provenance, Review, Source,
    Study, Timestamp, YearData, SCHEMA_VERSION,
};
use steadyinvest_persistence::{clear_lock, lock_is_stale, Error, Journal, JournalMode};
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
        pending: None,
    }
}

fn study(id: u128, journal_id: Uuid, ticker: &str, created_at: &str) -> Study {
    Study {
        id: Uuid::from_u128(id),
        journal_id,
        security_ticker: ticker.to_string(),
        native_currency: "CHF".to_string(),
        years: vec![YearData {
            year: 2022,
            sales: cell(Some("1322.500000")),
            eps: cell(Some("3.15")),
            high_price: cell(Some("150")),
            low_price: cell(Some("101.25")),
            dividend_per_share: Some(cell(None)),
            pre_tax_profit: None,
            book_value_per_share: None,
        }],
        judgment: Judgment {
            estimated_high_eps: Some(money("5.20")),
            estimated_low_eps: Some(money("2.10")),
            projected_sales_growth_pct: Some(money("8.5")),
            projected_eps_growth_pct: Some(money("11.0")),
            judged_avg_high_pe: Some(money("18")),
            judged_avg_low_pe: Some(money("11.5")),
            forecast_low_option: ForecastLowOption::AvgLowPeTimesEps,
            recent_severe_low: Some(money("72.40")),
            current_price: Some(money("104.00")),
            present_full_year_dividend: Some(money("2.95")),
            ttm_eps: None,
        },
        rationale: Some("Margin trend noted; demand steady.".to_string()),
        created_at: ts(created_at),
        schema_version: SCHEMA_VERSION,
    }
}

const JOURNAL_TS: &str = "2026-06-12T00:00:00Z";

// ── The full multi-session journey ──

#[test]
fn full_lifecycle_across_three_sessions_preserves_everything() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("journal.db");
    let jid = Uuid::from_u128(0xE2E);

    let nesn = study(1, jid, "NESN", "2026-06-10T09:00:00Z");
    let rog = study(2, jid, "ROG", "2026-06-11T09:00:00Z");
    let novn = study(3, jid, "NOVN", "2026-06-12T09:00:00Z");

    // Session 1: create the journal, write three studies, close (drop).
    {
        let mut journal =
            Journal::create(&path, jid, &ts(JOURNAL_TS)).expect("a fresh journal creates");
        for s in [&nesn, &rog, &novn] {
            journal.put_study(s).expect("study writes");
        }
        assert_eq!(journal.logical_version().expect("version reads"), 3);
    }

    // Session 2: reopen, verify the full state, revise one study, close.
    let mut revised = rog.clone();
    {
        let mut journal = Journal::open(&path).expect("session 2 reopens");
        assert_eq!(journal.id(), jid, "identity survives the session boundary");
        assert_eq!(journal.logical_version().expect("version reads"), 3);

        let listed = journal.list_studies().expect("list reads");
        assert_eq!(
            listed
                .iter()
                .map(|s| s.security_ticker.as_str())
                .collect::<Vec<_>>(),
            ["NESN", "ROG", "NOVN"],
            "all three studies are listed in created_at order"
        );

        revised.rationale = Some("Revised after the annual report.".to_string());
        revised.judgment.current_price = Some(money("99.85"));
        journal.put_study(&revised).expect("revision upserts");
        assert_eq!(journal.logical_version().expect("version reads"), 4);
    }

    // Session 3: reopen, everything written in sessions 1–2 reads back exactly.
    {
        let journal = Journal::open(&path).expect("session 3 reopens");
        assert_eq!(journal.logical_version().expect("version reads"), 4);
        assert_eq!(
            journal.list_studies().expect("list reads").len(),
            3,
            "the revision updated in place — still three studies"
        );
        for expected in [&nesn, &revised, &novn] {
            let back = journal
                .get_study(expected.id)
                .expect("study reads")
                .expect("study exists");
            assert_eq!(
                back, *expected,
                "{} reads back exactly across sessions",
                expected.security_ticker
            );
        }
        assert_eq!(
            journal
                .get_study(Uuid::from_u128(0x404))
                .expect("read works"),
            None,
            "an absent id is None, not an error"
        );
    }
}

#[test]
fn two_journals_side_by_side_are_fully_independent() {
    let dir = TempDir::new().expect("tempdir");
    let path_a = dir.path().join("a.db");
    let path_b = dir.path().join("b.db");
    let jid_a = Uuid::from_u128(0xAAAA);
    let jid_b = Uuid::from_u128(0xBBBB);

    let mut journal_a = Journal::create(&path_a, jid_a, &ts(JOURNAL_TS)).expect("journal A");
    let mut journal_b = Journal::create(&path_b, jid_b, &ts(JOURNAL_TS)).expect("journal B");

    let in_a = study(1, jid_a, "NESN", "2026-06-12T09:00:00Z");
    journal_a.put_study(&in_a).expect("A writes");
    journal_b
        .put_study(&study(2, jid_b, "ROG", "2026-06-12T09:00:00Z"))
        .expect("B writes");
    journal_b
        .put_study(&study(3, jid_b, "NOVN", "2026-06-12T10:00:00Z"))
        .expect("B writes again");

    assert_eq!(journal_a.id(), jid_a);
    assert_eq!(journal_b.id(), jid_b);
    assert_eq!(
        journal_a.logical_version().expect("version reads"),
        1,
        "A's heartbeat counts only A's mutations"
    );
    assert_eq!(journal_b.logical_version().expect("version reads"), 2);
    assert_eq!(journal_a.list_studies().expect("list reads").len(), 1);
    assert_eq!(journal_b.list_studies().expect("list reads").len(), 2);
    assert_eq!(
        journal_b.get_study(in_a.id).expect("read works"),
        None,
        "a study written to A never appears in B"
    );

    // A study carrying A's identity is rejected by B (cross-journal integrity, ADD6).
    let err = journal_b
        .put_study(&study(4, jid_a, "UBSG", "2026-06-12T11:00:00Z"))
        .expect_err("B refuses a study stamped with A's journal_id");
    assert!(
        matches!(err, Error::JournalIdentityMismatch { .. }),
        "got {err:?}"
    );
}

#[test]
fn list_studies_breaks_created_at_ties_by_id() {
    let dir = TempDir::new().expect("tempdir");
    let jid = Uuid::from_u128(0x71E);
    let mut journal = Journal::create(dir.path().join("journal.db"), jid, &ts(JOURNAL_TS))
        .expect("a fresh journal creates");

    // Same created_at; inserted in reverse id order to prove the ORDER BY does the work.
    journal
        .put_study(&study(2, jid, "ROG", "2026-06-12T09:00:00Z"))
        .expect("writes");
    journal
        .put_study(&study(1, jid, "NESN", "2026-06-12T09:00:00Z"))
        .expect("writes");

    let listed = journal.list_studies().expect("list reads");
    assert_eq!(
        listed.iter().map(|s| s.id).collect::<Vec<_>>(),
        [Uuid::from_u128(1), Uuid::from_u128(2)],
        "equal created_at falls back to id order — the listing stays deterministic"
    );
}

// ── Environment failure modes (the DB location is user-selectable — wrong picks happen) ──

#[test]
fn opening_a_non_sqlite_file_is_a_typed_error_not_a_panic() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("not-a-database.db");
    std::fs::write(
        &path,
        "This is a plain text file, not a SQLite database. ".repeat(8),
    )
    .expect("garbage file writes");

    let err = Journal::open(&path).expect_err("a non-SQLite file is refused");
    assert!(matches!(err, Error::Sqlite(_)), "got {err:?}");
}

#[test]
fn opening_a_foreign_sqlite_db_without_journal_meta_is_corrupt_meta_never_silent_adoption() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("foreign.db");
    {
        // A perfectly valid SQLite file that was never a journal (user_version 0, no meta row).
        let conn = Connection::open(&path).expect("foreign db creates");
        conn.execute_batch("CREATE TABLE someone_elses_data (x TEXT);")
            .expect("foreign table creates");
    }

    let err = Journal::open(&path).expect_err("a foreign db has no journal identity");
    assert!(
        matches!(err, Error::CorruptJournalMeta { .. }),
        "got {err:?}"
    );

    // The refused open never wrote into the foreign file: no journal tables, no migration
    // stamp, no WAL switch (the DB location is user-selectable — wrong picks must stay intact).
    let conn = Connection::open(&path).expect("raw reopen");
    let stranger_tables: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name NOT IN ('someone_elses_data')",
            [],
            |r| r.get(0),
        )
        .expect("sqlite_master reads");
    assert_eq!(
        stranger_tables, 0,
        "open wrote schema into a foreign database"
    );
    let v: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .expect("user_version reads");
    assert_eq!(v, 0, "open migrated a file that is not a journal");
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .expect("journal_mode reads");
    assert_ne!(
        mode.to_lowercase(),
        "wal",
        "open switched a foreign file's journal mode"
    );
}

#[test]
fn failed_create_leaves_no_file_behind() {
    // A create that fails must leave the path clean — a leftover half-journal would trap the
    // retry on JournalExists. Failure mode used here: the parent directory does not exist.
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("no-such-subdir").join("journal.db");
    let err = Journal::create(&path, Uuid::from_u128(0xA), &ts(JOURNAL_TS))
        .expect_err("create in a missing directory fails");
    // The single-instance lock (Story 5.5) is acquired first, so a missing parent directory now
    // surfaces as `Lock` (the lock sidecar can't be created) rather than `Sqlite` (the DB open) —
    // either is a clean "create failed" with no file left behind.
    assert!(
        matches!(err, Error::Sqlite(_) | Error::Lock { .. }),
        "got {err:?}"
    );
    assert!(!path.exists(), "a failed create left a file behind");
    let mut lock = path.as_os_str().to_os_string();
    lock.push("-lock");
    assert!(
        !std::path::Path::new(&lock).exists(),
        "a failed create left a lock sidecar behind"
    );

    // And a create that failed cannot block a later, correct retry at the same path.
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir creates");
    drop(Journal::create(&path, Uuid::from_u128(0xA), &ts(JOURNAL_TS)).expect("retry succeeds"));
}

#[test]
fn the_single_instance_lock_allows_same_process_reopen() {
    // Story 5.5 (ADD6): single-instance = single OS PROCESS. A same-process re-open (same pid AND
    // start-time) is allowed (SQLite coordinates intra-process); the lock releases on the owning drop.
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("journal.db");
    let first = Journal::create(&path, Uuid::from_u128(0xA), &ts(JOURNAL_TS)).expect("create");
    // Same process, same start-time → a re-open is allowed (the "reopen in a new session" test idiom).
    drop(Journal::open(&path).expect("a same-process re-open is allowed"));
    // The live, same-process lock is NOT stale (it must not be reclaimed out from under us).
    assert!(
        !lock_is_stale(&path),
        "a live same-process lock is not stale"
    );
    drop(first);
    // After the owner drops, the sidecar is gone → no lock, a fresh open succeeds.
    drop(Journal::open(&path).expect("open after the owner dropped"));
}

#[test]
fn a_reused_or_crashed_pid_lock_is_stale_and_reclaimable() {
    // Story 5.5: a lock with THIS pid but a DIFFERENT start-time models a crashed prior run whose pid
    // was reused (the start-time qualifier defeats PID recycling). It is stale + refused + reclaimable.
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("journal.db");
    drop(Journal::create(&path, Uuid::from_u128(0xA), &ts(JOURNAL_TS)).expect("create"));
    let mut lock = path.as_os_str().to_os_string();
    lock.push("-lock");
    // Our own PID, but start-time 1 (never the real one) → not the live owner → stale.
    std::fs::write(&lock, format!("{} 1", std::process::id())).expect("forge a reused-pid lock");
    assert!(
        lock_is_stale(&path),
        "a pid with a mismatched start-time is stale"
    );
    assert!(matches!(Journal::open(&path), Err(Error::LockHeld { .. })));
    clear_lock(&path).expect("reclaim");
    assert!(!lock_is_stale(&path), "no lock left to be stale");
    drop(Journal::open(&path).expect("open after reclaiming"));
}

#[test]
fn an_unparseable_lock_is_stale_and_reclaimable() {
    // Story 5.5: an empty/garbled lock (e.g. a failed PID write) is treated as stale, not a permanent
    // self-lockout.
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("journal.db");
    drop(Journal::create(&path, Uuid::from_u128(0xA), &ts(JOURNAL_TS)).expect("create"));
    let mut lock = path.as_os_str().to_os_string();
    lock.push("-lock");
    std::fs::write(&lock, "not a pid").expect("write a garbled lock");
    assert!(lock_is_stale(&path), "an unparseable lock is reclaimable");
    clear_lock(&path).expect("reclaim");
    drop(Journal::open(&path).expect("open after reclaiming"));
}

#[test]
fn delete_mode_opens_without_a_wal_sidecar() {
    // Story 5.5 (ADD8): the sync-safe DELETE mode leaves no -wal beside the live .db.
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("journal.db");
    let mut journal = Journal::create_with_mode(
        &path,
        Uuid::from_u128(0xA),
        &ts(JOURNAL_TS),
        JournalMode::Delete,
    )
    .expect("create in DELETE mode");
    // A write that would produce a WAL frame under WAL mode.
    journal
        .ensure_portfolio(Uuid::from_u128(0xB), "P", &ts(JOURNAL_TS))
        .expect("a write");
    let mut wal = path.as_os_str().to_os_string();
    wal.push("-wal");
    assert!(
        !std::path::Path::new(&wal).exists(),
        "DELETE mode leaves no -wal sidecar"
    );
}

#[test]
fn invalid_journal_id_in_meta_fails_open_with_corrupt_meta() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("journal.db");
    drop(Journal::create(&path, Uuid::from_u128(0xA), &ts(JOURNAL_TS)).expect("creates"));

    let conn = Connection::open(&path).expect("raw open");
    conn.execute(
        "UPDATE journal_meta SET journal_id = 'not-a-uuid' WHERE id = 1",
        [],
    )
    .expect("meta damages");
    drop(conn);

    let err = Journal::open(&path).expect_err("a damaged identity is loud, never guessed");
    assert!(
        matches!(err, Error::CorruptJournalMeta { .. }),
        "got {err:?}"
    );
}

#[test]
fn negative_logical_version_reads_as_corrupt_meta() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("journal.db");
    drop(Journal::create(&path, Uuid::from_u128(0xA), &ts(JOURNAL_TS)).expect("creates"));

    let conn = Connection::open(&path).expect("raw open");
    conn.execute(
        "UPDATE journal_meta SET logical_version = -5 WHERE id = 1",
        [],
    )
    .expect("meta damages");
    drop(conn);

    let journal = Journal::open(&path).expect("identity itself is intact — open succeeds");
    let err = journal
        .logical_version()
        .expect_err("a negative heartbeat is corrupt, not wrapped into u64");
    assert!(
        matches!(err, Error::CorruptJournalMeta { .. }),
        "got {err:?}"
    );
}
