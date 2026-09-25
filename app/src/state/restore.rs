//! Restore-from-backup (Story 5.4, FR61): assess a candidate backup **read-only** (integrity +
//! schema-version + identity against the live journal) and **park** it — a restore is never applied
//! silently. Confirm re-validates the file at apply time (TOCTOU), checkpoints + snapshots the live
//! journal, swaps the file **atomically** (temp + rename) and rolls back to the snapshot if the
//! restored file will not open — the user's original journal is never lost. A restore of the
//! journal onto itself is a safe no-op (it sidesteps the `fs::copy`-onto-itself truncation hazard).

use std::path::{Path, PathBuf};

use steadyinvest_persistence::{
    Error as PersistError, Journal, inspect_backup, restore_journal_file,
};
use uuid::Uuid;

use super::{
    JournalState, MSG_NO_JOURNAL, MSG_READ_ONLY_WRITE, MSG_RESTORE_CHECKPOINT_FAILED,
    MSG_RESTORE_FAILED, MSG_RESTORE_INTEGRITY, MSG_RESTORE_NEWER_SCHEMA, MSG_RESTORE_NOT_A_JOURNAL,
    MSG_RESTORE_SNAPSHOT_FAILED, MSG_RESTORE_UNCHECKPOINTED, MSG_RESTORE_UNREADABLE,
    path_with_suffix, restore_rollback_failed_message, restore_snapshot_exists_message,
    same_file_path, sync_mode_for,
};

/// How a candidate backup compares to the current journal (Story 5.4, AC2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreVerdict {
    /// Same journal, backup version ≥ current — a safe forward restore.
    Ok,
    /// Same journal, backup is **older** than the current journal.
    StaleOlder { backup: u64, current: u64 },
    /// A backup belonging to a **different** journal.
    ForeignJournal,
    /// The backup was written by a schema **newer** than this build supports (hard refusal).
    NewerSchema { found: i64, supported: u32 },
    /// `PRAGMA integrity_check` failed (hard refusal).
    IntegrityFailed,
    /// Issue #67: a non-empty sibling `-wal` sits next to the backup — a raw copy of a live,
    /// un-checkpointed journal. Its WAL-resident commits are invisible to the validation AND to
    /// the restore copy, so applying it would silently drop them (hard refusal — the honest fix
    /// is re-creating the backup from the app, whose `create_backup` checkpoints first).
    UncheckpointedWal,
}

/// A backup assessed against the current journal (Story 5.4) — the backup's surfaced identity plus the
/// verdict that gates the confirm flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreAssessment {
    pub journal_id: Uuid,
    pub logical_version: u64,
    pub verdict: RestoreVerdict,
}

impl RestoreAssessment {
    /// A hard refusal (newer schema / failed integrity) offers **no** confirm — only the soft verdicts
    /// (Ok / StaleOlder / ForeignJournal) park a pending restore the user can confirm.
    fn is_confirmable(&self) -> bool {
        matches!(
            self.verdict,
            RestoreVerdict::Ok | RestoreVerdict::StaleOlder { .. } | RestoreVerdict::ForeignJournal
        )
    }
}

/// A validated backup parked awaiting an explicit confirm (Story 5.4). Only the path is needed to
/// apply — the assessment was already surfaced to the user by `request_restore`.
#[derive(Debug, Clone)]
pub(crate) struct PendingRestore {
    backup_path: PathBuf,
}

impl JournalState {
    /// Assess a candidate backup `.db` against the current journal and **park** it for confirmation
    /// (Story 5.4, AC1/AC2). Validates read-only (integrity + schema-version + identity), never
    /// touching the live journal. A soft verdict (Ok / StaleOlder / ForeignJournal) parks a pending
    /// restore and returns the assessment for the UI to surface + confirm; a hard refusal (corrupt /
    /// newer-schema / unreadable / not-a-journal) parks nothing and returns the neutral cause. FR61:
    /// nothing is applied here.
    pub fn request_restore(&mut self, backup_path: &str) -> Result<RestoreAssessment, String> {
        self.pending_restore = None;
        // G1 G (on-screen check): a read-only dossier is never overwritten — refused before the
        // backup is even read, so no confirm is ever parked for it.
        self.refuse_if_read_only().map_err(str::to_string)?;
        // G1 P review (M2): a leftover `-prerestore` (possibly the only copy of an original) is
        // named BEFORE the confirm — never a confirm that the apply then refuses.
        if let Some(live) = self.path.as_deref() {
            let snapshot = path_with_suffix(live, "-prerestore");
            if std::fs::symlink_metadata(&snapshot).is_ok() {
                return Err(restore_snapshot_exists_message(&snapshot));
            }
        }
        let info = inspect_backup(backup_path).map_err(|error| match error {
            PersistError::CorruptJournalMeta { .. } => MSG_RESTORE_NOT_A_JOURNAL.to_string(),
            _ => MSG_RESTORE_UNREADABLE.to_string(),
        })?;

        let verdict = if !info.integrity_ok {
            RestoreVerdict::IntegrityFailed
        } else if info.is_newer_schema() {
            RestoreVerdict::NewerSchema {
                found: info.file_user_version,
                supported: info.supported_version,
            }
        } else if info.uncheckpointed_wal
            && !self
                .path
                .as_deref()
                .is_some_and(|live| same_file_path(live, Path::new(backup_path)))
        {
            // Issue #67: everything validated above reflects only the last-checkpointed state —
            // the version surfaced would lie about the backup's real contents. Refuse. The one
            // exception is the LIVE journal chosen as its own "backup": its open handle keeps a
            // legitimate -wal, and the confirm path no-ops on the same-path guard before any copy
            // could drop anything.
            RestoreVerdict::UncheckpointedWal
        } else {
            match self.journal.as_ref() {
                // No journal open → nothing to clash with; a forward restore.
                None => RestoreVerdict::Ok,
                Some(journal) if journal.id() != info.journal_id => RestoreVerdict::ForeignJournal,
                Some(journal) => {
                    // The live dossier's version could not be read: no comparison can be stated —
                    // refused, the English cause logged (G1 final review, L13).
                    let current = journal.logical_version().map_err(|error| {
                        tracing::warn!(
                            "restore: the live journal's version is unreadable: {error}"
                        );
                        MSG_RESTORE_FAILED.to_string()
                    })?;
                    if info.logical_version < current {
                        RestoreVerdict::StaleOlder {
                            backup: info.logical_version,
                            current,
                        }
                    } else {
                        RestoreVerdict::Ok
                    }
                }
            }
        };

        let assessment = RestoreAssessment {
            journal_id: info.journal_id,
            logical_version: info.logical_version,
            verdict: verdict.clone(),
        };

        if assessment.is_confirmable() {
            self.pending_restore = Some(PendingRestore {
                backup_path: PathBuf::from(backup_path),
            });
            Ok(assessment)
        } else {
            // A hard refusal — surface the cause, park nothing (confirm can't fire).
            Err(match verdict {
                RestoreVerdict::IntegrityFailed => MSG_RESTORE_INTEGRITY.to_string(),
                RestoreVerdict::NewerSchema { .. } => MSG_RESTORE_NEWER_SCHEMA.to_string(),
                RestoreVerdict::UncheckpointedWal => MSG_RESTORE_UNCHECKPOINTED.to_string(),
                _ => MSG_RESTORE_UNREADABLE.to_string(),
            })
        }
    }

    /// Apply the parked restore (Story 5.4, AC3) **safely**: re-validate the file at confirm time
    /// (TOCTOU — the parked path may have changed), checkpoint + snapshot the live journal, swap the
    /// file **atomically** (temp + rename, so a failure leaves the live journal intact), reopen, reset
    /// undo. If the restored file will not open, **roll back to the snapshot** so the user's original
    /// journal is never lost. A restore of the journal **onto itself** is a no-op. A neutral no-op
    /// error if nothing is parked.
    pub fn confirm_restore(&mut self) -> Result<(), String> {
        let pending = self
            .pending_restore
            .take()
            .ok_or(MSG_RESTORE_FAILED.to_string())?;
        // G1 review (Guy's decision 5): a dossier open READ-ONLY (written by a newer version) is
        // never overwritten — the confirm dialog names the reason and blocks its verb, and the rule
        // lives HERE too, so no other caller can bypass it. The parked restore is dropped.
        if self.read_only {
            return Err(MSG_READ_ONLY_WRITE.to_string());
        }
        let live = self.path.clone().ok_or(MSG_NO_JOURNAL.to_string())?;

        // Restoring the journal onto itself is a no-op — the live journal already IS this content (and
        // it sidesteps the `fs::copy`-onto-itself truncation hazard). The live handle stays open.
        if same_file_path(&live, &pending.backup_path) {
            return Ok(());
        }

        // Re-validate at confirm time: the file may have changed since `request_restore` parked it. A
        // now-corrupt / newer-schema / unreadable backup is refused **without touching** the live
        // journal — the "validate BEFORE overwrite" guarantee (FR61) holds against TOCTOU.
        let info = inspect_backup(&pending.backup_path).map_err(|error| match error {
            PersistError::CorruptJournalMeta { .. } => MSG_RESTORE_NOT_A_JOURNAL.to_string(),
            _ => MSG_RESTORE_UNREADABLE.to_string(),
        })?;
        if !info.integrity_ok {
            return Err(MSG_RESTORE_INTEGRITY.to_string());
        }
        if info.is_newer_schema() {
            return Err(MSG_RESTORE_NEWER_SCHEMA.to_string());
        }
        // Issue #67 holds at confirm time too (TOCTOU): a `-wal` that appeared beside the parked
        // path since the assessment means the file is now a live journal's raw copy — refuse.
        if info.uncheckpointed_wal {
            return Err(MSG_RESTORE_UNCHECKPOINTED.to_string());
        }

        // Checkpoint the live journal so its `.db` is self-contained, then drop the handle (one
        // connection per Journal — swapping over an open file is unsafe) and snapshot it for rollback.
        // G1 final review (L12): both are PRECONDITIONS, never best effort — an un-checkpointed
        // `.db` would make the snapshot miss the dossier's latest writes, and without a snapshot a
        // restored file that will not open could not be rolled back. Either failure refuses the
        // restore by name, before anything is touched (the handle stays open on a checkpoint
        // failure; the live journal is reopened on a snapshot failure).
        // G1 P (G3 M1): a path without an open handle cannot be checkpointed — never skipped.
        let Some(journal) = self.journal.as_ref() else {
            return Err(MSG_NO_JOURNAL.to_string());
        };
        if let Err(error) = journal.checkpoint() {
            tracing::warn!("restore refused: the live journal could not be checkpointed: {error}");
            return Err(MSG_RESTORE_CHECKPOINT_FAILED.to_string());
        }
        // G1 P (G3 M1): a `-prerestore` left by an earlier restore (whose rollback failed) may be
        // the ONLY copy of an original — never overwritten, never deleted: the restore is refused,
        // the file named.
        let snapshot = path_with_suffix(&live, "-prerestore");
        if std::fs::symlink_metadata(&snapshot).is_ok() {
            return Err(restore_snapshot_exists_message(&snapshot));
        }
        self.journal = None;
        if let Err(failure) = write_snapshot(&live, &snapshot) {
            tracing::warn!(
                "restore refused: the safety snapshot could not be written: {}",
                failure.error
            );
            // G1 P review (L-a): a partial copy is no snapshot — removed only when THIS restore
            // created the file; one that appeared meanwhile (AlreadyExists) is not ours.
            if failure.created {
                remove_snapshot(&snapshot);
            }
            self.reopen_live(&live);
            return Err(MSG_RESTORE_SNAPSHOT_FAILED.to_string());
        }

        // Atomic swap — a failure leaves the live file untouched, so the original survives. The
        // persistence error's (English) text is logged, never appended to the French refusal
        // (G1 final review, L13).
        if let Err(error) = restore_journal_file(&live, &pending.backup_path) {
            tracing::warn!("restore swap failed: {error}");
            remove_snapshot(&snapshot);
            self.reopen_live(&live);
            return Err(MSG_RESTORE_FAILED.to_string());
        }

        self.open_swapped(
            &live,
            &snapshot,
            |path| Journal::open_with_mode(path, sync_mode_for(path)),
            restore_journal_file,
        )
    }

    /// The tail of a restore once the file was swapped: open the restored file, or — when it will
    /// not open — roll the snapshot back. `open` / `rollback` are the real calls in production
    /// (injected so the failure paths are testable). A failed ROLLBACK is its own refusal (G1 P,
    /// G3 M1): the dossier WAS replaced, and the snapshot — then the only copy of the original —
    /// stays on disk, named.
    pub(super) fn open_swapped(
        &mut self,
        live: &Path,
        snapshot: &Path,
        open: impl FnOnce(&Path) -> Result<Journal, PersistError>,
        rollback: impl FnOnce(&Path, &Path) -> Result<(), PersistError>,
    ) -> Result<(), String> {
        match open(live) {
            Ok(journal) => {
                remove_snapshot(snapshot);
                self.read_only = journal.is_read_only();
                self.journal = Some(journal);
                self.reset_undo();
                Ok(())
            }
            Err(error) => {
                // The swap succeeded but the restored file will not open — roll the snapshot back so
                // the user's original journal is not lost, then reopen it.
                tracing::warn!("the restored journal will not open: {error}");
                if let Err(rollback_error) = rollback(live, snapshot) {
                    // G1 P review (L-f): the kept copy's path is logged, and named again at the
                    // next start (a sibling `-prerestore` of the dossier is reported then).
                    tracing::warn!(
                        snapshot = %snapshot.display(),
                        "rollback to the pre-restore snapshot failed: {rollback_error}"
                    );
                    self.reopen_live(live);
                    return Err(restore_rollback_failed_message(snapshot));
                }
                remove_snapshot(snapshot);
                self.reopen_live(live);
                Err(MSG_RESTORE_FAILED.to_string())
            }
        }
    }

    /// Discard a parked restore (Story 5.4) — no write.
    pub fn cancel_restore(&mut self) {
        self.pending_restore = None;
    }

    /// Test-only: whether a restore is currently parked awaiting confirmation (Story 5.4).
    #[cfg(test)]
    pub(crate) fn has_pending_restore(&self) -> bool {
        self.pending_restore.is_some()
    }

    /// Best-effort reopen of the live journal at `path` (used to recover after a failed restore swap so
    /// the app is never left journal-less).
    fn reopen_live(&mut self, path: &Path) {
        match Journal::open_with_mode(path, sync_mode_for(path)) {
            Ok(journal) => {
                self.read_only = journal.is_read_only();
                self.journal = Some(journal);
            }
            Err(error) => {
                tracing::warn!("could not reopen journal after a failed restore: {error}");
                // G3 #10 (the L10 rule): no journal open → no path either, so nothing reads the
                // live path as an open dossier.
                self.journal = None;
                self.path = None;
                self.read_only = false;
            }
        }
    }
}

/// Why the snapshot could not be written, and whether THIS call created the file (so only then
/// may a partial copy be removed — G1 P review L-a).
#[derive(Debug)]
pub(super) struct SnapshotFailure {
    pub(super) created: bool,
    pub(super) error: std::io::Error,
}

/// Write the pre-restore snapshot as a NEW file (never over an existing one — the TOCTOU twin of
/// the `-prerestore` existence refusal above), carrying the dossier's permissions (G1 P review
/// L-b: the copy of a private dossier stays private).
pub(super) fn write_snapshot(live: &Path, snapshot: &Path) -> Result<(), SnapshotFailure> {
    let not_created = |error| SnapshotFailure {
        created: false,
        error,
    };
    let created = |error| SnapshotFailure {
        created: true,
        error,
    };
    let mut source = std::fs::File::open(live).map_err(not_created)?;
    let permissions = source.metadata().map_err(not_created)?.permissions();
    let mut target = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(snapshot)
        .map_err(not_created)?;
    target.set_permissions(permissions).map_err(created)?;
    std::io::copy(&mut source, &mut target).map_err(created)?;
    target.sync_all().map_err(created)
}

/// Remove this restore's own snapshot; a failure is logged with the path (G1 P review M2) — the
/// file would then block the next restore, which names it.
fn remove_snapshot(snapshot: &Path) {
    if let Err(error) = std::fs::remove_file(snapshot) {
        tracing::warn!(
            snapshot = %snapshot.display(),
            "the pre-restore snapshot could not be removed: {error}"
        );
    }
}
