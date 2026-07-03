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

/// The pinned default dividend withholding rate in percent (Story 6.4, FR41 / PRD Appendix A):
/// the Swiss impôt anticipé — 35 %. Configurable in Réglages; per-entry override on the form.
pub const DEFAULT_WITHHOLDING_RATE_PCT: &str = "35";

/// Whether `s` is a well-formed withholding rate: an exact decimal in `[0, 100]` (Story 6.4).
/// The ONE validation shared by the config accessor and the Réglages handler (the
/// `is_valid_trailing_stop_pct` precedent — no hand-copied bounds that can drift).
pub fn is_valid_withholding_rate_pct(s: &str) -> bool {
    rust_decimal::Decimal::from_str_exact(s)
        .is_ok_and(|r| !r.is_sign_negative() && r <= rust_decimal::Decimal::ONE_HUNDRED)
}
/// The pinned default **concentration threshold** in percent (Story 6.7, FR45): the "configured
/// majority share" — 50 %. The Portefeuille concentration block murmurs from 10 points below it
/// (`core::risk::concentration_flagged`). Configurable in Réglages.
pub const DEFAULT_CONCENTRATION_THRESHOLD_PCT: &str = "50";

/// The pinned default diversify-by-size table (Story 6.7 — `change-request_guy.md`): the two
/// sales boundaries ($1 billion / $10 billion, compared in the reference currency, same unit as
/// entered in studies) and the three target portfolio shares (25 / 50 / 25 %). All configurable.
pub const DEFAULT_SIZE_SMALL_MAX: &str = "1000000000";
pub const DEFAULT_SIZE_MEDIUM_MAX: &str = "10000000000";
pub const DEFAULT_SIZE_TARGET_SMALL_PCT: &str = "25";
pub const DEFAULT_SIZE_TARGET_MEDIUM_PCT: &str = "50";
pub const DEFAULT_SIZE_TARGET_LARGE_PCT: &str = "25";

/// Whether `s` is a well-formed sales boundary for the diversify-by-size table (Story 6.7): an
/// exact, strictly positive decimal. A zero/negative/garbage boundary is rejected (the classes
/// would collapse); a nonpositive parseable value must not classify confidently (the 6.6 rule).
pub fn is_valid_size_bound(s: &str) -> bool {
    rust_decimal::Decimal::from_str_exact(s.trim())
        .is_ok_and(|b| b.is_sign_positive() && !b.is_zero())
}

/// Whether `s` is a well-formed size-class target share (Story 6.7): an exact decimal in
/// `(0, 100]`. The three targets need NOT sum to 100 — they are the user's per-class anchors
/// from the change request, not a partition the app enforces.
pub fn is_valid_size_target_pct(s: &str) -> bool {
    rust_decimal::Decimal::from_str_exact(s.trim())
        .is_ok_and(|p| p > rust_decimal::Decimal::ZERO && p <= rust_decimal::Decimal::ONE_HUNDRED)
}
/// Whether `s` is a well-formed fallback-provider wire name (Story 6.9, FR26): parses as a
/// [`crate::provider::ProviderChoice`] and is not `none` (an explicit "no fallback" is the
/// ABSENT field, never a stored sentinel).
pub fn is_valid_fallback_provider(s: &str) -> bool {
    matches!(
        crate::provider::ProviderChoice::parse(s.trim()),
        Some(choice) if choice != crate::provider::ProviderChoice::None
    )
}
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

/// One entry in the recent-journals list (Story 5.5, FR66/ADD7). The **pointer** is `(journal_id,
/// last_seen_version)` — not just a path: the path is where to reopen, the identity + last-seen
/// logical version let the app detect a journal that regressed on disk ("you saw vN, this is vM" —
/// the stale-detection #65 / the 5.4 review wanted). Lives in app-config, NEVER inside the journal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecentJournal {
    pub path: PathBuf,
    /// The journal's identity (stringified UUID) — distinguishes a moved/copied journal from a
    /// different one at a reused path.
    pub journal_id: String,
    /// The monotonic `logical_version` the app last saw for this journal (updated on every clean
    /// open/close). A lower on-disk version on reopen surfaces the neutral stale notice.
    pub last_seen_version: u64,
}

/// How many recent journals to remember (most-recent-first, de-duplicated by path).
const RECENT_JOURNALS_CAP: usize = 8;

/// A de-dupe key for a journal path (Story 5.5): the canonicalized path when it exists (resolves
/// symlinks / `.` / `..` / a NAS alias), else the raw path (a recent entry whose file was moved/deleted
/// still de-dupes by its literal path).
fn canonical_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Everything the app remembers across launches. Later stories append fields
/// (fold/regime state in 2.3, provider prefs in 3.2, recent journals in 5.5) — append-only,
/// defaults required.
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
    /// The recent-journals list (Story 5.5, FR66), most-recent-first. Each entry carries the
    /// `(journal_id, last_seen_version)` pointer for stale detection. Append-only `#[serde(default)]`:
    /// a pre-5.5 config loads with an empty list. The `journal_path` field above stays the last-used
    /// path (back-compat); the front of this list mirrors it.
    #[serde(default)]
    pub recent_journals: Vec<RecentJournal>,
    /// The last-selected **active portfolio** id (Story 6.1, FR37), as a UUID string. Lives in
    /// app-config (ADD7 — outside the journal). Append-only `#[serde(default)]` (Option → `None`): a
    /// pre-6.1 config loads fine and follows the first portfolio. A stale/garbage id is ignored by
    /// `JournalState::active_portfolio` (it validates against the live list).
    #[serde(default)]
    pub active_portfolio_id: Option<String>,
    /// The default dividend **withholding rate** in percent (Story 6.4, FR41 / Appendix A) — the
    /// rate the dividend form's empty « Retenue » auto-computes with. `None` = the pinned default
    /// [`DEFAULT_WITHHOLDING_RATE_PCT`] (35, the CH impôt anticipé). Append-only `#[serde(default)]`;
    /// read through [`AppConfig::withholding_rate_pct_or_default`] (validate before trust).
    #[serde(default)]
    pub withholding_rate_pct: Option<String>,
    /// The **concentration threshold** in percent (Story 6.7, FR45) — the configured majority
    /// share the Portefeuille block warns near. `None` = the pinned default
    /// [`DEFAULT_CONCENTRATION_THRESHOLD_PCT`] (50). Append-only `#[serde(default)]`; read through
    /// [`AppConfig::concentration_threshold_pct_or_default`] (validate before trust).
    #[serde(default)]
    pub concentration_threshold_pct: Option<String>,
    /// The diversify-by-size **sales boundaries** (Story 6.7 — `change-request_guy.md`): the
    /// small/medium and medium/large cut-offs, compared against study sales converted to the
    /// reference currency (same unit as entered in studies). `None` = the pinned $1B / $10B
    /// defaults. Append-only; read through [`AppConfig::size_bounds_or_default`], which also
    /// enforces `small < medium` (a crossed pair falls back whole — half a table is not a table).
    #[serde(default)]
    pub size_small_max: Option<String>,
    #[serde(default)]
    pub size_medium_max: Option<String>,
    /// The per-class **target portfolio shares** in percent (Story 6.7). `None` = the pinned
    /// 25 / 50 / 25 defaults. Targets are the user's anchors — they need not sum to 100 and the
    /// app never rebalances them. Read through [`AppConfig::size_targets_or_default`].
    #[serde(default)]
    pub size_target_small_pct: Option<String>,
    #[serde(default)]
    pub size_target_medium_pct: Option<String>,
    #[serde(default)]
    pub size_target_large_pct: Option<String>,
    /// The per-field-type FALLBACK providers (Story 6.9, FR26): the wire name of the provider a
    /// price / fundamentals / FX fetch fails over to when the PRIMARY (`preferred_provider`,
    /// unchanged) errors. `None` = no fallback (the pre-6.9 behaviour). Append-only
    /// `#[serde(default)]`; read through [`AppConfig::provider_chain`] (validate before trust —
    /// a damaged wire name, the primary itself, or a provider incapable of the field is dropped
    /// from the effective chain, never a broken request).
    #[serde(default)]
    pub price_fallback_provider: Option<String>,
    #[serde(default)]
    pub fundamentals_fallback_provider: Option<String>,
    #[serde(default)]
    pub fx_fallback_provider: Option<String>,
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

/// The fixed allow-list of currencies a **holding** can be denominated in (Story 6.2, FR38). Guy's
/// 2026-07-02 decision: a small fixed set (a guard-rail against typos and codes the price provider
/// can't serve), not a free 3-letter field. Mirrors the reference-currency picker in Settings; extend
/// here when a new currency is needed. The holding currency selector renders exactly this set.
pub const SUPPORTED_CURRENCIES: &[&str] = &["CHF", "EUR", "USD", "GBP", "JPY"];

/// Whether `code` is a currency a holding may be denominated in — i.e. a member of
/// [`SUPPORTED_CURRENCIES`] (Story 6.2). The app validates a chosen holding currency against this
/// before it reaches persistence, so a stored holding currency is always one of the allow-list.
pub fn is_supported_currency(code: &str) -> bool {
    SUPPORTED_CURRENCIES.contains(&code)
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
            recent_journals: Vec::new(),
            active_portfolio_id: None,
            withholding_rate_pct: None,
            concentration_threshold_pct: None,
            size_small_max: None,
            size_medium_max: None,
            size_target_small_pct: None,
            size_target_medium_pct: None,
            size_target_large_pct: None,
            price_fallback_provider: None,
            fundamentals_fallback_provider: None,
            fx_fallback_provider: None,
        }
    }
}

impl AppConfig {
    /// The reference currency if the persisted value is a member of [`SUPPORTED_CURRENCIES`], the
    /// default otherwise (a damaged/empty/off-list code must not produce a blank label — validate
    /// before trust, the same posture as [`Self::sane_window_size`]). Membership, not just shape
    /// (Story 6.2): the reference currency is the coalesce fallback for a pre-6.2 NULL-currency
    /// holding, and a fallback outside the allow-list (a hand-edited config, e.g. `"SEK"`) would
    /// prefill the holding editor with a currency its picker can't render or re-save.
    pub fn reference_currency_or_default(&self) -> String {
        if is_supported_currency(&self.reference_currency) {
            self.reference_currency.clone()
        } else {
            DEFAULT_REFERENCE_CURRENCY.to_string()
        }
    }

    /// Record a journal as the most-recently-used (Story 5.5): move it to the front of
    /// `recent_journals`, de-duplicating by **canonical** path (so the same journal reached via two
    /// path spellings is one entry), refresh its `(journal_id, last_seen_version)` pointer, and cap the
    /// list. Also mirrors `journal_path` (the last-used path, back-compat). Idempotent re-records keep
    /// the list stable apart from the refreshed pointer.
    pub fn record_recent(&mut self, path: &Path, journal_id: &str, last_seen_version: u64) {
        let key = canonical_key(path);
        self.recent_journals
            .retain(|r| canonical_key(&r.path) != key);
        self.recent_journals.insert(
            0,
            RecentJournal {
                path: path.to_path_buf(),
                journal_id: journal_id.to_string(),
                last_seen_version,
            },
        );
        self.recent_journals.truncate(RECENT_JOURNALS_CAP);
        self.journal_path = Some(path.to_path_buf());
    }

    /// The `(journal_id, last_seen_version)` recorded for the journal at `path` (Story 5.5), if any —
    /// for the stale-on-reopen check. The caller compares the **`journal_id`** too (a *different*
    /// journal placed at a reused path must not trigger a spurious "older than you saw" notice).
    /// Matched by canonical path.
    pub fn last_seen_for(&self, path: &Path) -> Option<(&str, u64)> {
        let key = canonical_key(path);
        self.recent_journals
            .iter()
            .find(|r| canonical_key(&r.path) == key)
            .map(|r| (r.journal_id.as_str(), r.last_seen_version))
    }

    /// The persisted default trailing-stop percentage if well-formed (Story 4.5), else `None` — a
    /// damaged on-disk value must not pre-fill a malformed percent (validate before trust).
    pub fn default_trailing_stop_pct_or_none(&self) -> Option<String> {
        self.default_trailing_stop_pct
            .as_deref()
            .filter(|s| is_valid_trailing_stop_pct(s))
            .map(str::to_string)
    }

    /// The dividend withholding rate in percent (Story 6.4): the persisted value when it is an
    /// exact decimal in `[0, 100]`, else [`DEFAULT_WITHHOLDING_RATE_PCT`] (35 — CH impôt
    /// anticipé). A damaged on-disk value must not silently compute a wrong net (validate before
    /// trust); the returned spelling is TRIMMED so consumers can parse it verbatim (2026-07-02
    /// review: validating one string and returning another defeated the accessor's purpose).
    pub fn withholding_rate_pct_or_default(&self) -> String {
        self.withholding_rate_pct
            .as_deref()
            .map(str::trim)
            .filter(|s| is_valid_withholding_rate_pct(s))
            .map(str::to_string)
            .unwrap_or_else(|| DEFAULT_WITHHOLDING_RATE_PCT.to_string())
    }

    /// The concentration threshold in percent (Story 6.7, FR45): the persisted value when it is
    /// an exact decimal strictly inside `(0, 100)` (the trailing-stop shape — a 0 %/100 %
    /// majority bar is meaningless), else [`DEFAULT_CONCENTRATION_THRESHOLD_PCT`] (50). Trimmed,
    /// parseable verbatim (the 6.4 accessor lesson).
    pub fn concentration_threshold_pct_or_default(&self) -> String {
        self.concentration_threshold_pct
            .as_deref()
            .map(str::trim)
            .filter(|s| is_valid_trailing_stop_pct(s))
            .map(str::to_string)
            .unwrap_or_else(|| DEFAULT_CONCENTRATION_THRESHOLD_PCT.to_string())
    }

    /// The diversify-by-size sales boundaries `(small_max, medium_max)` (Story 6.7): each bound
    /// resolves **independently** to its persisted value when valid, else its pinned default —
    /// the exact `""` = default semantics the Réglages commit uses (2026-07-03 review, HIGH: a
    /// half-specified table — one bound entered, the other left unset — must NOT discard the
    /// entered bound). The EFFECTIVE pair must then be ordered; a crossed/equal effective pair
    /// falls back whole (never one real and one default boundary out of order). Trimmed,
    /// parseable verbatim.
    pub fn size_bounds_or_default(&self) -> (String, String) {
        let one = |v: &Option<String>, default: &str| {
            v.as_deref()
                .map(str::trim)
                .filter(|s| is_valid_size_bound(s))
                .map(str::to_string)
                .unwrap_or_else(|| default.to_string())
        };
        let small = one(&self.size_small_max, DEFAULT_SIZE_SMALL_MAX);
        let medium = one(&self.size_medium_max, DEFAULT_SIZE_MEDIUM_MAX);
        match (
            rust_decimal::Decimal::from_str_exact(&small),
            rust_decimal::Decimal::from_str_exact(&medium),
        ) {
            (Ok(sd), Ok(md)) if sd < md => (small, medium),
            _ => (
                DEFAULT_SIZE_SMALL_MAX.to_string(),
                DEFAULT_SIZE_MEDIUM_MAX.to_string(),
            ),
        }
    }

    /// The per-class target shares `(small, medium, large)` in percent (Story 6.7). Each target
    /// validates independently (`(0, 100]`) and falls back alone — a damaged target must not
    /// discard the two healthy ones (they are independent anchors, not a partition).
    pub fn size_targets_or_default(&self) -> (String, String, String) {
        let one = |v: &Option<String>, default: &str| {
            v.as_deref()
                .map(str::trim)
                .filter(|s| is_valid_size_target_pct(s))
                .map(str::to_string)
                .unwrap_or_else(|| default.to_string())
        };
        (
            one(&self.size_target_small_pct, DEFAULT_SIZE_TARGET_SMALL_PCT),
            one(&self.size_target_medium_pct, DEFAULT_SIZE_TARGET_MEDIUM_PCT),
            one(&self.size_target_large_pct, DEFAULT_SIZE_TARGET_LARGE_PCT),
        )
    }

    /// The stored fallback wire name for `field` (Story 6.9) — raw, for the Réglages mirror.
    fn fallback_field(&self, field: steadyinvest_ingestion::FieldKind) -> &Option<String> {
        use steadyinvest_ingestion::FieldKind;
        match field {
            FieldKind::Price => &self.price_fallback_provider,
            FieldKind::Fundamentals => &self.fundamentals_fallback_provider,
            FieldKind::Fx => &self.fx_fallback_provider,
        }
    }

    /// The stored fallback for `field` when valid, capable of the field AND different from the
    /// current primary, else `None` — the Réglages mirror matches the EFFECTIVE chain (2026-07-03
    /// review: a chip must never show an active fallback the chain dedups away; a fundamentals
    /// fallback of `twelvedata` in a hand-edited config is dropped, the declared-capability rule).
    pub fn fallback_provider_or_none(
        &self,
        field: steadyinvest_ingestion::FieldKind,
    ) -> Option<crate::provider::ProviderChoice> {
        self.fallback_field(field)
            .as_deref()
            .map(str::trim)
            .and_then(crate::provider::ProviderChoice::parse)
            .filter(|c| *c != crate::provider::ProviderChoice::None)
            .filter(|c| *c != self.preferred_provider)
            .filter(|c| steadyinvest_ingestion::supports(c.wire(), field))
    }

    /// The EFFECTIVE provider chain for `field` (Story 6.9, FR26): the primary
    /// (`preferred_provider` — unchanged 7.4 semantics, it always leads its chain) followed by
    /// the per-field fallback when set, valid, capable of the field, and different from the
    /// primary. Empty when the primary is `None` (the existing no-provider guards fire before
    /// any enqueue). The `Vec` shape generalizes beyond two providers later.
    pub fn provider_chain(
        &self,
        field: steadyinvest_ingestion::FieldKind,
    ) -> Vec<crate::provider::ProviderChoice> {
        // No primary → an empty chain: "no provider configured" is a deliberate state the
        // existing MSG_PROVIDER_NONE guards own; a fallback never resurrects it silently.
        if self.preferred_provider == crate::provider::ProviderChoice::None {
            return Vec::new();
        }
        let fallback = self.fallback_provider_or_none(field);
        // An INCAPABLE primary with a capable configured fallback is SUBSTITUTED (2026-07-03
        // review, HIGH: twelvedata's fundamentals succeed EMPTY by design — it would never
        // error-trigger the failover the user configured the fallback for). Without a capable
        // fallback the primary passes through unchanged (the 7.4 partial-fundamentals
        // behaviour); the substitution is a visible event — the fallback notice compares the
        // effective member against the CONFIGURED primary, not chain position.
        if !steadyinvest_ingestion::supports(self.preferred_provider.wire(), field) {
            if let Some(fallback) = fallback {
                return vec![fallback];
            }
            return vec![self.preferred_provider];
        }
        let mut chain = vec![self.preferred_provider];
        if let Some(fallback) = fallback {
            if !chain.contains(&fallback) {
                chain.push(fallback);
            }
        }
        chain
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
            withholding_rate_pct: Some("15".to_string()),
            recent_journals: vec![RecentJournal {
                path: PathBuf::from("/tmp/steadyinvest/journal.db"),
                journal_id: "11111111-1111-1111-1111-111111111111".to_string(),
                last_seen_version: 12,
            }],
            active_portfolio_id: Some("22222222-2222-2222-2222-222222222222".to_string()),
            concentration_threshold_pct: Some("40".to_string()),
            price_fallback_provider: Some("twelvedata".to_string()),
            fundamentals_fallback_provider: Some("eodhd".to_string()),
            fx_fallback_provider: Some("twelvedata".to_string()),
            size_small_max: Some("500000000".to_string()),
            size_medium_max: Some("5000000000".to_string()),
            size_target_small_pct: Some("30".to_string()),
            size_target_medium_pct: Some("40".to_string()),
            size_target_large_pct: Some("30".to_string()),
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
    fn record_recent_moves_to_front_dedupes_caps_and_mirrors_journal_path() {
        let mut c = AppConfig::default();
        // Record three distinct journals.
        c.record_recent(Path::new("/a/journal.db"), "id-a", 3);
        c.record_recent(Path::new("/b/journal.db"), "id-b", 7);
        c.record_recent(Path::new("/c/journal.db"), "id-c", 1);
        assert_eq!(c.recent_journals.len(), 3);
        assert_eq!(c.recent_journals[0].path, PathBuf::from("/c/journal.db"));
        assert_eq!(c.journal_path, Some(PathBuf::from("/c/journal.db")));

        // Re-recording an existing one moves it to the front + refreshes its pointer, no duplicate.
        c.record_recent(Path::new("/a/journal.db"), "id-a", 9);
        assert_eq!(c.recent_journals.len(), 3, "no duplicate entry");
        assert_eq!(c.recent_journals[0].path, PathBuf::from("/a/journal.db"));
        assert_eq!(
            c.last_seen_for(Path::new("/a/journal.db")),
            Some(("id-a", 9))
        );
        assert_eq!(c.last_seen_for(Path::new("/unknown.db")), None);

        // Cap at RECENT_JOURNALS_CAP, most-recent-first.
        for i in 0..20 {
            c.record_recent(Path::new(&format!("/x{i}/journal.db")), "id-x", i);
        }
        assert_eq!(c.recent_journals.len(), RECENT_JOURNALS_CAP);
        assert_eq!(
            c.recent_journals[0].path,
            PathBuf::from("/x19/journal.db"),
            "most recent first"
        );
    }

    #[test]
    fn last_seen_for_returns_the_journal_id_so_a_reused_path_can_be_distinguished() {
        // Review patch (E4/F3): the stale check must compare journal_id, not just path+version.
        let mut c = AppConfig::default();
        c.record_recent(Path::new("/a/journal.db"), "id-old", 12);
        // A different journal placed at the same path carries a different id; the caller compares it.
        assert_eq!(
            c.last_seen_for(Path::new("/a/journal.db")),
            Some(("id-old", 12))
        );
        let (jid, _) = c.last_seen_for(Path::new("/a/journal.db")).unwrap();
        assert_ne!(jid, "id-new", "a different journal_id is distinguishable");
    }

    #[test]
    fn old_config_without_recent_journals_loads_with_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        // A pre-5.5 config file: no `recent_journals` field.
        std::fs::write(
            &path,
            r#"{ "window_width": 1280, "theme": "light", "journal_path": "/x/journal.db" }"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert!(
            loaded.config.recent_journals.is_empty(),
            "the new field defaults to an empty list"
        );
        assert_eq!(
            loaded.config.journal_path,
            Some(PathBuf::from("/x/journal.db"))
        );
    }

    #[test]
    fn recent_journals_round_trip_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        let mut config = AppConfig::default();
        config.record_recent(Path::new("/a/journal.db"), "id-a", 5);
        save(&path, &config).unwrap();
        assert_eq!(load(&path).config.recent_journals, config.recent_journals);
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

    // ── Story 6.7 (FR45) — concentration threshold + diversify-by-size table ──

    #[test]
    fn concentration_threshold_validates_and_falls_back_on_damage() {
        let mut c = AppConfig::default();
        assert_eq!(
            c.concentration_threshold_pct_or_default(),
            DEFAULT_CONCENTRATION_THRESHOLD_PCT,
            "unset → the pinned 50"
        );
        c.concentration_threshold_pct = Some(" 40 ".to_string());
        assert_eq!(c.concentration_threshold_pct_or_default(), "40", "trimmed");
        for damaged in ["0", "100", "-5", "abc", ""] {
            c.concentration_threshold_pct = Some(damaged.to_string());
            assert_eq!(
                c.concentration_threshold_pct_or_default(),
                DEFAULT_CONCENTRATION_THRESHOLD_PCT,
                "damaged {damaged:?} falls back"
            );
        }
    }

    #[test]
    fn size_bounds_validate_as_a_pair_and_fall_back_whole() {
        let mut c = AppConfig::default();
        assert_eq!(
            c.size_bounds_or_default(),
            (
                DEFAULT_SIZE_SMALL_MAX.to_string(),
                DEFAULT_SIZE_MEDIUM_MAX.to_string()
            )
        );
        c.size_small_max = Some("1000".to_string());
        c.size_medium_max = Some("10000".to_string());
        assert_eq!(
            c.size_bounds_or_default(),
            ("1000".to_string(), "10000".to_string()),
            "a valid ordered pair is used verbatim (unit-agnostic — Guy may enter millions)"
        );
        // A CROSSED pair falls back WHOLE — never one real + one default boundary.
        c.size_small_max = Some("10000".to_string());
        c.size_medium_max = Some("1000".to_string());
        assert_eq!(
            c.size_bounds_or_default(),
            (
                DEFAULT_SIZE_SMALL_MAX.to_string(),
                DEFAULT_SIZE_MEDIUM_MAX.to_string()
            ),
            "crossed boundaries fall back whole"
        );
        // An EQUAL pair is also crossed (the Medium band would be a single point below Small).
        c.size_small_max = Some("1000".to_string());
        c.size_medium_max = Some("1000".to_string());
        assert_eq!(
            c.size_bounds_or_default().0,
            DEFAULT_SIZE_SMALL_MAX,
            "equal boundaries fall back whole"
        );
        // A nonpositive/garbage bound resolves to ITS default; the effective pair then orders
        // (-1 → 1e9 default, 10000 stays) — crossed effectively, so the pair falls back whole.
        c.size_small_max = Some("-1".to_string());
        c.size_medium_max = Some("10000".to_string());
        assert_eq!(
            c.size_bounds_or_default(),
            (
                DEFAULT_SIZE_SMALL_MAX.to_string(),
                DEFAULT_SIZE_MEDIUM_MAX.to_string()
            ),
            "damaged small → default 1e9 crosses the entered 10000 → whole fallback"
        );
        assert!(!is_valid_size_bound("0"));
        assert!(!is_valid_size_bound("abc"));
        assert!(is_valid_size_bound(" 1000000000 "));
    }

    #[test]
    fn a_half_specified_bounds_pair_keeps_the_entered_bound() {
        // 2026-07-03 review (HIGH): one bound entered, the other left at default ("" → None) —
        // the entered bound must be SERVED, not silently discarded for the whole default pair
        // (the Réglages commit validated and reported success on exactly this shape).
        let c = AppConfig {
            size_medium_max: Some("5000000000".to_string()),
            ..AppConfig::default()
        };
        assert_eq!(
            c.size_bounds_or_default(),
            (DEFAULT_SIZE_SMALL_MAX.to_string(), "5000000000".to_string()),
            "(unset, entered) serves default small + the entered medium"
        );
        let c = AppConfig {
            size_small_max: Some("500000000".to_string()),
            ..AppConfig::default()
        };
        assert_eq!(
            c.size_bounds_or_default(),
            ("500000000".to_string(), DEFAULT_SIZE_MEDIUM_MAX.to_string()),
            "(entered, unset) serves the entered small + default medium"
        );
        // An entered bound that crosses the OTHER side's default still falls back whole (the
        // commit refuses this shape; a hand-edited config must not classify against it).
        let c = AppConfig {
            size_small_max: Some("20000000000".to_string()),
            ..AppConfig::default()
        };
        assert_eq!(
            c.size_bounds_or_default(),
            (
                DEFAULT_SIZE_SMALL_MAX.to_string(),
                DEFAULT_SIZE_MEDIUM_MAX.to_string()
            ),
            "small above the default medium is a crossed effective pair"
        );
    }

    #[test]
    fn size_targets_validate_independently() {
        let mut c = AppConfig::default();
        assert_eq!(
            c.size_targets_or_default(),
            ("25".to_string(), "50".to_string(), "25".to_string())
        );
        c.size_target_small_pct = Some("30".to_string());
        c.size_target_medium_pct = Some("garbage".to_string());
        c.size_target_large_pct = Some("100".to_string());
        assert_eq!(
            c.size_targets_or_default(),
            (
                "30".to_string(),
                DEFAULT_SIZE_TARGET_MEDIUM_PCT.to_string(),
                "100".to_string()
            ),
            "a damaged target falls back ALONE (independent anchors, not a partition)"
        );
        assert!(!is_valid_size_target_pct("0"), "a 0 % target is rejected");
        assert!(!is_valid_size_target_pct("101"));
        assert!(is_valid_size_target_pct("100"), "100 is a valid anchor");
        // The three targets need NOT sum to 100 — no cross-validation exists to test.
    }

    #[test]
    fn old_config_without_the_size_table_loads_and_defaults_every_field() {
        // The append-only rail (Story 6.7): a pre-6.7 config has none of the six fields. It must
        // still load — no migration, no failure — and every accessor serves the pinned defaults.
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        std::fs::write(
            &path,
            r#"{ "window_width": 1280, "theme": "light", "journal_path": "/x/journal.db" }"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert!(loaded.warning.is_none());
        assert_eq!(loaded.config.concentration_threshold_pct, None);
        assert_eq!(
            loaded.config.concentration_threshold_pct_or_default(),
            DEFAULT_CONCENTRATION_THRESHOLD_PCT
        );
        assert_eq!(
            loaded.config.size_bounds_or_default(),
            (
                DEFAULT_SIZE_SMALL_MAX.to_string(),
                DEFAULT_SIZE_MEDIUM_MAX.to_string()
            )
        );
    }

    // ── Story 6.9 (FR26) — per-field-type fallback chain ──

    #[test]
    fn provider_chain_is_primary_plus_a_valid_capable_distinct_fallback() {
        use crate::provider::ProviderChoice;
        use steadyinvest_ingestion::FieldKind;
        let mut c = AppConfig::default(); // primary = Eodhd
        assert_eq!(
            c.provider_chain(FieldKind::Price),
            vec![ProviderChoice::Eodhd],
            "no fallback set → the pre-6.9 single-provider chain"
        );
        c.price_fallback_provider = Some("twelvedata".to_string());
        assert_eq!(
            c.provider_chain(FieldKind::Price),
            vec![ProviderChoice::Eodhd, ProviderChoice::TwelveData]
        );
        // The primary as its own fallback dedups (accepted at commit, neutralized here).
        c.price_fallback_provider = Some("eodhd".to_string());
        assert_eq!(
            c.provider_chain(FieldKind::Price),
            vec![ProviderChoice::Eodhd]
        );
        // A damaged wire name is dropped, never a broken request.
        c.price_fallback_provider = Some("yahoo".to_string());
        assert_eq!(
            c.provider_chain(FieldKind::Price),
            vec![ProviderChoice::Eodhd]
        );
        // "none" is not a storable fallback (the absent field is the "no fallback" state).
        c.price_fallback_provider = Some("none".to_string());
        assert_eq!(
            c.provider_chain(FieldKind::Price),
            vec![ProviderChoice::Eodhd]
        );
        assert!(!is_valid_fallback_provider("none"));
        assert!(is_valid_fallback_provider(" twelvedata "));
    }

    #[test]
    fn an_incapable_fallback_is_dropped_for_that_field_only() {
        use crate::provider::ProviderChoice;
        use steadyinvest_ingestion::FieldKind;
        // Twelve Data serves NO fundamentals (7.4 by design) — a hand-edited config naming it as
        // the fundamentals fallback is dropped; the SAME wire name serves price/fx fine.
        let c = AppConfig {
            fundamentals_fallback_provider: Some("twelvedata".to_string()),
            price_fallback_provider: Some("twelvedata".to_string()),
            fx_fallback_provider: Some("twelvedata".to_string()),
            ..AppConfig::default()
        };
        assert_eq!(
            c.provider_chain(FieldKind::Fundamentals),
            vec![ProviderChoice::Eodhd],
            "incapable fallback dropped for fundamentals"
        );
        assert_eq!(
            c.provider_chain(FieldKind::Price),
            vec![ProviderChoice::Eodhd, ProviderChoice::TwelveData]
        );
        assert_eq!(
            c.provider_chain(FieldKind::Fx),
            vec![ProviderChoice::Eodhd, ProviderChoice::TwelveData]
        );
        // 2026-07-03 review (HIGH): an INCAPABLE primary is SUBSTITUTED by a capable fallback
        // for that field (twelvedata fundamentals succeed empty — error failover never fires),
        // and passes through unchanged when no capable fallback exists (7.4 semantics).
        let c = AppConfig {
            preferred_provider: ProviderChoice::TwelveData,
            fundamentals_fallback_provider: Some("eodhd".to_string()),
            price_fallback_provider: Some("eodhd".to_string()),
            ..AppConfig::default()
        };
        assert_eq!(
            c.provider_chain(FieldKind::Fundamentals),
            vec![ProviderChoice::Eodhd],
            "capable fallback substitutes the incapable primary"
        );
        assert_eq!(
            c.provider_chain(FieldKind::Price),
            vec![ProviderChoice::TwelveData, ProviderChoice::Eodhd],
            "the same primary leads the fields it CAN serve"
        );
        let c = AppConfig {
            preferred_provider: ProviderChoice::TwelveData,
            ..AppConfig::default()
        };
        assert_eq!(
            c.provider_chain(FieldKind::Fundamentals),
            vec![ProviderChoice::TwelveData],
            "no capable fallback → unchanged 7.4 partial-fundamentals behaviour"
        );
    }

    #[test]
    fn a_none_primary_yields_an_empty_chain_and_old_configs_default_the_fields() {
        use crate::provider::ProviderChoice;
        use steadyinvest_ingestion::FieldKind;
        let c = AppConfig {
            preferred_provider: ProviderChoice::None,
            price_fallback_provider: Some("eodhd".to_string()),
            ..AppConfig::default()
        };
        assert!(
            c.provider_chain(FieldKind::Price).is_empty(),
            "no primary → the existing no-provider guard fires before any enqueue"
        );
        // Append-only rail: a pre-6.9 config has none of the three fields.
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        std::fs::write(
            &path,
            r#"{ "window_width": 1280, "theme": "light", "journal_path": "/x/journal.db" }"#,
        )
        .unwrap();
        let loaded = load(&path);
        assert!(loaded.warning.is_none());
        assert_eq!(loaded.config.price_fallback_provider, None);
        assert_eq!(
            loaded.config.provider_chain(FieldKind::Fundamentals),
            vec![ProviderChoice::Eodhd],
            "pre-6.9 behaviour unchanged"
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
