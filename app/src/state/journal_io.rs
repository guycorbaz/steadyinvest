//! Journal file I/O (Stories 5.4/5.5 — FR61/FR66): open/create/switch journals with the
//! sync-folder-appropriate [`JournalMode`] (ADD8 — the sync-safe `Delete` in a detected sync
//! folder, so no `-wal` sidecar can corrupt under file-level sync), stale single-instance-lock
//! reclaim, raw-`.db` backups in a `backups/` folder beside the journal, and the path helpers
//! shared with the restore flow. A failed open/create restores the **previous** journal — the app
//! is never left journal-less.

use std::path::{Path, PathBuf};

use steadyinvest_persistence::{
    clear_lock, lock_is_stale, Error as PersistError, Journal, JournalMode,
};
use uuid::Uuid;

use super::{
    JournalState, OpenOutcome, MSG_JOURNAL_LOCKED, MSG_JOURNAL_OPEN_FAILED, MSG_NO_DATA_DIR,
    MSG_NO_JOURNAL, MSG_SAVE_FAILED,
};

/// Whether `path` (a journal file or its directory) lives in a **detected sync folder** (Story 5.5,
/// ADD8) — a path component matches a known consumer-sync provider, case-insensitively. A heuristic,
/// not an exhaustive list; a false negative just means the default WAL mode (no worse than today).
pub fn is_sync_folder(path: &Path) -> bool {
    const SYNC_MARKERS: &[&str] = &[
        "synologydrive",
        "synology drive",
        "cloudstation",
        "dropbox",
        "onedrive",
        "icloud",
        "mobile documents", // macOS iCloud Drive
        "google drive",
        "googledrive",
        "nextcloud",
        "owncloud",
    ];
    // Canonicalize first (resolve a symlink / mount like `~/sync → ~/Dropbox`, the form most likely in
    // practice) — scanning only the literal path would miss it. Falls back to the best-resolving
    // ancestor (the file itself may not exist yet on a create), then the literal path.
    let resolved = std::fs::canonicalize(path)
        .or_else(|_| {
            path.parent()
                .map_or_else(|| Err(()), |p| std::fs::canonicalize(p).map_err(|_| ()))
        })
        .unwrap_or_else(|_| path.to_path_buf());
    resolved.components().any(|c| {
        let name = c.as_os_str().to_string_lossy().to_lowercase();
        SYNC_MARKERS.iter().any(|m| name.contains(m))
    })
}

/// The [`JournalMode`] to open a journal at `path` with (Story 5.5): the sync-safe `Delete` in a
/// detected sync folder (no `-wal` to corrupt under file-level sync), else the default `Wal`.
pub(crate) fn sync_mode_for(path: &Path) -> JournalMode {
    if is_sync_folder(path) {
        JournalMode::Delete
    } else {
        JournalMode::Wal
    }
}

impl JournalState {
    /// Create a raw `.db` backup of the live journal (Story 5.4, FR61) — checkpoint the WAL so the copy
    /// is self-contained, then copy the file to a `backups/` folder **beside the journal** (Story 5.5 —
    /// so backups follow a user-selected location; falls back to the OS data dir if the journal has no
    /// parent). Returns the written path (the caller surfaces it). Guarded: no journal → a neutral notice.
    pub fn create_backup(&self) -> Result<PathBuf, String> {
        let journal = self.journal.as_ref().ok_or(MSG_NO_JOURNAL.to_string())?;
        let live = self.path.as_ref().ok_or(MSG_NO_JOURNAL.to_string())?;
        journal
            .checkpoint()
            .map_err(|error| format!("{MSG_SAVE_FAILED} {error}"))?;
        let version = journal
            .logical_version()
            .map_err(|error| format!("{MSG_SAVE_FAILED} {error}"))?;
        // Story 5.5: backups live beside the journal (a `backups/` sibling of the `.db`), so a
        // user-selected location keeps its backups together. Fall back to the OS data dir only if the
        // journal path has no parent (degenerate).
        let dir = match live.parent() {
            // A real parent directory (an absolute journal path) → backups sit beside the journal.
            Some(parent) if !parent.as_os_str().is_empty() => parent.join("backups"),
            // A bare/relative path with no real parent → the OS data dir (never the process CWD).
            _ => directories::ProjectDirs::from("", "", "steadyinvest")
                .map(|d| d.data_dir().join("backups"))
                .ok_or(MSG_NO_DATA_DIR.to_string())?,
        };
        std::fs::create_dir_all(&dir).map_err(|error| format!("{MSG_SAVE_FAILED} {error}"))?;
        // Key the filename on (id, version, timestamp) so two backups never silently overwrite each
        // other — a same-version backup (e.g. one taken right after a restore) keeps its own file. The
        // timestamp is filesystem-safe (no `:`).
        let stamp = self.clock.now().0.replace(':', "");
        let dest = dir.join(format!("journal-{}-v{version}-{stamp}.db", journal.id()));
        std::fs::copy(live, &dest).map_err(|error| format!("{MSG_SAVE_FAILED} {error}"))?;
        Ok(dest)
    }

    /// Close the current journal cleanly (Story 5.5): checkpoint its WAL, then drop the handle — which
    /// releases its single-instance lock. The `path` is left as-is (the caller sets the new one).
    fn close_current(&mut self) {
        if let Some(journal) = self.journal.as_ref() {
            let _ = journal.checkpoint();
        }
        self.journal = None;
    }

    /// Open an already-opened journal at `path` into `self` with the given mode, replacing the current
    /// journal (Story 5.5). Records identity/version, resets undo. Maps the lock/open failures to
    /// neutral notices. The caller is responsible for having closed/saved the previous journal.
    fn adopt_open(&mut self, path: &Path, mode: JournalMode) -> Result<OpenOutcome, String> {
        match Journal::open_with_mode(path, mode) {
            Ok(journal) => {
                let logical_version = journal
                    .logical_version()
                    .map_err(|error| format!("{MSG_JOURNAL_OPEN_FAILED} {error}"))?;
                let outcome = OpenOutcome {
                    journal_id: journal.id(),
                    logical_version,
                    sync_warning: matches!(mode, JournalMode::Delete),
                };
                self.read_only = journal.is_read_only();
                self.journal = Some(journal);
                self.path = Some(path.to_path_buf());
                self.reset_undo();
                self.pending_restore = None;
                Ok(outcome)
            }
            Err(PersistError::LockHeld { .. }) => Err(MSG_JOURNAL_LOCKED.to_string()),
            Err(error) => Err(format!("{MSG_JOURNAL_OPEN_FAILED} {error}")),
        }
    }

    /// Re-acquire the previous journal after a failed open/create, so the app is never journal-less
    /// (Story 5.5) — best-effort (mirrors the Story 5.4 `reopen_live` discipline).
    fn restore_previous(&mut self, prev: Option<PathBuf>) {
        if let Some(prev) = prev {
            let mode = sync_mode_for(&prev);
            let _ = self.adopt_open(&prev, mode);
        }
    }

    /// Open a journal at `path`, switching away from the current one (Story 5.5, AC1). Closes the
    /// current journal cleanly first (checkpoint + release its lock), opens the target with the
    /// sync-folder-appropriate [`JournalMode`], and returns an [`OpenOutcome`] (identity, version,
    /// whether a sync-folder warning applies). A failed open leaves the **previous** journal open
    /// (never journal-less). The caller records the recent entry + persists app-config + re-renders.
    pub fn open_journal(&mut self, path: &Path) -> Result<OpenOutcome, String> {
        // Re-selecting the journal that is already open is a no-op — closing + reopening it would
        // pointlessly wipe the undo history. Return the current identity without touching anything.
        if let Some(current) = self.path.clone() {
            if self.journal.is_some() && same_file_path(&current, path) {
                return Ok(OpenOutcome {
                    journal_id: self.journal_id().unwrap_or_else(Uuid::nil),
                    logical_version: self.logical_version_or_zero(),
                    sync_warning: matches!(sync_mode_for(path), JournalMode::Delete),
                });
            }
        }
        let prev = self.path.clone();
        self.close_current();
        match self.adopt_open(path, sync_mode_for(path)) {
            Ok(outcome) => Ok(outcome),
            Err(notice) => {
                self.restore_previous(prev);
                Err(notice)
            }
        }
    }

    /// Create a new journal at `dir/<name>.db` and switch to it (Story 5.5, AC1). Closes the current
    /// journal cleanly first; mints the identity + creation time from the injected sources (ADD15);
    /// uses the sync-folder-appropriate mode. A failed create leaves the previous journal open.
    pub fn create_journal(&mut self, dir: &Path, name: &str) -> Result<OpenOutcome, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(MSG_JOURNAL_OPEN_FAILED.to_string());
        }
        let file_name = if trimmed.ends_with(".db") {
            trimmed.to_string()
        } else {
            format!("{trimmed}.db")
        };
        let path = dir.join(file_name);
        let mode = sync_mode_for(dir);
        let prev = self.path.clone();
        self.close_current();
        let id = self.idgen.new_id();
        let created_at = self.clock.now();
        match Journal::create_with_mode(&path, id, &created_at, mode) {
            Ok(journal) => {
                let logical_version = journal.logical_version().unwrap_or(0);
                let outcome = OpenOutcome {
                    journal_id: journal.id(),
                    logical_version,
                    sync_warning: matches!(mode, JournalMode::Delete),
                };
                self.read_only = journal.is_read_only();
                self.journal = Some(journal);
                self.path = Some(path);
                self.reset_undo();
                self.pending_restore = None;
                Ok(outcome)
            }
            Err(PersistError::JournalExists(_)) => {
                self.restore_previous(prev);
                Err(MSG_JOURNAL_OPEN_FAILED.to_string())
            }
            Err(PersistError::LockHeld { .. }) => {
                self.restore_previous(prev);
                Err(MSG_JOURNAL_LOCKED.to_string())
            }
            Err(error) => {
                self.restore_previous(prev);
                Err(format!("{MSG_JOURNAL_OPEN_FAILED} {error}"))
            }
        }
    }

    /// Reclaim a **stale** single-instance lock at `path` (a lock left by a crashed run) and open the
    /// journal (Story 5.5, AC3). Only clears the lock when it is actually stale — never steals a live
    /// instance's lock.
    pub fn reclaim_and_open(&mut self, path: &Path) -> Result<OpenOutcome, String> {
        if lock_is_stale(path) {
            let _ = clear_lock(path);
        }
        self.open_journal(path)
    }
}

/// Whether two paths point at the **same file** (Story 5.4) — canonicalized to resolve symlinks /
/// relative components / a NAS path that aliases the live journal; falls back to a raw comparison when
/// a path cannot be canonicalized (e.g. it does not exist).
pub(crate) fn same_file_path(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}

/// Append a suffix to a path's file name (e.g. `journal.db` → `journal.db-prerestore`) — used for the
/// pre-restore snapshot sibling file.
pub(crate) fn path_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}
