//! Story 8.5a — what the app's draft inbox relies on in the persistence layer:
//! - `Journal::data_version` moves when ANOTHER connection commits, never on the journal's own
//!   writes (arch A9: the poller's signal);
//! - a read transaction held by another connection (the shape of an MCP read) on a DELETE-mode
//!   dossier delays the app's commit within `busy_timeout` and never loses it (FR67 [P4], NFR-R2);
//!   a read held past `busy_timeout` yields a named busy error, not a hang.

use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use steadyinvest_contract::{ForecastLowOption, Judgment, SCHEMA_VERSION, Study, Timestamp};
use steadyinvest_persistence::{ErrorKind, Journal, JournalMode};
use tempfile::TempDir;
use uuid::Uuid;

fn ts(s: &str) -> Timestamp {
    Timestamp(s.to_string())
}

fn study(id: u128, journal_id: Uuid, ticker: &str) -> Study {
    Study {
        id: Uuid::from_u128(id),
        journal_id,
        security_ticker: ticker.to_string(),
        native_currency: "CHF".to_string(),
        years: Vec::new(),
        judgment: Judgment {
            ai_placed: Default::default(),
            estimated_high_eps: None,
            estimated_low_eps: None,
            projected_sales_growth_pct: None,
            projected_eps_growth_pct: None,
            judged_avg_high_pe: None,
            judged_avg_low_pe: None,
            forecast_low_option: ForecastLowOption::AvgLowPriceLast5y,
            recent_severe_low: None,
            current_price: None,
            present_full_year_dividend: None,
            ttm_eps: None,
        },
        rationale: None,
        company_name: None,
        notes: Vec::new(),
        frozen_verdict: None,
        created_at: ts("2026-09-28T08:00:00Z"),
        schema_version: SCHEMA_VERSION,
    }
}

const JID: u128 = 0x85A0;

fn dossier(mode: JournalMode) -> (TempDir, PathBuf, Journal) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("dossier.db");
    let jid = Uuid::from_u128(JID);
    let mut journal =
        Journal::create_with_mode(&path, jid, &ts("2026-09-28T07:00:00Z"), mode).expect("creates");
    journal
        .put_study(&study(1, jid, "NESN"))
        .expect("first study");
    (dir, path, journal)
}

#[test]
fn data_version_moves_on_another_connections_commit_only() {
    let (_dir, path, mut journal) = dossier(JournalMode::Wal);
    let before = journal.data_version().expect("reads");

    // The journal's own write: the signal must NOT move (the app pushes its own changes).
    journal
        .put_study(&study(2, Uuid::from_u128(JID), "ROG"))
        .expect("own write");
    assert_eq!(journal.data_version().expect("reads"), before);

    // Another connection commits (the MCP server, another process): the signal moves.
    let other = Connection::open(&path).expect("other connection");
    other
        .execute(
            "UPDATE journal_meta SET logical_version = logical_version + 1 WHERE id = 1",
            [],
        )
        .expect("other commit");
    drop(other);
    assert_ne!(journal.data_version().expect("reads"), before);
}

/// Hold a read transaction (SHARED lock in DELETE mode) on `path` from another connection for
/// `hold`; returns once the lock is held, with the thread's handle.
fn hold_read(path: &Path, hold: Duration) -> thread::JoinHandle<()> {
    let (ready_tx, ready_rx) = mpsc::channel();
    let path = path.to_path_buf();
    let handle = thread::spawn(move || {
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .expect("read-only open");
        conn.execute_batch("BEGIN").expect("begin");
        let n: i64 = conn
            .query_row("SELECT count(*) FROM studies", [], |r| r.get(0))
            .expect("read holds SHARED");
        assert!(n >= 1);
        ready_tx.send(()).expect("signal");
        thread::sleep(hold);
        conn.execute_batch("COMMIT").expect("end read");
    });
    ready_rx.recv().expect("reader ready");
    handle
}

#[test]
fn a_held_mcp_read_delays_the_apps_commit_within_busy_timeout_and_never_loses_it() {
    let (_dir, path, mut journal) = dossier(JournalMode::Delete);
    let jid = Uuid::from_u128(JID);
    let reader = hold_read(&path, Duration::from_millis(1000));

    let started = Instant::now();
    journal
        .put_study(&study(3, jid, "NOVN"))
        .expect("the commit waits for the read, then lands");
    let elapsed = started.elapsed();
    reader.join().expect("reader ends");

    assert!(
        elapsed < Duration::from_millis(5000),
        "within busy_timeout, took {elapsed:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(500),
        "the commit really waited on the read (took {elapsed:?}) — else the test proves nothing"
    );
    let back = journal
        .get_study(Uuid::from_u128(3))
        .expect("reads")
        .expect("persisted");
    assert_eq!(back.security_ticker, "NOVN");
}

#[test]
fn a_read_held_past_busy_timeout_is_a_named_busy_error_not_a_hang() {
    let (_dir, path, mut journal) = dossier(JournalMode::Delete);
    let jid = Uuid::from_u128(JID);
    let reader = hold_read(&path, Duration::from_millis(6500));

    let started = Instant::now();
    let err = journal
        .put_study(&study(4, jid, "UBSG"))
        .expect_err("the lock outlives busy_timeout");
    let elapsed = started.elapsed();
    reader.join().expect("reader ends");

    assert_eq!(err.kind(), ErrorKind::Locked, "named, got {err:?}");
    assert!(
        elapsed < Duration::from_millis(6400),
        "gave up at busy_timeout, took {elapsed:?}"
    );
    // Nothing half-written: the study is absent and the journal still works.
    assert!(
        journal
            .get_study(Uuid::from_u128(4))
            .expect("reads")
            .is_none()
    );
    journal
        .put_study(&study(4, jid, "UBSG"))
        .expect("once the read ends, the write lands");
}
