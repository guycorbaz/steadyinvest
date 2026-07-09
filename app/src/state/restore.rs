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
    JournalState, MSG_NO_JOURNAL, MSG_RESTORE_FAILED, MSG_RESTORE_INTEGRITY,
    MSG_RESTORE_NEWER_SCHEMA, MSG_RESTORE_NOT_A_JOURNAL, MSG_RESTORE_UNCHECKPOINTED,
    MSG_RESTORE_UNREADABLE, MSG_SAVE_FAILED, path_with_suffix, same_file_path, sync_mode_for,
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
                    let current = journal
                        .logical_version()
                        .map_err(|error| format!("{MSG_SAVE_FAILED} {error}"))?;
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
        if let Some(journal) = self.journal.as_ref() {
            let _ = journal.checkpoint();
        }
        self.journal = None;
        let snapshot = path_with_suffix(&live, "-prerestore");
        let have_snapshot = std::fs::copy(&live, &snapshot).is_ok();

        // Atomic swap — a failure leaves the live file untouched, so the original survives.
        if let Err(error) = restore_journal_file(&live, &pending.backup_path) {
            let _ = std::fs::remove_file(&snapshot);
            self.reopen_live(&live);
            return Err(format!("{MSG_RESTORE_FAILED} {error}"));
        }

        match Journal::open_with_mode(&live, sync_mode_for(&live)) {
            Ok(journal) => {
                let _ = std::fs::remove_file(&snapshot);
                self.read_only = journal.is_read_only();
                self.journal = Some(journal);
                self.reset_undo();
                Ok(())
            }
            Err(error) => {
                // The swap succeeded but the restored file will not open — roll the snapshot back so
                // the user's original journal is not lost, then reopen it.
                if have_snapshot {
                    let _ = restore_journal_file(&live, &snapshot);
                }
                let _ = std::fs::remove_file(&snapshot);
                self.reopen_live(&live);
                Err(format!("{MSG_RESTORE_FAILED} {error}"))
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
                self.journal = None;
            }
        }
    }
}
