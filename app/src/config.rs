//! App-config persistence (ADD7): per-machine preferences in the OS config dir via
//! `directories::ProjectDirs` — NEVER inside (or beside) the journal. The journal is not even
//! opened in Story 2.1.
//!
//! User-data rail (Epic-1 retro lesson 5): the struct is forward-extensible — container-level
//! `#[serde(default)]` so every missing field falls back, unknown fields tolerated (serde's
//! default) — so 2.3 can add fold/regime state without a migration. A missing or corrupt file
//! falls back to defaults and never blocks launch; a corrupt file is moved aside, never
//! destroyed (validate-before-mutate, 1.10 lesson).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::labels::LabelSet;
use crate::theme::Theme;
use crate::viewmodel::format::NumberFormat;

/// Default window size (physical pixels) on first launch.
const DEFAULT_WINDOW_WIDTH: u32 = 1100;
const DEFAULT_WINDOW_HEIGHT: u32 = 720;
/// Sanity bounds for a persisted size — outside means a damaged value, fall back.
const MIN_SANE_WINDOW: u32 = 320;
const MAX_SANE_WINDOW: u32 = 16_384;

/// Everything the app remembers across launches in 2.1. Later stories append fields
/// (fold/regime state in 2.3, provider prefs in 3.2) — append-only, defaults required.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub window_width: u32,
    pub window_height: u32,
    pub maximized: bool,
    pub theme: Theme,
    pub label_set: LabelSet,
    pub number_format: NumberFormat,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            window_width: DEFAULT_WINDOW_WIDTH,
            window_height: DEFAULT_WINDOW_HEIGHT,
            maximized: false,
            theme: Theme::default(),
            label_set: LabelSet::default(),
            number_format: NumberFormat::default(),
        }
    }
}

impl AppConfig {
    /// Persisted window size if it is sane, the default size otherwise (a 0×0 or absurd value
    /// must not produce an unusable window — validate before trust).
    pub fn sane_window_size(&self) -> (u32, u32) {
        let sane = |v: u32| (MIN_SANE_WINDOW..=MAX_SANE_WINDOW).contains(&v);
        if sane(self.window_width) && sane(self.window_height) {
            (self.window_width, self.window_height)
        } else {
            (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT)
        }
    }
}

/// A load result: the config to use plus an optional human-readable warning when the file was
/// unreadable or invalid (the caller surfaces it — a fallback is a visible event, never a
/// silence).
pub struct Loaded {
    pub config: AppConfig,
    pub warning: Option<String>,
}

/// `~/.config/steadyinvest/config.json` (per-platform via `ProjectDirs`). `None` only when the
/// OS exposes no home/config directory at all.
pub fn default_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "steadyinvest")
        .map(|dirs| dirs.config_dir().join("config.json"))
}

/// Load with fallback-to-defaults. Never panics, never blocks launch, never destroys the file:
/// an unparseable file is renamed aside (`config.json.invalid`) so the next save cannot
/// overwrite the evidence.
pub fn load(path: &Path) -> Loaded {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Loaded {
                config: AppConfig::default(),
                warning: None,
            };
        }
        Err(error) => {
            let warning = format!(
                "app-config {} unreadable ({error}); defaults in effect",
                path.display()
            );
            tracing::warn!("{warning}");
            return Loaded {
                config: AppConfig::default(),
                warning: Some(warning),
            };
        }
    };
    match serde_json::from_str(&raw) {
        Ok(config) => Loaded {
            config,
            warning: None,
        },
        Err(error) => {
            let aside = path.with_extension("json.invalid");
            let preserved = std::fs::rename(path, &aside).is_ok();
            let warning = format!(
                "app-config {} invalid ({error}); defaults in effect{}",
                path.display(),
                if preserved {
                    format!("; original kept as {}", aside.display())
                } else {
                    String::from("; original left in place")
                }
            );
            tracing::warn!("{warning}");
            Loaded {
                config: AppConfig::default(),
                warning: Some(warning),
            }
        }
    }
}

/// Atomic-ish save: write a sibling temp file, then rename over the target (write-then-rename
/// is enough here — a torn config is at worst a fallback to defaults, never data loss).
pub fn save(path: &Path, config: &AppConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config).expect("AppConfig serializes");
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config_path(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("config.json")
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        let config = AppConfig {
            window_width: 1440,
            window_height: 900,
            maximized: true,
            theme: Theme::Light,
            label_set: LabelSet::Neutral,
            number_format: NumberFormat::Point,
        };
        save(&path, &config).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.config, config);
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn missing_file_yields_defaults_without_warning() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load(&temp_config_path(&dir));
        assert_eq!(loaded.config, AppConfig::default());
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn unknown_fields_are_tolerated_and_missing_fields_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        // A future version's file: one known field, one field from 2.3, one unknown object.
        std::fs::write(
            &path,
            r#"{ "theme": "light", "fold_state": "contemplation", "future": { "x": 1 } }"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.config.theme, Theme::Light);
        assert_eq!(
            loaded.config.window_width,
            AppConfig::default().window_width
        );
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults_and_is_preserved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        std::fs::write(&path, "{ this is not json").unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.config, AppConfig::default());
        assert!(loaded.warning.is_some(), "fallback must be a visible event");
        let aside = path.with_extension("json.invalid");
        assert_eq!(
            std::fs::read_to_string(&aside).unwrap(),
            "{ this is not json",
            "the corrupt original must be preserved, never destroyed"
        );
        assert!(
            !path.exists(),
            "the bad file was moved aside, not left to be re-read"
        );
    }

    #[test]
    fn save_overwrites_atomically_via_rename() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        save(&path, &AppConfig::default()).unwrap();
        let updated = AppConfig {
            theme: Theme::Light,
            ..AppConfig::default()
        };
        save(&path, &updated).unwrap();
        assert_eq!(load(&path).config, updated);
        assert!(
            !path.with_extension("json.tmp").exists(),
            "temp file cleaned up by rename"
        );
    }

    #[test]
    fn insane_window_sizes_fall_back_to_defaults() {
        let mut config = AppConfig {
            window_width: 0,
            window_height: 50,
            ..AppConfig::default()
        };
        assert_eq!(
            config.sane_window_size(),
            (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT)
        );
        config.window_width = 1280;
        config.window_height = 800;
        assert_eq!(config.sane_window_size(), (1280, 800));
    }
}
