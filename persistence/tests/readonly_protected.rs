//! Integration tests — a journal the OS will not let us write (2026-09-26 on-screen defect: a
//! `chmod 444` dossier opened as writable and the first write failed with SQLite's own English
//! text). Such a file — or a writable file in a protected directory — opens READ-ONLY with its
//! named cause, reads work, every write is refused up front by the API gate, and the open leaves no
//! `-wal`/`-shm` strays beside the protected file.
//!
//! Unix-only (permission bits). Skipped at run time when permissions do not bind (running as
//! root): a probe write open decides, so a root CI cannot turn these into false failures.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use steadyinvest_contract::{ForecastLowOption, Judgment, SCHEMA_VERSION, Study, Timestamp};
use steadyinvest_persistence::{Error, Journal, JournalMode, ReadOnlyCause};
use tempfile::TempDir;
use uuid::Uuid;

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
        created_at: ts("2026-06-12T08:30:00Z"),
        schema_version: SCHEMA_VERSION,
    }
}

const JID: Uuid = Uuid::from_u128(0xD0551E);

/// A closed WAL journal holding one study, at `dir/name`.
fn journal_with_one_study(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let mut journal =
        Journal::create(&path, JID, &ts("2026-06-12T00:00:00Z")).expect("a fresh journal creates");
    journal
        .put_study(&minimal_study(1, JID))
        .expect("study writes");
    path
}

fn set_mode(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
}

/// Whether the OS actually refuses a write open of `path` (false when running as root).
fn write_refused(path: &Path) -> bool {
    std::fs::OpenOptions::new().write(true).open(path).is_err()
}

/// Whether a file can be created in `dir` (false for a protected directory, unless root).
fn dir_refuses_new_files(dir: &Path) -> bool {
    let probe = dir.join(".probe");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            false
        }
        Err(_) => true,
    }
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("dir lists")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn protected_file_opens_read_only_reads_work_writes_are_refused() {
    let dir = TempDir::new().expect("tempdir");
    let path = journal_with_one_study(dir.path(), "ro.db");
    set_mode(&path, 0o444);
    if !write_refused(&path) {
        return; // permissions do not bind (root) — nothing to observe
    }

    let mut journal = Journal::open(&path).expect("a protected file still opens (read-only)");
    assert!(journal.is_read_only());
    assert_eq!(
        journal.read_only_cause(),
        Some(ReadOnlyCause::FileWriteProtected)
    );
    assert_eq!(journal.id(), JID, "identity reads");
    assert_eq!(journal.list_studies().expect("list works").len(), 1);

    let err = journal
        .put_study(&minimal_study(2, JID))
        .expect_err("a write on a protected file is refused");
    assert!(
        matches!(err, Error::WriteProtected { directory: false }),
        "the API gate names the protected FILE: {err:?}"
    );
    assert!(err.is_write_protected());
    let err = journal
        .set_study_status(Uuid::from_u128(1), "archived")
        .expect_err("archiving is refused too");
    assert!(matches!(err, Error::WriteProtected { directory: false }));
    assert_eq!(journal.logical_version().expect("version reads"), 1);

    drop(journal);
    assert_eq!(
        names_in(dir.path()),
        vec!["ro.db".to_string()],
        "no -wal/-shm (nor lock) left beside the protected file"
    );
    set_mode(&path, 0o644);
}

#[test]
fn protected_directory_opens_read_only_with_its_own_cause() {
    let root = TempDir::new().expect("tempdir");
    let dir = root.path().join("locked");
    std::fs::create_dir(&dir).expect("mkdir");
    let path = journal_with_one_study(&dir, "journal.db");
    set_mode(&dir, 0o555);
    if !dir_refuses_new_files(&dir) {
        set_mode(&dir, 0o755);
        return;
    }

    let result = Journal::open(&path);
    let names = names_in(&dir);
    set_mode(&dir, 0o755); // restore before any assert, so the TempDir cleans up
    let mut journal = result.expect("a writable file in a protected directory opens (read-only)");
    assert_eq!(
        journal.read_only_cause(),
        Some(ReadOnlyCause::DirectoryWriteProtected)
    );
    assert_eq!(journal.list_studies().expect("list works").len(), 1);
    let err = journal
        .put_study(&minimal_study(2, JID))
        .expect_err("a write is refused up front");
    assert!(
        matches!(err, Error::WriteProtected { directory: true }),
        "the API gate names the protected DIRECTORY: {err:?}"
    );
    assert_eq!(names, vec!["journal.db".to_string()], "nothing created");
}

#[test]
fn protected_file_with_uncheckpointed_wal_still_reads_the_committed_rows() {
    let src = TempDir::new().expect("tempdir");
    let path = journal_with_one_study(src.path(), "src.db");
    // Freeze a mid-life state: a second study committed into the -wal, not yet checkpointed, then
    // the three files copied aside while the writer is still open (a crash / sync snapshot shape).
    let copy_dir = TempDir::new().expect("tempdir");
    let copy = copy_dir.path().join("ro.db");
    {
        let mut journal = Journal::open(&path).expect("reopen");
        let conn = Connection::open(&path).expect("raw");
        conn.pragma_update(None, "wal_autocheckpoint", 0)
            .expect("no autocheckpoint");
        journal
            .put_study(&minimal_study(2, JID))
            .expect("second study");
        for suffix in ["", "-wal", "-shm"] {
            let mut from = path.as_os_str().to_os_string();
            from.push(suffix);
            let mut to = copy.as_os_str().to_os_string();
            to.push(suffix);
            if Path::new(&from).exists() {
                std::fs::copy(&from, &to).expect("copy");
            }
        }
    }
    let mut wal = copy.as_os_str().to_os_string();
    wal.push("-wal");
    if std::fs::metadata(&wal).map_or(true, |m| m.len() == 0) {
        return; // the WAL got checkpointed on this platform — the shape did not arise
    }
    set_mode(&copy, 0o444);
    if !write_refused(&copy) {
        return;
    }
    let journal = Journal::open(&copy).expect("opens read-only");
    assert_eq!(
        journal.read_only_cause(),
        Some(ReadOnlyCause::FileWriteProtected)
    );
    assert_eq!(
        journal.list_studies().expect("list works").len(),
        2,
        "the committed row still in the -wal is read, never ignored"
    );
    drop(journal);
    set_mode(&copy, 0o644);
}

#[test]
fn protected_file_older_than_this_build_is_refused_untouched() {
    let dir = TempDir::new().expect("tempdir");
    let path = journal_with_one_study(dir.path(), "old.db");
    let conn = Connection::open(&path).expect("raw open");
    conn.pragma_update(None, "user_version", 1)
        .expect("user_version lowers");
    drop(conn);
    set_mode(&path, 0o444);
    if !write_refused(&path) {
        return;
    }
    let err = Journal::open(&path).expect_err("an unmigratable file is not opened");
    assert!(
        matches!(
            err,
            Error::WriteProtectedOutdated {
                file_user_version: 1,
                ..
            }
        ),
        "got {err:?}"
    );
    assert_eq!(names_in(dir.path()), vec!["old.db".to_string()]);
    set_mode(&path, 0o644);
}

#[test]
fn protected_file_newer_than_this_build_keeps_the_newer_schema_cause() {
    let dir = TempDir::new().expect("tempdir");
    let path = journal_with_one_study(dir.path(), "new.db");
    let conn = Connection::open(&path).expect("raw open");
    conn.pragma_update(None, "user_version", 99)
        .expect("user_version bumps");
    drop(conn);
    set_mode(&path, 0o444);
    if !write_refused(&path) {
        return;
    }
    let journal = Journal::open(&path).expect("opens read-only");
    assert_eq!(
        journal.read_only_cause(),
        Some(ReadOnlyCause::NewerSchema {
            file_user_version: 99
        })
    );
    drop(journal);
    set_mode(&path, 0o644);
}

#[test]
fn protected_file_under_uri_special_characters_opens_the_right_file() {
    let root = TempDir::new().expect("tempdir");
    let dir = root.path().join("a b?c#d%e");
    std::fs::create_dir(&dir).expect("mkdir");
    let path = journal_with_one_study(&dir, "mon dossier.db");
    set_mode(&path, 0o444);
    if !write_refused(&path) {
        return;
    }
    let journal = Journal::open_with_mode(&path, JournalMode::Delete)
        .expect("a path with URI syntax in it opens");
    assert_eq!(journal.id(), JID, "the very file chosen was opened");
    drop(journal);
    set_mode(&path, 0o644);
}

#[test]
fn a_writable_journal_is_not_read_only() {
    let dir = TempDir::new().expect("tempdir");
    let path = journal_with_one_study(dir.path(), "rw.db");
    let journal = Journal::open(&path).expect("opens");
    assert_eq!(journal.read_only_cause(), None);
}
