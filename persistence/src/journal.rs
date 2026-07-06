//! The [`Journal`] — a local SQLite journal file with identity (ADD6) and a migrations harness.
//!
//! **Identity and time are caller-supplied** (ADD15 injected Clock/IdGen discipline): this crate
//! NEVER calls `Uuid::new_v4()` or any clock. The app wires real sources later; tests pass fixed
//! values for full determinism.
//!
//! **Pragmas** on every read-write open/create: `journal_mode=WAL` (persistent in the file),
//! `synchronous=NORMAL`, `busy_timeout`, `foreign_keys=ON`. The newer-file read-only path applies
//! only the connection-local ones (`busy_timeout`, `foreign_keys`) and skips the WAL write — a
//! file mutation that belongs to the read-write path only. Story 5.5 added the sync-path
//! `journal_mode=DELETE` switching ([`JournalMode`]) and the single-instance lock sidecar
//! ([`lock_is_stale`] / [`clear_lock`]) implemented in this module.

use crate::error::{Error, Result};
use crate::migrations;
use rusqlite::{Connection, OpenFlags};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use steadyinvest_contract::Timestamp;
use uuid::Uuid;

/// The SQLite rollback-journal mode a journal file is opened with (Story 5.5, ADD8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalMode {
    /// `journal_mode=WAL` — the default; fast, but a `-wal`/`-shm` sidecar makes a file-sync of the
    /// live `.db` corruption-prone (the Synology risk).
    Wal,
    /// `journal_mode=DELETE` — sync-safe: no persistent `-wal` sidecar, so a live `.db` placed in a
    /// detected sync folder will not corrupt under file-level sync (ADD8).
    Delete,
}

impl JournalMode {
    fn pragma(self) -> &'static str {
        match self {
            JournalMode::Wal => "WAL",
            JournalMode::Delete => "DELETE",
        }
    }
}

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
    /// The single-instance lock guard (Story 5.5, ADD6). Dropping the `Journal` (close / switch /
    /// exit) drops this, releasing the lock. Declared **after** `conn` so the connection closes first.
    _lock: JournalLock,
}

/// An RAII guard over a journal's single-instance lock sidecar (`…-lock`). The **owning** guard
/// removes the sidecar on `Drop`, releasing the lock (Story 5.5, ADD6). A non-owning guard (a
/// same-process re-open) leaves the sidecar for the owner to clean up. Best-effort: a removal failure
/// is not reported (the process is exiting / switching; a leftover lock is reclaimable via
/// [`lock_is_stale`]).
#[derive(Debug)]
struct JournalLock {
    path: PathBuf,
    /// `true` for the handle that created the sidecar; `false` for a same-PID re-open that found it
    /// already held by this process (single-instance = single OS process — SQLite coordinates the
    /// intra-process connections itself, so a second *process* is what the lock guards against).
    owns: bool,
}

impl Drop for JournalLock {
    fn drop(&mut self) {
        if self.owns {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// The lock sidecar path for a journal (`<path>-lock`) — distinct from the SQLite `-wal`/`-shm`.
fn lock_path_for(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_os_string();
    p.push("-lock");
    PathBuf::from(p)
}

/// The `(pid, start_time)` recorded in a lock sidecar, if it parses (Story 5.5). The start-time
/// qualifies the PID against reuse: a crashed owner's PID reassigned to an unrelated process has a
/// different start-time, so the lock is correctly seen as stale rather than "still held".
fn read_lock(lock_path: &Path) -> Option<(u32, u64)> {
    let text = std::fs::read_to_string(lock_path).ok()?;
    let mut parts = text.split_whitespace();
    let pid = parts.next()?.parse::<u32>().ok()?;
    let start_time = parts.next()?.parse::<u64>().ok()?;
    Some((pid, start_time))
}

/// A process's start-time in clock ticks (Linux `/proc/<pid>/stat`, field 22). `None` when the process
/// does not exist (or `/proc` is unreadable). The `comm` field (field 2) may contain spaces/parens, so
/// parsing starts **after the last `)`** — field 22 is then index 19 of the remaining whitespace-split.
fn process_start_time(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_comm = stat.rsplit_once(')')?.1;
    after_comm.split_whitespace().nth(19)?.parse::<u64>().ok()
}

/// Whether a recorded `(pid, start_time)` names a process that is **currently alive as that same
/// process** (PID present AND its start-time matches what the lock recorded) — defeating PID reuse.
fn lock_owner_is_live(pid: u32, start_time: u64) -> bool {
    process_start_time(pid) == Some(start_time)
}

/// Acquire the single-instance lock by **atomically** creating the `…-lock` sidecar (`create_new` —
/// fails if it already exists). Records `(pid, start_time)`. A sidecar held by a **live** other process
/// → [`Error::LockHeld`]; a sidecar from this same process instance → a non-owning re-entry; a stale
/// sidecar (crashed/PID-reused owner) → also [`Error::LockHeld`] but [`lock_is_stale`] reports it
/// reclaimable. A lock-file IO failure on a **read-only** location is non-fatal: the open proceeds
/// without a lock (a journal that cannot be locked also cannot be double-written — the read-only case).
fn acquire_lock(path: &Path) -> Result<JournalLock> {
    let lock_path = lock_path_for(path);
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
    {
        Ok(mut file) => {
            let pid = std::process::id();
            let start = process_start_time(pid).unwrap_or(0);
            let _ = write!(file, "{pid} {start}");
            Ok(JournalLock {
                path: lock_path,
                owns: true,
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let us = (std::process::id(), process_start_time(std::process::id()));
            match read_lock(&lock_path) {
                // The SAME process instance (pid + start-time) re-opening — allowed (non-owning guard;
                // SQLite coordinates intra-process connections). A reused PID has a different start-time.
                Some((pid, start)) if us.1 == Some(start) && us.0 == pid => Ok(JournalLock {
                    path: lock_path,
                    owns: false,
                }),
                // A genuinely live other instance → refused.
                Some((pid, start)) if lock_owner_is_live(pid, start) => {
                    Err(Error::LockHeld { pid })
                }
                // Stale (crashed / PID-reused / unparseable) → refused here, but reclaimable.
                other => Err(Error::LockHeld {
                    pid: other.map(|(p, _)| p).unwrap_or(0),
                }),
            }
        }
        // A read-only directory / media cannot hold a lock — proceed lock-less (a read-only journal
        // cannot be double-written, so single-instance write-protection is moot). Best-effort.
        Err(_) => Ok(JournalLock {
            path: lock_path,
            owns: false,
        }),
    }
}

/// Whether the journal's lock sidecar is **stale** — present but NOT held by a live process as
/// recorded (a crashed run, a reused PID, or an unparseable/empty sidecar) (Story 5.5). A missing lock
/// is not stale (nothing to reclaim). Used to offer a reclaim without ever stealing a live lock.
pub fn lock_is_stale(path: impl AsRef<Path>) -> bool {
    let lock_path = lock_path_for(path.as_ref());
    if !lock_path.exists() {
        return false;
    }
    match read_lock(&lock_path) {
        Some((pid, start)) => !lock_owner_is_live(pid, start),
        None => true, // unparseable / empty → reclaimable
    }
}

/// Remove a journal's lock sidecar (Story 5.5) — to **reclaim** a stale lock before re-opening. The
/// caller is expected to have confirmed staleness via [`lock_is_stale`] (this does not check). A
/// missing sidecar is a success (idempotent).
pub fn clear_lock(path: impl AsRef<Path>) -> Result<()> {
    let lock_path = lock_path_for(path.as_ref());
    match std::fs::remove_file(&lock_path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::Lock {
            detail: format!(
                "the lock at {} could not be cleared: {e}",
                lock_path.display()
            ),
        }),
    }
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
        Self::create_with_mode(path, journal_id, created_at, JournalMode::Wal)
    }

    /// Create a new journal at `path` with a chosen [`JournalMode`] (Story 5.5) — `Delete` for a
    /// sync-folder location (ADD8). Acquires the single-instance lock first.
    pub fn create_with_mode(
        path: impl AsRef<Path>,
        journal_id: Uuid,
        created_at: &Timestamp,
        mode: JournalMode,
    ) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            return Err(Error::JournalExists(path.to_path_buf()));
        }
        let result = Self::create_at(path, journal_id, created_at, mode);
        if result.is_err() {
            // Best-effort cleanup — removal failure stays unreported on purpose: the create
            // error already carries the actual cause, and the file is in /the caller's/ chosen
            // location where a leftover is recoverable by hand. (The lock guard, if it was acquired,
            // already released on the failed-result drop.)
            for suffix in ["", "-wal", "-shm"] {
                let mut sidecar = path.as_os_str().to_os_string();
                sidecar.push(suffix);
                let _ = std::fs::remove_file(&sidecar);
            }
        }
        result
    }

    fn create_at(
        path: &Path,
        journal_id: Uuid,
        created_at: &Timestamp,
        mode: JournalMode,
    ) -> Result<Self> {
        // Acquire the lock BEFORE opening — a second instance is refused before touching the file. A
        // failure anywhere below drops this guard, releasing the lock.
        let lock = acquire_lock(path)?;
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        apply_read_write_pragmas(&conn, mode)?;
        // Issue #79: run every migration AND seed the `journal_meta` row in ONE transaction, so a hard
        // crash (power loss / kill -9) mid-create leaves either an empty file or a complete journal —
        // never a fully-migrated file with a zero-row `journal_meta` (which the next open would misread
        // as `CorruptJournalMeta` on a file the user never actually corrupted). SQLite DDL is
        // transactional, so any failing step rolls the whole thing back untouched.
        {
            let tx = conn.transaction()?;
            migrations::apply_all_in_tx(&tx, migrations::REGISTRY)?;
            tx.execute(
                "INSERT INTO journal_meta (id, journal_id, logical_version, created_at)
                 VALUES (1, ?1, 0, ?2)",
                rusqlite::params![journal_id.to_string(), created_at.0],
            )?;
            tx.commit()?;
        }
        Ok(Journal {
            conn,
            id: journal_id,
            newer_file_version: None,
            _lock: lock,
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
        Self::open_with_mode(path, JournalMode::Wal)
    }

    /// Open an existing journal with a chosen [`JournalMode`] (Story 5.5) — `Delete` for a sync-folder
    /// location (ADD8). Acquires the single-instance lock first.
    pub fn open_with_mode(path: impl AsRef<Path>, mode: JournalMode) -> Result<Self> {
        let path = path.as_ref();
        // Lock before touching the file — a second instance is refused up front. Any error below drops
        // this guard, releasing the lock.
        let lock = acquire_lock(path)?;
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
                _lock: lock,
            });
        }

        // Identity is read BEFORE anything writes to the file (the WAL pragma and migrations
        // both mutate it): the DB location is user-selectable, so `open` on a wrong pick — a
        // foreign SQLite database, or any non-journal file — must fail without ever writing
        // our schema into it. Every real journal has `journal_meta` from migration 1 onward.
        let id = read_journal_id(&conn)?;
        apply_read_write_pragmas(&conn, mode)?;
        migrations::run_pending(&mut conn, migrations::REGISTRY)?;
        Ok(Journal {
            conn,
            id,
            newer_file_version: None,
            _lock: lock,
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

/// Full pragma set for read-write handles. The `journal_mode` (WAL or the sync-safe DELETE, Story 5.5)
/// is persistent in the DB file; `synchronous=NORMAL` pairs with WAL, `FULL` is safer for DELETE on a
/// sync target.
fn apply_read_write_pragmas(conn: &Connection, mode: JournalMode) -> Result<()> {
    apply_connection_local_pragmas(conn)?;
    conn.pragma_update(None, "journal_mode", mode.pragma())?;
    let synchronous = match mode {
        JournalMode::Wal => "NORMAL",
        JournalMode::Delete => "FULL",
    };
    conn.pragma_update(None, "synchronous", synchronous)?;
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
