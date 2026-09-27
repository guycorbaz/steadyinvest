//! Restore from a raw `.db` backup (Story 5.4, FR61).
//!
//! The portable **backup/restore unit is the raw `.db` file** (architecture §"Export / backup format":
//! the JSON envelope of Stories 5.2/5.3 is the exchange/seed unit; the `.db` copy is the file-level
//! NAS backup unit). This module validates a candidate backup **before** any overwrite and performs
//! the file-level swap; the app owns the confirm flow and the handle lifecycle.
//!
//! - [`inspect_backup`] opens the candidate **read-only and immutable** — it never migrates or
//!   WAL-writes the backup — and reports its SQLite integrity, schema version and journal identity as
//!   a [`BackupInfo`]. The app compares that to the current journal and gates a stale/foreign restore
//!   behind a confirmation (FR61 — never applied silently).
//! - [`restore_journal_file`] copies the backup over the live path and removes the **live** stale
//!   `-wal`/`-shm` sidecars. **Precondition:** the caller has already dropped every [`Journal`] handle
//!   on the live path (one connection per handle; copying over an open SQLite file is unsafe).
//!
//! **Backup unit = a checkpointed, single-file `.db`.** Both inspect and restore consider only the
//! main `.db` (inspect opens `immutable=1`, which ignores any sibling `-wal`). App-made backups are
//! safe: [`Journal::checkpoint`](crate::Journal::checkpoint) truncates the WAL before the copy.
//!
//! Issue #67: a hand-rolled raw copy of a *live* (un-checkpointed) journal splits committed data
//! across a sibling `-wal` that `immutable=1` never reads — so everything inspect validated
//! (integrity, version, identity) reflects only the last-checkpointed state, and a restore would
//! silently drop the WAL-resident commits. [`inspect_backup`] therefore FLAGS a **non-empty**
//! sibling `-wal` (`BackupInfo::uncheckpointed_wal`) so the caller can refuse with the honest
//! cause ("re-create the backup from the app") rather than restore a file that lies about its
//! contents. An empty (zero-length) `-wal` — what `wal_checkpoint(TRUNCATE)` leaves behind — is
//! fine: the main `.db` is self-contained then.

use crate::error::{Error, Result};
use crate::migrations;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::Path;
use uuid::Uuid;

/// A read-only assessment of a candidate backup `.db` (Story 5.4) — never mutates the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupInfo {
    /// The backup's journal identity (`journal_meta.journal_id`).
    pub journal_id: Uuid,
    /// The backup's monotonic logical version (`journal_meta.logical_version`).
    pub logical_version: u64,
    /// The backup file's SQLite schema version (`PRAGMA user_version`).
    pub file_user_version: i64,
    /// The newest schema version **this build** supports.
    pub supported_version: u32,
    /// `true` when `PRAGMA integrity_check` returned `"ok"`.
    pub integrity_ok: bool,
    /// Issue #67: `true` when a **non-empty** sibling `-wal` sits next to the backup — the file is
    /// a raw copy of a live, un-checkpointed journal, and everything above reflects only its
    /// last-checkpointed state (the WAL-resident commits are invisible to the `immutable=1` open).
    /// A restore would silently drop them; the caller must refuse.
    pub uncheckpointed_wal: bool,
}

impl BackupInfo {
    /// The backup was written by a schema **newer** than this build can read (a hard refusal — this
    /// build must not restore a file it cannot open read-write afterward).
    pub fn is_newer_schema(&self) -> bool {
        self.file_user_version > i64::from(self.supported_version)
    }
}

/// Inspect a candidate backup `.db` **read-only**, without ever mutating it (Story 5.4, AC1). Opens
/// with `immutable=1` so a backup still in WAL mode (a raw copy of a live journal) is readable without
/// recovery or sidecar writes. Runs `PRAGMA integrity_check`, reads `PRAGMA user_version`, and reads
/// the `journal_meta` identity. A file that is not a journal (no `journal_meta`) is a typed
/// [`Error::CorruptJournalMeta`]; an unreadable/garbage file is a typed SQLite error. Never panics,
/// never migrates, never writes.
pub fn inspect_backup(path: impl AsRef<Path>) -> Result<BackupInfo> {
    let path = path.as_ref();
    // `immutable=1` promises SQLite the file will not change — it skips locking and WAL recovery, so a
    // read-only open succeeds even on a WAL-mode `.db` whose `-wal` is absent/stale. URI form is
    // required for the query parameter.
    let uri = immutable_file_uri(path);
    let conn = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;

    // Integrity: the first row of `integrity_check` is "ok" on a healthy database, else the first
    // detected problem. We only need the pass/fail signal.
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    let integrity_ok = integrity == "ok";

    let file_user_version = migrations::user_version(&conn)?;
    let supported_version = migrations::latest_version(migrations::REGISTRY);

    let (journal_id, logical_version) = read_meta(&conn)?;

    // Issue #67: a non-empty sibling `-wal` means the backup is a raw copy of a live journal whose
    // WAL-resident commits everything above did NOT see (`immutable=1` skips WAL recovery). A
    // zero-length `-wal` (the `wal_checkpoint(TRUNCATE)` leftover) is self-contained and fine.
    let mut wal = path.as_os_str().to_os_string();
    wal.push("-wal");
    let uncheckpointed_wal = std::fs::metadata(&wal).is_ok_and(|m| m.len() > 0);

    Ok(BackupInfo {
        journal_id,
        logical_version,
        file_user_version,
        supported_version,
        integrity_ok,
        uncheckpointed_wal,
    })
}

/// Read `journal_meta` (the `journal_id` + `logical_version`) from a read-only connection. A missing
/// table or row means the file is not a journal — [`Error::CorruptJournalMeta`], not a bare SQLite
/// error (so the app can say "this is not a journal" rather than leaking SQL).
fn read_meta(conn: &Connection) -> Result<(Uuid, u64)> {
    let row: Option<(String, i64)> = conn
        .query_row(
            "SELECT journal_id, logical_version FROM journal_meta WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(map_missing_meta)?;
    let (id_text, version) = row.ok_or_else(|| Error::CorruptJournalMeta {
        detail: "the journal_meta singleton row is absent".to_string(),
    })?;
    let journal_id = Uuid::parse_str(&id_text).map_err(|e| Error::CorruptJournalMeta {
        detail: format!("journal_id {id_text:?} is not a valid UUID: {e}"),
    })?;
    let logical_version = u64::try_from(version).map_err(|_| Error::CorruptJournalMeta {
        detail: format!("logical_version {version} is negative"),
    })?;
    Ok((journal_id, logical_version))
}

/// A missing `journal_meta` table is how we reject a valid SQLite file that was never a journal.
fn map_missing_meta(e: rusqlite::Error) -> Error {
    match e {
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

/// Replace the live journal file with a validated backup **atomically** (Story 5.4, AC3).
///
/// **Precondition:** the caller has already dropped every [`Journal`](crate::Journal) handle on
/// `live_path`. The swap is **copy-to-temp then rename**: the backup is copied to a sibling
/// `…-restore-incoming` temp, then `std::fs::rename`d over `live_path` (atomic on the same
/// filesystem) — a failure before the rename leaves `live_path` **untouched**.
///
/// **Against the MCP access surface** (Story 8.3, arch A11 — it takes no instance lock), in order:
/// 1. the **marker** `…-restoring` (owner pid + start time) is written FIRST: from then on every new
///    MCP call is refused up front (`dossier_busy`), and it is removed LAST, once the restored file
///    stands and every restore connection is closed;
/// 2. the live file **leaves WAL** and is **locked exclusively** ([`lock_live_journal`]);
/// 3. under that lock the live `-wal` / `-shm` are removed (after leaving WAL there are none — only a
///    file that could not be locked may still have some) — **nothing is deleted after the rename**,
///    where it would be the restored file's (G3 N1);
/// 4. copy, rename, release the lock, remove the marker.
///
/// The restored file is migrated forward (if older) by the caller's subsequent `Journal::open`.
pub fn restore_journal_file(live_path: &Path, backup_path: &Path) -> Result<()> {
    restore_with(
        live_path,
        backup_path,
        None,
        RESTORE_LOCK_WAIT,
        RestoreHooks::none(),
    )
}

/// [`restore_journal_file`] that first writes a **pre-restore snapshot** of the live journal to
/// `snapshot` — a NEW file (never over an existing one), with the dossier's permissions — copied
/// **under the restore's exclusive lock** (G3 N3): no MCP writer can tear it, and it holds every
/// write up to the swap. On `Ok` the snapshot exists (the caller's rollback copy); on
/// any `Err` the snapshot this call created has been removed, and a snapshot failure is its own
/// error ([`Error::RestoreSnapshot`]).
pub fn restore_journal_file_keeping(
    live_path: &Path,
    backup_path: &Path,
    snapshot: &Path,
) -> Result<()> {
    restore_with(
        live_path,
        backup_path,
        Some(snapshot),
        RESTORE_LOCK_WAIT,
        RestoreHooks::none(),
    )
}

/// How long the restore waits for the live file (MCP readers leaving WAL, a write in progress
/// ending — Story 8.3, arch A11) before refusing by name.
const RESTORE_LOCK_WAIT: std::time::Duration = std::time::Duration::from_millis(5000);

/// The marker a restore writes beside the live file for its whole duration (Story 8.3 G3 N4): the
/// MCP access surface refuses every call while it exists.
pub(crate) fn restore_marker(live_path: &Path) -> std::path::PathBuf {
    let mut p = live_path.as_os_str().to_os_string();
    p.push("-restoring");
    std::path::PathBuf::from(p)
}

/// The restore's staging copy, renamed over the live file.
pub(crate) fn restore_staging(live_path: &Path) -> std::path::PathBuf {
    let mut p = live_path.as_os_str().to_os_string();
    p.push("-restore-incoming");
    std::path::PathBuf::from(p)
}

/// Whether a restore of `live_path` is running (its marker names a live process), was interrupted
/// (a marker or a staging copy left by a process that is gone — G3 N2), or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RestoreState {
    Idle,
    Running,
    Interrupted,
}

pub(crate) fn restore_state(live_path: &Path) -> RestoreState {
    let marker = restore_marker(live_path);
    if marker.exists() {
        return match crate::journal::read_lock(&marker) {
            Some((pid, start)) if crate::journal::lock_owner_is_live(pid, start) => {
                RestoreState::Running
            }
            // A marker being written (empty for an instant) is treated as running; an unparseable
            // or dead owner's marker is a leftover.
            None if std::fs::metadata(&marker).is_ok_and(|m| m.len() == 0) => RestoreState::Running,
            _ => RestoreState::Interrupted,
        };
    }
    if restore_staging(live_path).exists() {
        return RestoreState::Interrupted;
    }
    RestoreState::Idle
}

/// Remove what an interrupted restore left beside `live_path` — its marker and its staging copy —
/// returning the paths removed. Called by `Journal::open` while it holds the single-instance lock
/// (G3 N2): no restore of the app's own can be running then (the app drops its handle to restore,
/// and reopens after). A marker of a LIVE process is never removed.
pub(crate) fn clear_interrupted_restore(live_path: &Path) -> Vec<std::path::PathBuf> {
    if restore_state(live_path) != RestoreState::Interrupted {
        return Vec::new();
    }
    [restore_marker(live_path), restore_staging(live_path)]
        .into_iter()
        .filter(|p| p.exists() && std::fs::remove_file(p).is_ok())
        .collect()
}

/// Removes the marker when the restore ends, whatever the path (G3 N4: it is the LAST step).
struct MarkerGuard(std::path::PathBuf);

impl Drop for MarkerGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn write_marker(live_path: &Path) -> Result<MarkerGuard> {
    use std::io::Write as _;
    let marker = restore_marker(live_path);
    let pid = std::process::id();
    let start = crate::journal::process_start_time(pid).unwrap_or(0);
    let mut file = std::fs::File::create(&marker).map_err(|e| Error::Restore {
        detail: format!("the restore marker could not be written: {e}"),
    })?;
    let guard = MarkerGuard(marker);
    write!(file, "{pid} {start}").map_err(|e| Error::Restore {
        detail: format!("the restore marker could not be written: {e}"),
    })?;
    Ok(guard)
}

/// Test seams of a restore: once the live file is locked, once the snapshot is written, and once
/// the file is swapped (before the lock is released and the marker removed).
#[derive(Default)]
pub(crate) struct RestoreHooks<'h> {
    pub(crate) after_lock: Option<&'h mut dyn FnMut()>,
    pub(crate) after_snapshot: Option<&'h mut dyn FnMut()>,
    pub(crate) after_swap: Option<&'h mut dyn FnMut()>,
}

impl RestoreHooks<'_> {
    fn none() -> Self {
        RestoreHooks::default()
    }
}

/// Take the live journal out of the way of every other connection before it is replaced (Story
/// 8.3, arch A11): the app's handles are dropped (the precondition), but an MCP connection takes no
/// instance lock and may be reading or writing.
///
/// 1. **Leave WAL** (`wal_checkpoint(TRUNCATE)` + `journal_mode = DELETE`): SQLite grants it only
///    when no other connection has the file open in WAL — so after it, no connection holds the old
///    file's `-wal` / `-shm`, which would otherwise alias the restored file's side files of the same
///    names. Retried while readers finish, until `wait`; then refused by name. (The app sets its own
///    journal mode again when it opens the restored file.)
/// 2. **`BEGIN EXCLUSIVE`** (DELETE mode: it blocks readers and writers) held across the swap: no MCP
///    call is in flight on the old file when it is replaced, and one that opened the old file meanwhile
///    finds another file at the path (its identity re-check — `dossier_replaced`).
///
/// `None` when there is nothing to lock: no live file, a file that is not a database (nothing can
/// be writing it), or a write-protected one (no write can land in it). Any other failure refuses the
/// restore by name ([`Error::Restore`]) — the live journal untouched.
fn lock_live_journal(live_path: &Path, wait: std::time::Duration) -> Result<Option<Connection>> {
    use rusqlite::ErrorCode as C;
    if !live_path.exists() {
        return Ok(None);
    }
    let named = |what: &str, e: &dyn std::fmt::Display| Error::Restore {
        detail: format!("{what}: {e}"),
    };
    let skip = |e: &rusqlite::Error| {
        matches!(e, rusqlite::Error::SqliteFailure(f, _)
            if matches!(f.code, C::NotADatabase | C::ReadOnly))
    };
    let busy = |e: &rusqlite::Error| {
        matches!(e, rusqlite::Error::SqliteFailure(f, _)
            if matches!(f.code, C::DatabaseBusy | C::DatabaseLocked))
    };
    let conn = Connection::open_with_flags(
        live_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| named("the live journal could not be opened for its lock", &e))?;
    match conn.is_readonly(rusqlite::MAIN_DB) {
        Ok(true) => return Ok(None),
        Ok(false) => {}
        Err(e) => {
            return Err(named(
                "the live journal could not be opened for its lock",
                &e,
            ));
        }
    }
    let deadline = std::time::Instant::now() + wait;
    conn.busy_timeout(std::time::Duration::from_millis(50))
        .map_err(|e| named("the live journal lock could not be set up", &e))?;
    // 1. Leave WAL — only possible once no other connection holds the file.
    loop {
        let left = conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
            .and_then(|()| {
                conn.query_row("PRAGMA journal_mode = DELETE", [], |r| {
                    r.get::<_, String>(0)
                })
            });
        match left {
            Ok(mode) if mode.eq_ignore_ascii_case("delete") => break,
            Ok(_) => {}
            Err(e) if skip(&e) => return Ok(None),
            Err(e) if busy(&e) => {}
            Err(e) => return Err(named("the live journal could not leave WAL mode", &e)),
        }
        if std::time::Instant::now() >= deadline {
            return Err(Error::Restore {
                detail: "another connection still uses the live journal".to_string(),
            });
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    // 2. The exclusive lock, for what is left of the wait.
    let left = deadline.saturating_duration_since(std::time::Instant::now());
    conn.busy_timeout(left.max(std::time::Duration::from_millis(1)))
        .map_err(|e| named("the live journal lock could not be set up", &e))?;
    match conn.execute_batch("BEGIN EXCLUSIVE") {
        Ok(()) => Ok(Some(conn)),
        Err(e) if busy(&e) => Err(Error::Restore {
            detail: "a write in progress on the live journal did not end in time".to_string(),
        }),
        Err(e) if skip(&e) => Ok(None),
        Err(e) => Err(named("the live journal could not be locked", &e)),
    }
}

/// [`restore_journal_file`] with a chosen wait for the live file's lock (tests use a short one).
#[cfg(test)]
pub(crate) fn restore_journal_file_with_wait(
    live_path: &Path,
    backup_path: &Path,
    wait: std::time::Duration,
) -> Result<()> {
    restore_with(live_path, backup_path, None, wait, RestoreHooks::none())
}

/// Write the pre-restore snapshot as a NEW file with the dossier's permissions — a byte copy of the
/// live file taken while the restore holds its EXCLUSIVE lock (DELETE mode, G3 N3): no other
/// connection can be writing it, so the copy is consistent and holds every write up to the swap.
/// (SQLite's backup API cannot read through a connection holding a write transaction.)
///
/// **The source descriptor is not closed here** (Unix): closing any descriptor of the file would
/// release every POSIX lock this process holds on it — the restore's exclusive lock included. It is
/// pushed into `keep`, which the caller drops only after the lock's connection is closed. On
/// Windows (no such hazard, and an open handle would block the rename) it is closed at once. `Err`
/// carries whether THIS call created the snapshot (so only then may it be removed).
fn write_snapshot(
    live_path: &Path,
    snapshot: &Path,
    keep: &mut Vec<std::fs::File>,
) -> std::result::Result<(), (bool, String)> {
    let permissions = std::fs::metadata(live_path)
        .map_err(|e| {
            (
                false,
                format!("the dossier's permissions could not be read: {e}"),
            )
        })?
        .permissions();
    let mut target = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(snapshot)
        .map_err(|e| {
            (
                false,
                format!("the snapshot file could not be created: {e}"),
            )
        })?;
    target.set_permissions(permissions).map_err(|e| {
        (
            true,
            format!("the snapshot's permissions could not be set: {e}"),
        )
    })?;
    let mut source = std::fs::File::open(live_path)
        .map_err(|e| (true, format!("the dossier could not be read: {e}")))?;
    let copied = std::io::copy(&mut source, &mut target)
        .map_err(|e| (true, format!("the snapshot could not be written: {e}")))
        .and_then(|_| {
            target
                .sync_all()
                .map_err(|e| (true, format!("the snapshot could not be flushed: {e}")))
        });
    if cfg!(unix) {
        keep.push(source);
    }
    copied
}

/// The restore itself (see [`restore_journal_file`] for the order of its steps).
pub(crate) fn restore_with(
    live_path: &Path,
    backup_path: &Path,
    snapshot: Option<&Path>,
    wait: std::time::Duration,
    mut hooks: RestoreHooks<'_>,
) -> Result<()> {
    // 1. The marker FIRST (G3 N4) — removed last, by its guard, whatever happens.
    let _marker = write_marker(live_path)?;
    // 2. Out of WAL and the exclusive lock: a failure here leaves the live file untouched.
    // `keep` holds the snapshot's source descriptor (Unix) until the lock is released: declared
    // BEFORE `lock`, so on every return it drops AFTER the lock's connection (locals drop in reverse
    // declaration order) — closing it earlier would release the lock (POSIX).
    let mut keep: Vec<std::fs::File> = Vec::new();
    let lock = lock_live_journal(live_path, wait)?;
    if let Some(hook) = hooks.after_lock.as_mut() {
        hook();
    }
    // 3. The rollback snapshot, under the lock (G3 N3).
    let mut created_snapshot: Option<&Path> = None;
    if let Some(snapshot) = snapshot {
        if let Err((created, detail)) = write_snapshot(live_path, snapshot, &mut keep) {
            if created {
                let _ = std::fs::remove_file(snapshot);
            }
            return Err(Error::RestoreSnapshot { detail });
        }
        created_snapshot = Some(snapshot);
        if let Some(hook) = hooks.after_snapshot.as_mut() {
            hook();
        }
    }
    let fail = |detail: String| {
        if let Some(snapshot) = created_snapshot {
            let _ = std::fs::remove_file(snapshot);
        }
        Error::Restore { detail }
    };
    // 4. The live side files go UNDER the lock, before the rename (G3 N1) — after leaving WAL there
    //    are none; a file that could not be locked (not a database, protected) may still have some.
    //    Nothing is deleted after the rename: it would be the RESTORED file's.
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = live_path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let _ = std::fs::remove_file(&sidecar); // best-effort: a missing side file is fine
    }
    // 5. Copy into the staging file first; a failure here leaves the live file untouched.
    let incoming = restore_staging(live_path);
    if let Err(e) = std::fs::copy(backup_path, &incoming) {
        let _ = std::fs::remove_file(&incoming);
        return Err(fail(format!("the copy did not complete: {e}")));
    }
    // Windows cannot replace a file this process holds open: the lock is released just before the
    // rename there — the residual window between an MCP write's identity check and its commit is
    // documented (Story 8.3 Dev Notes §4); the marker still refuses new MCP calls. On Unix the lock
    // is held across the rename.
    #[cfg(windows)]
    drop(lock);
    // 6. Atomic replace. (No explicit fsync of the temp before the rename: on a crash between the
    //    two, ext4's rename heuristics flush the data; the worst case on other filesystems is an
    //    empty/short live file, recovered by re-running the restore — the backup is never touched.)
    if let Err(e) = std::fs::rename(&incoming, live_path) {
        let _ = std::fs::remove_file(&incoming);
        return Err(fail(format!(
            "the staged file did not replace the journal: {e}"
        )));
    }
    if let Some(hook) = hooks.after_swap.as_mut() {
        hook();
    }
    // 7. The lock's connection belongs to the REPLACED file (DELETE mode, no write: its close
    //    touches nothing at the path); then the marker goes, last (its guard).
    #[cfg(not(windows))]
    drop(lock);
    drop(keep);
    Ok(())
}

/// Build a `file:` URI with `immutable=1`, percent-encoding the few characters that are reserved in a
/// SQLite URI (`%`, `?`, `#`, space). Local file paths rarely contain them, but a robust restore must
/// not mis-parse a path that does.
fn immutable_file_uri(path: &Path) -> String {
    let mut uri = String::from("file:");
    for ch in path.to_string_lossy().chars() {
        match ch {
            '%' => uri.push_str("%25"),
            '?' => uri.push_str("%3F"),
            '#' => uri.push_str("%23"),
            ' ' => uri.push_str("%20"),
            other => uri.push(other),
        }
    }
    uri.push_str("?immutable=1");
    uri
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Journal;
    use steadyinvest_contract::Timestamp;
    use tempfile::tempdir;

    fn ts(s: &str) -> Timestamp {
        Timestamp(s.to_string())
    }

    #[test]
    fn inspect_a_fresh_journal_reports_identity_and_integrity() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("j.db");
        let jid = Uuid::from_u128(0x5040);
        let j = Journal::create(&path, jid, &ts("2026-06-29T00:00:00Z")).unwrap();
        let before_version = j.logical_version().unwrap();
        drop(j); // release the handle before a read-only re-open

        let info = inspect_backup(&path).unwrap();
        assert!(info.integrity_ok, "a fresh journal passes integrity_check");
        assert_eq!(info.journal_id, jid);
        assert_eq!(info.logical_version, before_version);
        assert_eq!(
            info.file_user_version,
            i64::from(info.supported_version),
            "a fresh journal is at the latest schema"
        );
        assert!(!info.is_newer_schema());
    }

    #[test]
    fn inspecting_does_not_mutate_the_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("j.db");
        let jid = Uuid::from_u128(0x5041);
        Journal::create(&path, jid, &ts("2026-06-29T00:00:00Z")).unwrap();
        let v1 = inspect_backup(&path).unwrap().logical_version;
        let v2 = inspect_backup(&path).unwrap().logical_version;
        // Re-opening the live journal read-write must still show the same version — inspection wrote
        // nothing (no migration, no WAL, no version drift).
        let reopened = Journal::open(&path).unwrap().logical_version().unwrap();
        assert_eq!(v1, v2);
        assert_eq!(
            v1, reopened,
            "inspection left the backup's version untouched"
        );
    }

    /// Issue #67: a NON-EMPTY sibling `-wal` (a raw copy of a live, un-checkpointed journal) is
    /// flagged — the `immutable=1` inspection saw only the last-checkpointed state, so restoring
    /// the `.db` alone would silently drop the WAL-resident commits. The zero-length `-wal` that
    /// `wal_checkpoint(TRUNCATE)` leaves behind is self-contained and NOT flagged.
    #[test]
    fn a_nonempty_sibling_wal_flags_the_backup_as_uncheckpointed() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("copy.db");
        Journal::create(&path, Uuid::from_u128(0x67), &ts("2026-07-09T00:00:00Z")).unwrap();

        assert!(
            !inspect_backup(&path).unwrap().uncheckpointed_wal,
            "no sidecar at all — self-contained"
        );

        let mut wal = path.as_os_str().to_os_string();
        wal.push("-wal");
        std::fs::write(&wal, b"").unwrap();
        assert!(
            !inspect_backup(&path).unwrap().uncheckpointed_wal,
            "a zero-length -wal (the checkpoint TRUNCATE leftover) is self-contained"
        );

        std::fs::write(&wal, b"wal frames the .db does not contain").unwrap();
        assert!(
            inspect_backup(&path).unwrap().uncheckpointed_wal,
            "a non-empty sibling -wal is flagged — the validation reflects only the checkpointed state"
        );
    }

    #[test]
    fn a_non_journal_sqlite_file_is_not_a_journal() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("foreign.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE t (x INTEGER); INSERT INTO t VALUES (1);")
            .unwrap();
        drop(conn);
        assert!(matches!(
            inspect_backup(&path),
            Err(Error::CorruptJournalMeta { .. })
        ));
    }

    #[test]
    fn a_garbage_file_is_a_typed_error_not_a_panic() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("garbage.db");
        std::fs::write(&path, b"this is not a sqlite database at all").unwrap();
        assert!(inspect_backup(&path).is_err());
    }

    #[test]
    fn restore_swaps_content_and_clears_the_live_wal() {
        let dir = tempdir().unwrap();
        let live = dir.path().join("live.db");
        let backup = dir.path().join("backup.db");
        let live_jid = Uuid::from_u128(0xA);
        let backup_jid = Uuid::from_u128(0xB);

        // Two distinct journals.
        let lj = Journal::create(&live, live_jid, &ts("2026-06-01T00:00:00Z")).unwrap();
        drop(lj);
        let bj = Journal::create(&backup, backup_jid, &ts("2026-06-02T00:00:00Z")).unwrap();
        drop(bj);

        // Re-open the live one to produce a -wal, then drop the handle before the swap.
        {
            let mut lj = Journal::open(&live).unwrap();
            // a write to materialize a WAL
            lj.ensure_portfolio(Uuid::from_u128(0xC), "P", &ts("2026-06-03T00:00:00Z"))
                .unwrap();
        }

        restore_journal_file(&live, &backup).unwrap();

        // Immediately after the swap (before any re-open recreates them), the stale live -wal/-shm
        // sidecars are gone.
        for suffix in ["-wal", "-shm"] {
            let mut sidecar = live.as_os_str().to_os_string();
            sidecar.push(suffix);
            assert!(
                !Path::new(&sidecar).exists(),
                "the stale live {suffix} sidecar was removed by the swap"
            );
        }

        // The live path now carries the backup's identity.
        let restored = Journal::open(&live).unwrap();
        assert_eq!(
            restored.id(),
            backup_jid,
            "live now holds the backup's journal"
        );
    }

    fn two_journals(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
        let live = dir.join("live.db");
        let backup = dir.join("backup.db");
        let mut lj =
            Journal::create(&live, Uuid::from_u128(0xA), &ts("2026-06-01T00:00:00Z")).unwrap();
        lj.ensure_portfolio(
            Uuid::from_u128(0xC),
            "Original",
            &ts("2026-06-03T00:00:00Z"),
        )
        .unwrap();
        drop(lj);
        drop(Journal::create(&backup, Uuid::from_u128(0xB), &ts("2026-06-02T00:00:00Z")).unwrap());
        (live, backup)
    }

    /// G3 N3: the pre-restore snapshot is taken UNDER the restore lock (no writer can land while
    /// it is written), holds the original, carries the dossier's permissions, and the swap happens.
    #[test]
    fn the_snapshot_is_taken_under_the_lock_and_holds_the_original() {
        let dir = tempdir().unwrap();
        let (live, backup) = two_journals(dir.path());
        let snapshot = dir.path().join("live.db-prerestore");
        let mut writer_refused = None;
        restore_with(
            &live,
            &backup,
            Some(&snapshot),
            std::time::Duration::from_secs(2),
            RestoreHooks {
                // Right before the snapshot: another connection cannot take the write lock.
                after_lock: Some(&mut || {
                    let writer = Connection::open(&live).unwrap();
                    writer.busy_timeout(std::time::Duration::ZERO).unwrap();
                    writer_refused = Some(writer.execute_batch("BEGIN IMMEDIATE").is_err());
                }),
                after_snapshot: None,
                after_swap: None,
            },
        )
        .unwrap();
        assert_eq!(
            writer_refused,
            Some(true),
            "the live file was locked for the snapshot"
        );
        assert_eq!(
            Journal::open(&live).unwrap().id(),
            Uuid::from_u128(0xB),
            "swapped"
        );
        let kept = Journal::open(&snapshot).unwrap();
        assert_eq!(
            kept.id(),
            Uuid::from_u128(0xA),
            "the snapshot is the original"
        );
        assert_eq!(kept.list_portfolios().unwrap().len(), 1, "with its data");
        assert_eq!(
            std::fs::metadata(&snapshot).unwrap().permissions(),
            std::fs::metadata(&live).unwrap().permissions(),
            "the dossier's permissions (L-b)"
        );
    }

    /// G3 N3: a snapshot is never written over an existing file: the restore is refused by name,
    /// the existing file intact, the live journal unchanged.
    #[test]
    fn an_existing_snapshot_is_never_overwritten_and_the_restore_is_refused() {
        let dir = tempdir().unwrap();
        let (live, backup) = two_journals(dir.path());
        let snapshot = dir.path().join("live.db-prerestore");
        std::fs::write(&snapshot, b"earlier").unwrap();
        let err = restore_journal_file_keeping(&live, &backup, &snapshot).unwrap_err();
        assert!(matches!(err, Error::RestoreSnapshot { .. }), "{err:?}");
        assert_eq!(std::fs::read(&snapshot).unwrap(), b"earlier");
        assert_eq!(
            Journal::open(&live).unwrap().id(),
            Uuid::from_u128(0xA),
            "unchanged"
        );
        assert!(
            !restore_marker(&live).exists(),
            "the marker went with the refusal"
        );
    }

    /// A refused restore (the live file stays locked by a write) removes the snapshot it created.
    #[test]
    fn a_refused_restore_leaves_no_snapshot() {
        let dir = tempdir().unwrap();
        let (live, backup) = two_journals(dir.path());
        let snapshot = dir.path().join("live.db-prerestore");
        let holder = Connection::open(&live).unwrap();
        holder.execute_batch("BEGIN IMMEDIATE").unwrap();
        let err = restore_with(
            &live,
            &backup,
            Some(&snapshot),
            std::time::Duration::from_millis(150),
            RestoreHooks::none(),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Restore { .. }), "{err:?}");
        assert!(!snapshot.exists(), "no snapshot left");
        holder.execute_batch("ROLLBACK").unwrap();
    }
}
