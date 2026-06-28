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
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::labels::LabelSet;
use crate::provider::ProviderChoice;
use crate::regime::{Regime, SECTION_COUNT};
use crate::theme::Theme;
use crate::viewmodel::format::NumberFormat;

/// Default window size (physical pixels) on first launch.
const DEFAULT_WINDOW_WIDTH: u32 = 1100;
const DEFAULT_WINDOW_HEIGHT: u32 = 720;
/// The reference currency a fresh install starts with (Story 4.3, FR63). A plain ISO-4217-style
/// 3-letter code; the set is open (the user can pick another in Settings), so this is a `String`,
/// not an enum — no `contract` type, no migration when the picker grows.
pub const DEFAULT_REFERENCE_CURRENCY: &str = "CHF";
/// Sanity bounds for a persisted size — outside means a damaged value, fall back.
const MIN_SANE_WINDOW: u32 = 320;
const MAX_SANE_WINDOW: u32 = 16_384;

/// Per-study UI view-state (Story 2.3, FR56): the active reading regime and the per-section
/// open/closed flags (§1–§5). This is **UI view-state**, so it lives in app-config (ADD7) — NEVER
/// in the journal or `contract::Study` (an immutable, versioned domain snapshot). Container-level
/// `#[serde(default)]` so a partial entry (regime present, folds missing — or vice versa) still
/// loads, falling back to the regime's own preset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StudyViewState {
    pub regime: Regime,
    /// One open/closed flag per section §1–§5 (`true` = open).
    pub folds: [bool; SECTION_COUNT],
}

impl Default for StudyViewState {
    fn default() -> Self {
        // A never-customised view defaults to the entry regime and its all-open preset — the same
        // state a freshly-opened study shows, so a missing/partial entry is indistinguishable from
        // "opened, never folded".
        let regime = Regime::default();
        StudyViewState {
            regime,
            folds: regime.fold_preset(),
        }
    }
}

/// Everything the app remembers across launches. Later stories append fields
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
    /// Path of the last-used journal (Story 2.2, ADD7/NFR-R3). `None` until the first journal is
    /// opened or created; then the app reopens it automatically next launch. Lives here in
    /// app-config — outside the journal itself (never beside `config.json` in the *config* dir
    /// either: a default journal goes in the OS *data* dir, see `state::default_journal_path`).
    /// The location picker + recent-journals list is Story 5-5. Append-only `#[serde(default)]`,
    /// so an older config without the field still loads.
    #[serde(default)]
    pub journal_path: Option<PathBuf>,
    /// Per-study reading-regime + fold view-state (Story 2.3, FR56), keyed by the stringified study
    /// id. Absent key ⇒ the study has never been folded ⇒ [`StudyViewState::default`] (entry regime,
    /// all open). Append-only `#[serde(default)]`, exactly like `journal_path`: a 2.1/2.2-era config
    /// without the field still loads. App-config only — never the journal (ADD7).
    #[serde(default)]
    pub study_view_state: BTreeMap<String, StudyViewState>,
    /// The preferred data provider (Story 3.2, FR63). Records only *which* provider — the API
    /// **key** lives only in the OS secret store (`keychain.rs`), NEVER here (NFR-S1). Append-only
    /// `#[serde(default)]`: a pre-3.2 config without the field loads and defaults to
    /// [`ProviderChoice::Eodhd`] (the only adapter today).
    #[serde(default)]
    pub preferred_provider: ProviderChoice,
    /// The single global reference currency the portfolio is read in (Story 4.3, FR63). A 3-letter
    /// ISO-4217-style code (CHF/EUR/USD/…). The holdings register **labels** amounts with it but
    /// performs **no** FX conversion in Epic 4 (Guy's 2026-06-27 single-currency decision). Lives in
    /// app-config (ADD7 — outside the journal). Append-only `#[serde(default …)]`: a pre-4.3 config
    /// without the field loads and defaults to [`DEFAULT_REFERENCE_CURRENCY`]. Read it through
    /// [`AppConfig::reference_currency_or_default`] so a damaged on-disk value falls back.
    #[serde(default = "default_reference_currency")]
    pub reference_currency: String,
    /// The default trailing-stop **percentage** the set-stop control pre-fills (Story 4.5, FR42/FR63
    /// thresholds). `None` = no default. Lives in app-config (ADD7 — outside the journal). Append-only
    /// `#[serde(default)]` (Option → `None`): a pre-4.5 config loads fine. Read it through
    /// [`AppConfig::default_trailing_stop_pct_or_none`] so a damaged on-disk value is ignored.
    #[serde(default)]
    pub default_trailing_stop_pct: Option<String>,
}

/// Whether `s` is a well-formed trailing-stop percentage: an exact decimal strictly inside `(0, 100)`
/// (Story 4.5). A 0 %/100 %/negative/garbage value is rejected (a 0 % stop is meaningless; a ≥100 %
/// stop would put the level at/below zero).
pub fn is_valid_trailing_stop_pct(s: &str) -> bool {
    rust_decimal::Decimal::from_str_exact(s.trim())
        .ok()
        .is_some_and(|p| p > rust_decimal::Decimal::ZERO && p < rust_decimal::Decimal::ONE_HUNDRED)
}

/// serde default for [`AppConfig::reference_currency`] (a free function — `#[serde(default = …)]`
/// needs a path, and `String`'s own `Default` is `""`, which we never want).
fn default_reference_currency() -> String {
    DEFAULT_REFERENCE_CURRENCY.to_string()
}

/// Whether `code` is a well-formed reference currency: exactly three ASCII uppercase letters
/// (ISO-4217 shape). We validate the *shape*, not membership in the live ISO list — the user may
/// legitimately use a code our hard-coded picker doesn't list.
pub fn is_valid_currency_code(code: &str) -> bool {
    code.len() == 3 && code.bytes().all(|b| b.is_ascii_uppercase())
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
            journal_path: None,
            study_view_state: BTreeMap::new(),
            preferred_provider: ProviderChoice::default(),
            reference_currency: default_reference_currency(),
            default_trailing_stop_pct: None,
        }
    }
}

impl AppConfig {
    /// The reference currency if the persisted value is well-formed, the default otherwise (a
    /// damaged/empty code must not produce a blank or malformed currency label — validate before
    /// trust, the same posture as [`Self::sane_window_size`]).
    pub fn reference_currency_or_default(&self) -> String {
        if is_valid_currency_code(&self.reference_currency) {
            self.reference_currency.clone()
        } else {
            DEFAULT_REFERENCE_CURRENCY.to_string()
        }
    }

    /// The persisted default trailing-stop percentage if well-formed (Story 4.5), else `None` — a
    /// damaged on-disk value must not pre-fill a malformed percent (validate before trust).
    pub fn default_trailing_stop_pct_or_none(&self) -> Option<String> {
        self.default_trailing_stop_pct
            .as_deref()
            .filter(|s| is_valid_trailing_stop_pct(s))
            .map(str::to_string)
    }

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
            journal_path: Some(PathBuf::from("/tmp/steadyinvest/journal.db")),
            study_view_state: BTreeMap::from([(
                "11111111-1111-1111-1111-111111111111".to_string(),
                StudyViewState {
                    regime: Regime::Contemplation,
                    folds: [true, false, false, true, false],
                },
            )]),
            preferred_provider: ProviderChoice::None,
            reference_currency: "EUR".to_string(),
            default_trailing_stop_pct: Some("15".to_string()),
        };
        save(&path, &config).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.config, config);
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn reference_currency_round_trips_and_an_old_config_defaults_it() {
        // Append-only rail (Story 4.3): a chosen currency persists; a pre-4.3 config without the
        // field still loads, defaulting to CHF — no migration, no failure.
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        let config = AppConfig {
            reference_currency: "USD".to_string(),
            ..AppConfig::default()
        };
        save(&path, &config).unwrap();
        assert_eq!(load(&path).config.reference_currency, "USD");

        std::fs::write(
            &path,
            r#"{ "window_width": 1280, "theme": "light", "journal_path": "/x/journal.db" }"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert!(
            loaded.warning.is_none(),
            "an older config is not a fallback"
        );
        assert_eq!(
            loaded.config.reference_currency, DEFAULT_REFERENCE_CURRENCY,
            "the new currency field defaults to CHF"
        );
    }

    #[test]
    fn a_damaged_reference_currency_falls_back_on_read() {
        // The on-disk value is loaded verbatim, but the read accessor validates the shape: a blank
        // or malformed code must not surface as a currency label.
        assert!(is_valid_currency_code("CHF"));
        assert!(!is_valid_currency_code(""));
        assert!(!is_valid_currency_code("chf"), "must be uppercase");
        assert!(!is_valid_currency_code("US"), "must be three letters");
        assert!(!is_valid_currency_code("US1"), "letters only");

        let damaged = AppConfig {
            reference_currency: "??".to_string(),
            ..AppConfig::default()
        };
        assert_eq!(
            damaged.reference_currency_or_default(),
            DEFAULT_REFERENCE_CURRENCY,
            "a malformed persisted code falls back to the default on read"
        );
    }

    #[test]
    fn preferred_provider_round_trips_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        let config = AppConfig {
            preferred_provider: ProviderChoice::None,
            ..AppConfig::default()
        };
        save(&path, &config).unwrap();
        assert_eq!(load(&path).config.preferred_provider, ProviderChoice::None);
    }

    #[test]
    fn old_config_without_preferred_provider_loads_and_defaults_the_field() {
        // The append-only rail (Story 3.2): a pre-3.2 config has no `preferred_provider`. It must
        // still load, defaulting the field to `Eodhd` — no migration, no failure. The API key is
        // NOT in config (NFR-S1), so there is nothing secret to default here.
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        std::fs::write(
            &path,
            r#"{ "window_width": 1280, "theme": "light", "journal_path": "/x/journal.db" }"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert!(
            loaded.warning.is_none(),
            "an older config is not a fallback"
        );
        assert_eq!(
            loaded.config.preferred_provider,
            ProviderChoice::Eodhd,
            "the new provider field defaults to the only adapter"
        );
        assert_eq!(loaded.config.window_width, 1280, "known fields still load");
    }

    #[test]
    fn config_never_serializes_an_api_key_field() {
        // NFR-S1 structural guard: app-config holds the provider *choice*, never a secret. Serialize
        // a config and assert no key-like field name appears — the key lives only in the keychain.
        let json = serde_json::to_string(&AppConfig {
            preferred_provider: ProviderChoice::Eodhd,
            ..AppConfig::default()
        })
        .unwrap();
        // Match field-name tokens precisely (`"api_key"`, `"secret"` as JSON keys) rather than a
        // bare `"key"` substring, which would false-positive on any future field whose name merely
        // contains "key". The real NFR-S1 guarantee is structural — no key field exists — so this is
        // a smoke test against an accidental future addition, not the guarantee itself.
        assert!(!json.contains("\"api_key\""), "no api_key field in config");
        assert!(!json.contains("\"secret\""), "no secret field in config");
        assert!(
            json.contains("preferred_provider"),
            "but the choice is recorded"
        );
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
    fn old_config_without_journal_path_loads_and_defaults_the_field() {
        // The append-only rail (Story 2.2): a 2.1-era config file has no `journal_path`. It must
        // still load, with the new field defaulting to `None` — no migration, no failure.
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        std::fs::write(
            &path,
            r#"{ "window_width": 1280, "window_height": 800, "theme": "light" }"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert!(
            loaded.warning.is_none(),
            "an older config is not a fallback"
        );
        assert_eq!(loaded.config.journal_path, None, "the new field defaults");
        assert_eq!(loaded.config.window_width, 1280, "known fields still load");
    }

    #[test]
    fn old_config_without_study_view_state_loads_and_defaults_the_field() {
        // The append-only rail (Story 2.3): a 2.1/2.2-era config has no `study_view_state`. It must
        // still load, with the new field defaulting to an empty map — no migration, no failure.
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        std::fs::write(
            &path,
            r#"{ "window_width": 1280, "theme": "light", "journal_path": "/x/journal.db" }"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert!(
            loaded.warning.is_none(),
            "an older config is not a fallback"
        );
        assert!(
            loaded.config.study_view_state.is_empty(),
            "the new view-state field defaults empty"
        );
        assert_eq!(loaded.config.window_width, 1280, "known fields still load");
    }

    #[test]
    fn study_view_state_round_trips_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        let mut state = BTreeMap::new();
        state.insert(
            "22222222-2222-2222-2222-222222222222".to_string(),
            StudyViewState {
                regime: Regime::Contemplation,
                folds: [true, false, true, false, true],
            },
        );
        let config = AppConfig {
            study_view_state: state,
            ..AppConfig::default()
        };
        save(&path, &config).unwrap();
        assert_eq!(load(&path).config.study_view_state, config.study_view_state);
    }

    #[test]
    fn partial_view_state_entry_falls_back_to_the_regime_preset() {
        // A view-state entry that carries only the regime (no `folds`) loads, with `folds` defaulting
        // to the regime's preset rather than an all-false (all-collapsed) accident.
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        std::fs::write(
            &path,
            r#"{ "study_view_state": { "abc": { "regime": "entry" } } }"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert!(loaded.warning.is_none());
        assert_eq!(
            loaded.config.study_view_state.get("abc").unwrap().folds,
            Regime::Entry.fold_preset(),
            "a missing folds array defaults to the regime preset, never all-collapsed"
        );
    }

    #[test]
    fn study_view_state_default_is_entry_all_open() {
        let s = StudyViewState::default();
        assert_eq!(s.regime, Regime::Entry);
        assert_eq!(s.folds, [true; SECTION_COUNT]);
    }

    #[test]
    fn fold_and_regime_edits_survive_a_simulated_relaunch() {
        // Mirrors the `main.rs` toggle-fold / set-regime callback path (`entry(id).or_default()` →
        // mutate → persist) and then reloads from the real on-disk file — the headless proof of the
        // AC2/AC6 fold+regime restore. (The in-GUI click-through is human/AT-SPI verified, as 2.1/2.2
        // recorded for the sandbox.)
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        let id = "33333333-3333-3333-3333-333333333333".to_string();

        // Launch 1: a never-opened study defaults to Entry + all-open; the user folds §3, then
        // switches to contemplation (which applies that regime's preset).
        let mut cfg = AppConfig::default();
        cfg.study_view_state.entry(id.clone()).or_default().folds[2] = false;
        save(&path, &cfg).unwrap();

        let mut cfg = load(&path).config;
        {
            let entry = cfg.study_view_state.entry(id.clone()).or_default();
            entry.regime = Regime::Contemplation;
            entry.folds = Regime::Contemplation.fold_preset();
        }
        save(&path, &cfg).unwrap();

        // Launch 2: reopen → the persisted regime + folds come back verbatim.
        let restored = load(&path).config;
        let view = restored.study_view_state.get(&id).expect("entry persisted");
        assert_eq!(view.regime, Regime::Contemplation, "regime restored");
        assert_eq!(
            view.folds,
            Regime::Contemplation.fold_preset(),
            "the contemplation fold preset restored on reopen"
        );
        // An untouched study still defaults (absence ⇒ Entry + all open).
        assert!(!restored.study_view_state.contains_key("never-opened"));
    }

    #[test]
    fn journal_path_round_trips_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        let config = AppConfig {
            journal_path: Some(PathBuf::from(
                "/home/guy/.local/share/steadyinvest/journal.db",
            )),
            ..AppConfig::default()
        };
        save(&path, &config).unwrap();
        assert_eq!(load(&path).config.journal_path, config.journal_path);
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
