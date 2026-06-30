//! The [`Journal`] — a local SQLite journal file with identity (ADD6) and a migrations harness.
//!
//! **Identity and time are caller-supplied** (ADD15 injected Clock/IdGen discipline): this crate
//! NEVER calls `Uuid::new_v4()` or any clock. The app wires real sources later; tests pass fixed
//! values for full determinism.
//!
//! **Pragmas** on every read-write open/create: `journal_mode=WAL` (persistent in the file),
//! `synchronous=NORMAL`, `busy_timeout`, `foreign_keys=ON`. The newer-file read-only path applies
//! only the connection-local ones (`busy_timeout`, `foreign_keys`) and skips the WAL write — a
//! file mutation that belongs to the read-write path only. Sync-path detection /
//! `journal_mode=DELETE` switching / single-instance lock are Epic 5 (story 5-5).

use crate::error::{Error, Result};
use crate::migrations;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use steadyinvest_contract::Timestamp;
use uuid::Uuid;

/// An open journal: one SQLite connection plus the journal's identity. Single connection per
/// `Journal` is enough for this headless story (the mutex-guarded write connection + WAL
/// concurrent readers is app-era machinery).
#[derive(Debug)]
pub struct Journal {
    pub(crate) conn: Connection,
    id: Uuid,
    /// `Some(file_user_version)` when the file is newer than this build's latest migration:
    /// the journal is read-only (NFR-R3) and write methods fail with the cause-named error.
    newer_file_version: Option<i64>,
}

impl Journal {
    /// Create a new journal file at `path` with caller-supplied identity and creation time.
    ///
    /// Refuses to overwrite: the path is required not to exist. On success the file is at the
    /// latest schema version and `journal_meta` holds `journal_id`, `created_at` and
    /// `logical_version = 0` (0 = created, never mutated; the first mutation commits 1).
    /// On failure the half-written file is removed: the path did not exist before this call,
    /// so a failed create never leaves a journal-shaped husk that a retry would trip on
    /// (`JournalExists`) or an open would misread (`CorruptJournalMeta`).
    pub fn create(
        path: impl AsRef<Path>,
        journal_id: Uuid,
        created_at: &Timestamp,
    ) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            return Err(Error::JournalExists(path.to_path_buf()));
        }
        let result = Self::create_at(path, journal_id, created_at);
        if result.is_err() {
            // Best-effort cleanup — removal failure stays unreported on purpose: the create
            // error already carries the actual cause, and the file is in /the caller's/ chosen
            // location where a leftover is recoverable by hand.
            for suffix in ["", "-wal", "-shm"] {
                let mut sidecar = path.as_os_str().to_os_string();
                sidecar.push(suffix);
                let _ = std::fs::remove_file(&sidecar);
            }
        }
        result
    }

    fn create_at(path: &Path, journal_id: Uuid, created_at: &Timestamp) -> Result<Self> {
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        apply_read_write_pragmas(&conn)?;
        migrations::run_pending(&mut conn, migrations::REGISTRY)?;
        conn.execute(
            "INSERT INTO journal_meta (id, journal_id, logical_version, created_at)
             VALUES (1, ?1, 0, ?2)",
            rusqlite::params![journal_id.to_string(), created_at.0],
        )?;
        Ok(Journal {
            conn,
            id: journal_id,
            newer_file_version: None,
        })
    }

    /// Open an existing journal file.
    ///
    /// Runs pending migrations when the file is older than this build. When the file's
    /// `user_version` is **newer** than the latest known migration, the journal opens
    /// **read-only** (NFR-R3): the handle is re-opened with `SQLITE_OPEN_READ_ONLY`, only
    /// connection-local pragmas apply, no migration runs, and write methods return the
    /// cause-named error while reads keep working.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        // No CREATE flag: opening a missing file is an error, never a silent empty journal.
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        // Version check BEFORE any pragma that mutates the file (journal_mode=WAL writes).
        let file_version = migrations::user_version(&conn)?;
        let latest = migrations::latest_version(migrations::REGISTRY);
        if file_version > i64::from(latest) {
            drop(conn);
            let conn = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            apply_connection_local_pragmas(&conn)?;
            let id = read_journal_id(&conn)?;
            return Ok(Journal {
                conn,
                id,
                newer_file_version: Some(file_version),
            });
        }

        // Identity is read BEFORE anything writes to the file (the WAL pragma and migrations
        // both mutate it): the DB location is user-selectable, so `open` on a wrong pick — a
        // foreign SQLite database, or any non-journal file — must fail without ever writing
        // our schema into it. Every real journal has `journal_meta` from migration 1 onward.
        let id = read_journal_id(&conn)?;
        apply_read_write_pragmas(&conn)?;
        migrations::run_pending(&mut conn, migrations::REGISTRY)?;
        Ok(Journal {
            conn,
            id,
            newer_file_version: None,
        })
    }

    /// The journal's identity (UUID), as stored in the `journal_meta` singleton.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// The journal's monotonic logical version. Starts at 0 on create; every mutating call
    /// increments it in the same transaction as the mutation. The SQLite column is INTEGER
    /// (i64); exposed as `u64` to match the `Provenance.logical_version` axis (checked
    /// conversion — a negative stored value is corrupt).
    pub fn logical_version(&self) -> Result<u64> {
        let v: i64 = self
            .conn
            .query_row(
                "SELECT logical_version FROM journal_meta WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .map_err(map_missing_meta)?;
        u64::try_from(v).map_err(|_| Error::CorruptJournalMeta {
            detail: format!("logical_version {v} is negative"),
        })
    }

    /// True when the file was written by a newer schema and is therefore opened read-only.
    pub fn is_read_only(&self) -> bool {
        self.newer_file_version.is_some()
    }

    /// Checkpoint the WAL into the main database file and truncate it (`PRAGMA
    /// wal_checkpoint(TRUNCATE)`), so a plain file copy of the `.db` is **self-contained** — no
    /// recently-committed data left stranded in a `-wal` sidecar (Story 5.4 `create_backup`). A
    /// read-safe operation that changes no logical data. A no-op on a read-only handle (nothing to
    /// checkpoint there).
    pub fn checkpoint(&self) -> Result<()> {
        // A read-only handle (for any reason) cannot checkpoint — a no-op rather than a hard error, so
        // backing up a read-only journal still works (it just copies the main `.db`).
        if self.is_read_only() {
            return Ok(());
        }
        self.conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        Ok(())
    }

    /// API-level write gate (defense in depth on top of `SQLITE_OPEN_READ_ONLY`): every mutating
    /// method calls this first.
    pub(crate) fn check_writable(&self) -> Result<()> {
        match self.newer_file_version {
            Some(file_user_version) => Err(Error::NewerJournalSchema {
                file_user_version,
                supported: migrations::latest_version(migrations::REGISTRY),
            }),
            None => Ok(()),
        }
    }
}

/// Pragmas that only affect this connection — safe on a read-only handle.
fn apply_connection_local_pragmas(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(())
}

/// Full pragma set for read-write handles. `journal_mode=WAL` is persistent in the DB file.
fn apply_read_write_pragmas(conn: &Connection) -> Result<()> {
    apply_connection_local_pragmas(conn)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

/// Read and parse the journal identity from the `journal_meta` singleton row.
fn read_journal_id(conn: &Connection) -> Result<Uuid> {
    let text: String = conn
        .query_row(
            "SELECT journal_id FROM journal_meta WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .map_err(map_missing_meta)?;
    Uuid::parse_str(&text).map_err(|e| Error::CorruptJournalMeta {
        detail: format!("journal_id {text:?} is not a valid UUID: {e}"),
    })
}

/// A missing singleton row (or missing table) is corrupt metadata, not a bare SQLite error.
/// The missing-table case is how `open` rejects a valid SQLite file that was never a journal
/// (`journal_meta` exists in every real journal from migration 1 onward).
fn map_missing_meta(e: rusqlite::Error) -> Error {
    match e {
        rusqlite::Error::QueryReturnedNoRows => Error::CorruptJournalMeta {
            detail: "the singleton row is absent".to_string(),
        },
        rusqlite::Error::SqliteFailure(_, Some(ref msg))
            if msg.contains("no such table: journal_meta") =>
        {
            Error::CorruptJournalMeta {
                detail: "the journal_meta table is absent (this file is not a journal)".to_string(),
            }
        }
        other => Error::Sqlite(other),
    }
}
