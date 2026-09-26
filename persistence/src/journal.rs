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
//!
//! **Write protection** (2026-09-26 on-screen defect): a file or directory the OS will not let us
//! write opens read-only with its named [`ReadOnlyCause`] — detected at open, never discovered by
//! the first write's SQLite error.

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

/// Why an open journal is read-only — each cause is named apart, so the refusal of a write says
/// the RIGHT reason (a newer schema is not a protected file).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnlyCause {
    /// The file was written by a newer schema than this build knows (NFR-R3).
    NewerSchema { file_user_version: i64 },
    /// The OS refuses to write the file itself (`chmod 444`, a read-only medium).
    FileWriteProtected,
    /// The file is writable but its directory is not: SQLite cannot create the `-wal`/`-journal`
    /// sidecar a write needs, so the first write would fail.
    DirectoryWriteProtected,
}

/// An open journal: one SQLite connection plus the journal's identity. Single connection per
/// `Journal` is enough for this headless story (the mutex-guarded write connection + WAL
/// concurrent readers is app-era machinery).
#[derive(Debug)]
pub struct Journal {
    pub(crate) conn: Connection,
    id: Uuid,
    /// `Some(cause)` when the journal is read-only — a file newer than this build's latest
    /// migration (NFR-R3), or a file/directory protected against writing — and write methods fail
    /// with the cause-named error.
    read_only: Option<ReadOnlyCause>,
    /// The private read copy a protected journal is read through (see
    /// [`Journal::open_write_protected`]), removed on drop. Declared **after** `conn` so the
    /// connection closes before its files go.
    _scratch: Option<ScratchCopy>,
    /// The single-instance lock guard (Story 5.5, ADD6). Dropping the `Journal` (close / switch /
    /// exit) drops this, releasing the lock. Declared **after** `conn` so the connection closes first.
    _lock: JournalLock,
}

/// A private copy of a protected journal (its `.db` and any `-wal` / `-journal` holding content)
/// in a fresh directory under the OS temp dir. Reading THROUGH it is what lets a protected journal
/// with unconsolidated writes be read at all without touching its own directory: SQLite needs a
/// `-shm` beside a WAL file it reads, and would otherwise create one beside the protected file —
/// with that file's `r--r--r--` mode, which then breaks the first write once the protection is
/// lifted (G3 M1) — or fail outright in a protected directory (G3 M2). Removed on drop.
#[derive(Debug)]
struct ScratchCopy {
    dir: PathBuf,
}

impl Drop for ScratchCopy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl ScratchCopy {
    /// Copy `path` and its content-holding sidecars into a new private directory; returns the
    /// guard and the copy's `.db` path. The directory name is the process id plus a per-process
    /// counter (no clock, no random identity — ADD15), created exclusively.
    fn of(path: &Path) -> Result<(Self, PathBuf)> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir();
        let dir = loop {
            let candidate = base.join(format!(
                "steadyinvest-read-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => {
                    return Err(Error::ReadCopy {
                        detail: format!("its directory could not be created: {e}"),
                    });
                }
            }
        };
        let guard = ScratchCopy { dir };
        let copy = guard.dir.join("journal.db");
        for suffix in ["", "-wal", "-journal"] {
            let from = with_suffix(path, suffix);
            if suffix.is_empty() || std::fs::metadata(&from).is_ok_and(|m| m.len() > 0) {
                std::fs::copy(&from, with_suffix(&copy, suffix)).map_err(|e| Error::ReadCopy {
                    detail: format!("a file could not be copied: {e}"),
                })?;
            }
        }
        Ok((guard, copy))
    }
}

/// `path` with `suffix` appended to its file name (`journal.db` + `-wal`).
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut p = path.as_os_str().to_os_string();
    p.push(suffix);
    PathBuf::from(p)
}

/// The journal path as the filesystem resolves it (G3 L3): a symlinked dossier has its lock, its
/// write probe and SQLite's `-wal` / `-shm` all beside the TARGET — never the lock beside the link
/// and the sidecars beside the target. A path that does not exist yet (a create) resolves through
/// its parent; anything unresolvable stays as given.
fn resolved(path: &Path) -> PathBuf {
    if let Ok(real) = std::fs::canonicalize(path) {
        return real;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
            std::fs::canonicalize(parent).map_or_else(|_| path.to_path_buf(), |p| p.join(name))
        }
        _ => path.to_path_buf(),
    }
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
    with_suffix(&resolved(path), "-lock")
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
///
/// The sidecar's creation doubles as the **directory write probe**: the second value is `false` when
/// the OS refused to create a file beside the journal (permission denied / read-only file system),
/// which is exactly the refusal SQLite would meet creating its `-wal`/`-journal` at the first write.
fn acquire_lock(path: &Path) -> Result<(JournalLock, bool)> {
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
            Ok((
                JournalLock {
                    path: lock_path,
                    owns: true,
                },
                true,
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let us = (std::process::id(), process_start_time(std::process::id()));
            match read_lock(&lock_path) {
                // The SAME process instance (pid + start-time) re-opening — allowed (non-owning guard;
                // SQLite coordinates intra-process connections). A reused PID has a different start-time.
                // The sidecar was created there, so the directory took a new file.
                Some((pid, start)) if us.1 == Some(start) && us.0 == pid => Ok((
                    JournalLock {
                        path: lock_path,
                        owns: false,
                    },
                    true,
                )),
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
        // cannot be double-written, so single-instance write-protection is moot). Best-effort. Only
        // the OS's write refusals mark the directory protected; any other failure keeps the
        // lock-less read-write open (SQLite then reports what it meets).
        Err(e) => Ok((
            JournalLock {
                path: lock_path,
                owns: false,
            },
            !matches!(
                e.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem
            ),
        )),
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
        let (lock, directory_writable) = acquire_lock(path)?;
        // G3 L2: a directory that refused the lock sidecar refuses the new file too — named as
        // the protected directory it is, never SQLite's « unable to open » (read as « missing »).
        if !directory_writable {
            return Err(Error::WriteProtected { directory: true });
        }
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
            read_only: None,
            _scratch: None,
            _lock: lock,
        })
    }

    /// Open an existing journal file.
    ///
    /// Runs pending migrations when the file is older than this build. When the file's
    /// `user_version` is **newer** than the latest known migration, the journal opens
    /// **read-only** (NFR-R3): the handle is re-opened with `SQLITE_OPEN_READ_ONLY`, only
    /// connection-local pragmas apply, no migration runs, and write methods return the
    /// cause-named error while reads keep working. A file or directory **protected against
    /// writing** opens read-only the same way, with its own cause ([`ReadOnlyCause`]).
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_mode(path, JournalMode::Wal)
    }

    /// Open an existing journal with a chosen [`JournalMode`] (Story 5.5) — `Delete` for a sync-folder
    /// location (ADD8). Acquires the single-instance lock first.
    pub fn open_with_mode(path: impl AsRef<Path>, mode: JournalMode) -> Result<Self> {
        // G3 L3: the lock, the probes and SQLite's sidecars all work on the resolved file.
        let path = &resolved(path.as_ref());
        // Lock before touching the file — a second instance is refused up front. Any error below drops
        // this guard, releasing the lock.
        let (lock, directory_writable) = acquire_lock(path)?;
        // No CREATE flag: opening a missing file is an error, never a silent empty journal.
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        // Write-protection probe, BEFORE the first read: SQLite's own detection (a READ_WRITE open
        // of a file the OS will not let it write falls back to read-only and says so) for the file,
        // the lock sidecar's creation for the directory. Checked before any read because the first
        // read of a WAL file creates `-wal`/`-shm` beside it — with the protected file's own
        // `r--r--r--` mode, strays left behind after close (seen on screen 2026-09-26).
        let mut protection = if conn.is_readonly(rusqlite::MAIN_DB)? {
            Some(ReadOnlyCause::FileWriteProtected)
        } else if !directory_writable {
            Some(ReadOnlyCause::DirectoryWriteProtected)
        } else {
            None
        };
        // G3 M1: a writable file whose `-wal` / `-shm` is NOT writable (left `r--r--r--` by an
        // earlier read-only open, before this fix, or by any other tool) would open writable and
        // fail at the first write. Such a sidecar is ours to repair (owner write added back); when
        // it cannot be, the file is treated as protected — up front, never at the first write.
        if protection.is_none() && !repair_read_only_sidecars(path) {
            protection = Some(ReadOnlyCause::FileWriteProtected);
        }
        if let Some(cause) = protection {
            drop(conn);
            return Self::open_write_protected(path, cause, lock);
        }

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
                read_only: Some(ReadOnlyCause::NewerSchema {
                    file_user_version: file_version,
                }),
                _scratch: None,
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
            read_only: None,
            _scratch: None,
            _lock: lock,
        })
    }

    /// The read-only open of a journal the OS will not let us write (`cause` is file or directory
    /// protection). Nothing is ever created beside the protected file:
    ///
    /// - a protected FILE with no `-wal` / `-journal` holding content is read in place,
    ///   `mode=ro&immutable=1` — SQLite reads it as it stands and creates no sidecar. Immutability
    ///   holds because this process cannot change the file (G3 L4: only for the protected FILE);
    /// - otherwise — a protected directory (its file may still be written by others, and SQLite
    ///   could not create the `-shm` a WAL read needs there, G3 M2), or unconsolidated writes in a
    ///   sidecar (a `-shm` would be created beside the protected file, `r--r--r--`, and break the
    ///   first write once the protection is lifted, G3 M1) — it is read through a private COPY
    ///   ([`ScratchCopy`]) of the file and its content-holding sidecars, which SQLite recovers there.
    ///
    /// A file newer than this build keeps the newer-schema cause (the more lasting reason: lifting
    /// the protection would not make it writable); a file OLDER than this build is refused
    /// ([`Error::WriteProtectedOutdated`]) — its migrations cannot run, and this build's queries
    /// would misread an unmigrated file.
    fn open_write_protected(path: &Path, cause: ReadOnlyCause, lock: JournalLock) -> Result<Self> {
        let sidecar_has_content = ["-wal", "-journal"]
            .iter()
            .any(|suffix| std::fs::metadata(with_suffix(path, suffix)).is_ok_and(|m| m.len() > 0));
        let in_place = cause == ReadOnlyCause::FileWriteProtected && !sidecar_has_content;
        let (conn, scratch) = if in_place {
            let conn = Connection::open_with_flags(
                format!("{}?mode=ro&immutable=1", file_uri(path)),
                OpenFlags::SQLITE_OPEN_READ_ONLY
                    | OpenFlags::SQLITE_OPEN_URI
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            (conn, None)
        } else {
            // The copy is private and writable: SQLite replays a `-wal` / rolls back a hot journal
            // THERE. The journal stays read-only through the API gate (`check_writable`).
            let (scratch, copy) = ScratchCopy::of(path)?;
            let conn = Connection::open_with_flags(
                &copy,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            (conn, Some(scratch))
        };
        apply_connection_local_pragmas(&conn)?;
        let file_version = migrations::user_version(&conn)?;
        let latest = migrations::latest_version(migrations::REGISTRY);
        let cause = match file_version.cmp(&i64::from(latest)) {
            std::cmp::Ordering::Greater => ReadOnlyCause::NewerSchema {
                file_user_version: file_version,
            },
            std::cmp::Ordering::Less => {
                return Err(Error::WriteProtectedOutdated {
                    file_user_version: file_version,
                    supported: latest,
                    // G3 M5: which is protected — the file wins when both are.
                    directory: cause == ReadOnlyCause::DirectoryWriteProtected,
                });
            }
            std::cmp::Ordering::Equal => cause,
        };
        let id = read_journal_id(&conn)?;
        Ok(Journal {
            conn,
            id,
            read_only: Some(cause),
            _scratch: scratch,
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

    /// True when the journal is opened read-only (a newer schema, or write protection).
    pub fn is_read_only(&self) -> bool {
        self.read_only.is_some()
    }

    /// Why the journal is read-only, or `None` when it is writable.
    pub fn read_only_cause(&self) -> Option<ReadOnlyCause> {
        self.read_only
    }

    /// Write a self-contained copy of the journal AS THIS CONNECTION READS IT to `dest` (`VACUUM
    /// INTO`): every committed page, those still in a `-wal` included, in one standalone file with
    /// no sidecar. Works on a read-only handle too (G3 M3: a protected dossier's backup used to
    /// copy only its `.db` and silently drop the commits still in its `-wal`). `dest` must not
    /// exist — SQLite refuses to overwrite.
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        let dest = dest.to_str().ok_or_else(|| Error::Restore {
            detail: "the backup path is not valid UTF-8".to_string(),
        })?;
        self.conn.execute("VACUUM INTO ?1", [dest])?;
        Ok(())
    }

    /// Checkpoint the WAL into the main database file and truncate it (`PRAGMA
    /// wal_checkpoint(TRUNCATE)`) — on close, so the file left behind is self-contained. A
    /// read-safe operation that changes no logical data. A no-op on a read-only handle (nothing to
    /// checkpoint there).
    pub fn checkpoint(&self) -> Result<()> {
        // A read-only handle (for any reason) cannot checkpoint — a no-op rather than a hard error.
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
        match self.read_only {
            Some(ReadOnlyCause::NewerSchema { file_user_version }) => {
                Err(Error::NewerJournalSchema {
                    file_user_version,
                    supported: migrations::latest_version(migrations::REGISTRY),
                })
            }
            Some(ReadOnlyCause::FileWriteProtected) => {
                Err(Error::WriteProtected { directory: false })
            }
            Some(ReadOnlyCause::DirectoryWriteProtected) => {
                Err(Error::WriteProtected { directory: true })
            }
            None => Ok(()),
        }
    }
}

/// A filesystem path as an SQLite `file:` URI: every byte outside the unreserved set (and `/`)
/// percent-encoded, so a `?`, `#`, `%` or space in a user-chosen folder name cannot be read as URI
/// syntax and open a different file. An absolute path carries the explicit `localhost` authority
/// (G3 L5): `file:` + a path starting `//` would otherwise read its first segment as a host.
fn file_uri(path: &Path) -> String {
    use std::fmt::Write as _;
    let mut out = String::from(if path.is_absolute() {
        "file://localhost"
    } else {
        "file:"
    });
    for &byte in path.as_os_str().as_encoded_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// G3 M1: make any existing `-wal` / `-shm` of a writable journal writable again. SQLite gives a
/// sidecar the mode of its database file, so a read of a `r--r--r--` file (before the 2026-09-26
/// fix, or by any other SQLite tool) leaves `r--r--r--` sidecars; once the file is writable again,
/// its first write then fails « readonly » while nothing is protected. Only the OWNER write bit is
/// added back (the file's own mode stays as the user set it). `false` when a read-only sidecar
/// could not be repaired (not ours) — the caller then opens the journal read-only, up front.
fn repair_read_only_sidecars(path: &Path) -> bool {
    ["-wal", "-shm"].iter().all(|suffix| {
        let sidecar = with_suffix(path, suffix);
        let Ok(meta) = std::fs::metadata(&sidecar) else {
            return true; // absent: nothing to repair
        };
        let writable = std::fs::OpenOptions::new()
            .write(true)
            .open(&sidecar)
            .is_ok();
        writable || add_owner_write(&sidecar, meta.permissions())
    })
}

#[cfg(unix)]
fn add_owner_write(path: &Path, mut permissions: std::fs::Permissions) -> bool {
    use std::os::unix::fs::PermissionsExt;
    permissions.set_mode(permissions.mode() | 0o200);
    std::fs::set_permissions(path, permissions).is_ok()
        && std::fs::OpenOptions::new().write(true).open(path).is_ok()
}

#[cfg(not(unix))]
fn add_owner_write(path: &Path, mut permissions: std::fs::Permissions) -> bool {
    #[allow(clippy::permissions_set_readonly_false)] // Windows: the read-only attribute only
    permissions.set_readonly(false);
    std::fs::set_permissions(path, permissions).is_ok()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_uri_never_reads_a_path_segment_as_a_host() {
        // G3 L5: `//server/x.db` must stay a path.
        assert_eq!(
            file_uri(Path::new("//home/a b/x?.db")),
            "file://localhost//home/a%20b/x%3F.db"
        );
        assert_eq!(file_uri(Path::new("/d/x.db")), "file://localhost/d/x.db");
        assert_eq!(file_uri(Path::new("rel/x.db")), "file:rel/x.db");
    }
}
