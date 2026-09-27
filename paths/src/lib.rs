//! The per-machine locations of SteadyInvest (Story 8.4, arch §Phase 4 A10): the app-config file,
//! the default dossier, the log directory — and a **read-only, tolerant** reader of the dossier
//! pointers stored in the app-config, with the MCP server's per-call dossier resolution.
//!
//! One definition, two users: the desktop app (`steadyinvest-app`) delegates its paths here, and
//! the `steadyinvest-mcp` binary reads them here — so the MCP server never depends on the app
//! crate (A1) and the two can never drift apart.
//!
//! **Never writes.** Nothing in this crate creates, renames or writes a file: an invalid
//! `config.json` is reported, never moved aside (the app does that on its next launch — the MCP
//! server must not act ahead of it).

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The OS project directories of SteadyInvest (`None` when the OS exposes no home directory).
pub fn project_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("", "", "steadyinvest")
}

/// `~/.config/steadyinvest/config.json` (per platform). `None` only when the OS exposes no
/// home / config directory at all.
pub fn config_file_path() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.config_dir().join("config.json"))
}

/// Where a default dossier lives when the owner has none yet: the OS **data** dir (not the config
/// dir, not beside `config.json`) — outside any sync-watched tree.
pub fn default_journal_path() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.data_dir().join("journal.db"))
}

/// The directory of the rotating log files (the app's `steadyinvest.log.*` and the MCP server's
/// `steadyinvest-mcp.log.*` live side by side, each with its own file prefix).
pub fn log_dir() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.data_dir().join("logs"))
}

/// The dossier pointers of the app-config (the only two fields this crate reads). Unknown fields
/// are ignored and missing ones are `None` — the app-config is append-only, so an older or newer
/// file reads the same way.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct DossierPointers {
    /// The dossier the app last **actually opened** (Story 8.4) — the one the owner sees.
    pub last_opened_path: Option<PathBuf>,
    /// The configured dossier (kept on a dossier refused by name, G3 M4).
    pub journal_path: Option<PathBuf>,
}

/// Why the app-config could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointerError {
    /// The file exists but could not be read (permissions, I/O).
    Unreadable { path: PathBuf, detail: String },
    /// The file is not the JSON the app writes.
    Invalid { path: PathBuf, detail: String },
}

impl PointerError {
    /// The app-config file the error is about.
    pub fn path(&self) -> &Path {
        match self {
            PointerError::Unreadable { path, .. } | PointerError::Invalid { path, .. } => path,
        }
    }

    /// The technical detail (for a log line, never shown as is).
    pub fn detail(&self) -> &str {
        match self {
            PointerError::Unreadable { detail, .. } | PointerError::Invalid { detail, .. } => {
                detail
            }
        }
    }
}

impl std::fmt::Display for PointerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PointerError::Unreadable { path, detail } => {
                write!(f, "app-config {} unreadable: {detail}", path.display())
            }
            PointerError::Invalid { path, detail } => {
                write!(f, "app-config {} invalid: {detail}", path.display())
            }
        }
    }
}

impl std::error::Error for PointerError {}

/// Read the dossier pointers of the app-config at `path`: `Ok(None)` when there is no file,
/// `Err` when it cannot be read or is not JSON. Never writes, renames or creates anything.
pub fn read_dossier_pointers(path: &Path) -> Result<Option<DossierPointers>, PointerError> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(PointerError::Unreadable {
                path: path.to_path_buf(),
                detail: e.to_string(),
            });
        }
    };
    serde_json::from_str::<DossierPointers>(&raw)
        .map(Some)
        .map_err(|e| PointerError::Invalid {
            path: path.to_path_buf(),
            detail: e.to_string(),
        })
}

/// Why no dossier path could be determined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// The app-config exists but could not be read — refused rather than guessed.
    ConfigUnreadable(PointerError),
    /// No explicit path, no pointer, and the OS exposes no data directory for a default.
    NoLocation,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::ConfigUnreadable(e) => e.fmt(f),
            ResolveError::NoLocation => f.write_str("no dossier location could be determined"),
        }
    }
}

impl std::error::Error for ResolveError {}

/// The MCP server's dossier for one call (arch A10, owner decisions O3 / D10): `explicit` (the
/// `--dossier` argument) when given; else the app-config's `last_opened_path` (the dossier the
/// owner sees); else its `journal_path`; else the app's default dossier path. An unreadable
/// app-config is refused ([`ResolveError::ConfigUnreadable`]) — never a guess.
pub fn resolve_dossier(explicit: Option<&Path>) -> Result<PathBuf, ResolveError> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    let pointers = match config_file_path() {
        Some(config) => read_dossier_pointers(&config),
        None => Ok(None),
    };
    resolve_dossier_with(None, pointers, default_journal_path())
}

/// [`resolve_dossier`] over given inputs (the pure precedence rule, for tests).
pub fn resolve_dossier_with(
    explicit: Option<&Path>,
    pointers: Result<Option<DossierPointers>, PointerError>,
    default: Option<PathBuf>,
) -> Result<PathBuf, ResolveError> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    let pointers = pointers.map_err(ResolveError::ConfigUnreadable)?;
    if let Some(p) = pointers {
        if let Some(last) = p.last_opened_path {
            return Ok(last);
        }
        if let Some(configured) = p.journal_path {
            return Ok(configured);
        }
    }
    default.ok_or(ResolveError::NoLocation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn precedence_is_explicit_then_last_opened_then_configured_then_default() {
        let both = DossierPointers {
            last_opened_path: Some(p("/seen.db")),
            journal_path: Some(p("/configured.db")),
        };
        assert_eq!(
            resolve_dossier_with(
                Some(Path::new("/explicit.db")),
                Ok(Some(both.clone())),
                Some(p("/default.db"))
            ),
            Ok(p("/explicit.db"))
        );
        assert_eq!(
            resolve_dossier_with(None, Ok(Some(both)), Some(p("/default.db"))),
            Ok(p("/seen.db"))
        );
        let configured_only = DossierPointers {
            last_opened_path: None,
            journal_path: Some(p("/configured.db")),
        };
        assert_eq!(
            resolve_dossier_with(None, Ok(Some(configured_only)), Some(p("/default.db"))),
            Ok(p("/configured.db"))
        );
        assert_eq!(
            resolve_dossier_with(None, Ok(Some(DossierPointers::default())), Some(p("/d.db"))),
            Ok(p("/d.db"))
        );
        assert_eq!(
            resolve_dossier_with(None, Ok(None), Some(p("/d.db"))),
            Ok(p("/d.db"))
        );
        assert_eq!(
            resolve_dossier_with(None, Ok(None), None),
            Err(ResolveError::NoLocation)
        );
    }

    /// AC 5: a configured dossier refused by name keeps `journal_path` on it (G3 M4) while the app
    /// runs on the default stand-in — MCP serves the stand-in, the dossier the owner sees.
    #[test]
    fn a_refused_configured_dossier_serves_the_stand_in_the_owner_sees() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.json");
        std::fs::write(
            &config,
            r#"{"journal_path":"/refused.db","last_opened_path":"/default.db","window_width":900}"#,
        )
        .unwrap();
        let pointers = read_dossier_pointers(&config);
        assert_eq!(
            resolve_dossier_with(None, pointers, Some(p("/other-default.db"))),
            Ok(p("/default.db"))
        );
    }

    #[test]
    fn a_missing_config_is_no_pointer_and_resolves_to_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.json");
        assert_eq!(read_dossier_pointers(&config), Ok(None));
        assert_eq!(
            resolve_dossier_with(None, read_dossier_pointers(&config), Some(p("/d.db"))),
            Ok(p("/d.db"))
        );
        assert!(!config.exists(), "nothing is created");
    }

    #[test]
    fn an_invalid_config_is_refused_and_left_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.json");
        std::fs::write(&config, b"{ not json").unwrap();
        let result = resolve_dossier_with(None, read_dossier_pointers(&config), Some(p("/d.db")));
        match result {
            Err(ResolveError::ConfigUnreadable(PointerError::Invalid { path, .. })) => {
                assert_eq!(path, config)
            }
            other => panic!("expected ConfigUnreadable(Invalid), got {other:?}"),
        }
        assert_eq!(std::fs::read(&config).unwrap(), b"{ not json");
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("config.json")]);
    }

    #[test]
    fn unknown_fields_are_tolerated_and_missing_ones_are_none() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.json");
        std::fs::write(
            &config,
            r#"{"theme":"dark","recent_journals":[],"some_future_field":{"x":1}}"#,
        )
        .unwrap();
        assert_eq!(
            read_dossier_pointers(&config),
            Ok(Some(DossierPointers::default()))
        );
    }

    #[test]
    fn null_pointers_read_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.json");
        std::fs::write(&config, r#"{"journal_path":null,"last_opened_path":null}"#).unwrap();
        assert_eq!(
            read_dossier_pointers(&config),
            Ok(Some(DossierPointers::default()))
        );
    }
}
