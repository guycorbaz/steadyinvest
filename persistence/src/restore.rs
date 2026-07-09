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
/// `live_path` — copying over an open SQLite file is unsafe. The swap is done as **copy-to-temp then
/// rename**: the backup is copied to a sibling `…-restore-incoming` temp, then `std::fs::rename`d over
/// `live_path` (atomic on the same filesystem). A failure mid-copy therefore leaves `live_path`
/// **untouched** (the original survives) — never a half-written, corrupt live journal (the
/// `fs::copy`-in-place hazard: it truncates the destination first). The **live** `-wal`/`-shm` sidecars
/// are then removed (a stale WAL from the pre-restore journal would corrupt the restored file on the
/// next open). Sidecar/temp removal is best-effort. The restored file is migrated forward (if older) by
/// the caller's subsequent `Journal::open`.
pub fn restore_journal_file(live_path: &Path, backup_path: &Path) -> Result<()> {
    let mut incoming = live_path.as_os_str().to_os_string();
    incoming.push("-restore-incoming");
    let incoming = std::path::PathBuf::from(incoming);

    // Copy into the temp first; a failure here leaves the live file untouched.
    if let Err(e) = std::fs::copy(backup_path, &incoming) {
        let _ = std::fs::remove_file(&incoming);
        return Err(Error::Restore {
            detail: format!("the copy did not complete: {e}"),
        });
    }
    // Atomic replace. (No explicit fsync of the temp before the rename: on a crash between the two,
    // ext4's rename heuristics flush the data; the worst case on other filesystems is an empty/short
    // live file, recovered by re-running the restore — the validated backup itself is never touched.)
    if let Err(e) = std::fs::rename(&incoming, live_path) {
        let _ = std::fs::remove_file(&incoming);
        return Err(Error::Restore {
            detail: format!("the staged file did not replace the journal: {e}"),
        });
    }
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = live_path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let _ = std::fs::remove_file(&sidecar); // best-effort: a missing sidecar is fine
    }
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
}
