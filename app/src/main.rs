//! steadyinvest-app — thin Slint desktop UI (binary).
//!
//! Story 2.1 shell: persistent nav rail + top bar + pinned footer disclaimer (FR64), token
//! design system with runtime dark/light switch (`theme.rs` → `Tokens` global), NAIC↔neutral
//! label set (`labels.rs`) and locale number format (`viewmodel/format.rs`), app-config
//! persistence in the OS config dir (`config.rs`, ADD7 — never inside the journal). Charts are
//! drawn natively in Slint (`Path` + `TouchArea`); there is no web view and no egui.
//!
//! # Internationalisation (UX-DR29)
//!
//! Every user-visible string in `ui/**/*.slint` is wrapped in `@tr()` with **French source
//! text**; no translation catalogs ship yet. When a second UI language lands, the drop-in
//! pipeline is:
//!
//! 1. extract: `cargo install slint-tr-extractor`, then
//!    `find ui -name '*.slint' | xargs slint-tr-extractor -o steadyinvest-app.pot`
//! 2. translate the `.pot` into per-language `.po` files (e.g. `lang/en/steadyinvest-app.po`);
//! 3. bundle: enable `slint-build`'s bundled-translations support in `build.rs`
//!    (`CompilerConfiguration::with_bundled_translations("lang")`) so catalogs compile into the
//!    binary — single-binary posture, no gettext system dependency;
//! 4. switch at runtime with `slint::select_bundled_translation("en")`.
//!
//! This axis is strictly distinct from the NAIC↔neutral label set (`labels.rs`), which is a
//! runtime data table of method vocabulary, not a translation.

// Story 2.2 put `contract`, `persistence`, `uuid` and `chrono` onto the runtime path; Story 2.3
// added `core` (the faithful form header shows `core::METHOD_VERSION`, a static method-identity
// `&str` — display, NOT the forbidden engine call); Story 2.4 promotes `arboard` (production
// paste-a-column clipboard read) and `rust_decimal` (locale entry parsing → `contract::Money`) from
// dev-only to genuinely-used runtime deps. So the crate-wide allow now covers a SHRUNK set: only
// `ingestion`, `report` and `tokio` remain unused (they light up in Epic 3 — ingestion/report data
// flow, tokio async provider I/O). (Scoping a crate-level lint allow to specific deps is not
// expressible; the comment is the scope of record, re-verified each story.)
#![allow(unused_crate_dependencies)]

mod clock;
mod config;
mod fetch;
mod keychain;
mod labels;
mod posture;
mod provider;
mod regime;
mod seam_check;
mod state;
mod theme;
mod viewmodel;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use uuid::Uuid;

use crate::clock::{SystemClock, UuidGen};
use crate::config::{AppConfig, StudyViewState};
use crate::labels::LabelSet;
use crate::provider::ProviderChoice;
use crate::regime::Regime;
use crate::state::{JournalState, UnlockScope};
use crate::theme::Theme;
use crate::viewmodel::format::{format_amount, NumberFormat};

slint::include_modules!();

/// Constant sample amounts for the Settings locale panel (formatted for display, computed
/// nothing — Cardinal Rule). Two stacked values with different digits double as the tabular-
/// figures check for the numeric font.
const SAMPLE_AMOUNT: &str = "-1234567.89";
const SAMPLE_AMOUNT_ALT: &str = "8888888.88";

/// Persist `config`, surfacing (not swallowing) a failure — a config that cannot be written is
/// a visible event, never a silence, but it must not take the app down.
fn persist(path: Option<&PathBuf>, config: &AppConfig) {
    let Some(path) = path else { return };
    if let Err(error) = config::save(path, config) {
        let message = format!("app-config save to {} failed: {error}", path.display());
        tracing::warn!("{message}");
        eprintln!("steadyinvest: {message}");
    }
}

/// The legacy interim key source (Story 3.1), kept ONLY as a fallback for environments with no
/// running OS secret agent (headless/NAS — AC5/AC6).
const ENV_KEY_FALLBACK: &str = "STEADYINVEST_EODHD_API_KEY";

/// Resolve the API key for a fetch/test (Story 3.2): the OS keychain first, then the env-var
/// fallback. `None` for a keyless provider or when no key is found anywhere. The key value is never
/// logged — only the fact that the fallback was used.
fn resolve_provider_key(provider: ProviderChoice) -> Option<String> {
    if !provider.requires_key() {
        return None;
    }
    match keychain::get_key(provider) {
        Ok(Some(key)) => return Some(key),
        Ok(None) => {}
        // Store unavailable (no agent) — fall through to the env fallback rather than fail (AC6).
        Err(_) => {}
    }
    // Trim consistently with the keychain path (which stores `key.trim()`), so a padded env value
    // doesn't fetch with stray whitespace the provider would reject (F9).
    let env_key = std::env::var(ENV_KEY_FALLBACK)
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty());
    if env_key.is_some() {
        tracing::info!(
            "provider key read from the {ENV_KEY_FALLBACK} fallback environment variable"
        );
    }
    env_key
}

/// Mirror the provider choice + keychain status into `Prefs` (Story 3.2). The key VALUE never
/// crosses — only the boolean "configured" status (NFR-S1). A store failure shows as "not
/// configured" plus a neutral notice (AC6).
fn mirror_provider_prefs(ui: &MainWindow, provider: ProviderChoice) {
    let prefs = ui.global::<Prefs>();
    prefs.set_provider(provider.wire().into());
    // A read failure (no secret agent) is reported as "not configured" — SILENTLY here: this runs at
    // startup and on provider switch, before the user has asked for anything. The explicit
    // save/delete/test actions surface `MSG_KEYCHAIN_UNAVAILABLE` when the user actually acts (AC6),
    // and a fetch still falls back to the env-var key.
    let configured = provider.requires_key() && keychain::has_key(provider).unwrap_or(false);
    prefs.set_key_configured(configured);
}

fn push_samples(ui: &MainWindow, format: NumberFormat) {
    let prefs = ui.global::<Prefs>();
    prefs.set_sample_amount(format_amount(SAMPLE_AMOUNT, format).into());
    prefs.set_sample_amount_alt(format_amount(SAMPLE_AMOUNT_ALT, format).into());
}

/// Rebuild the dashboard list from the journal and mirror the read-only flag into the `Studies`
/// global. Called on startup and after every create.
fn refresh_studies(ui: &MainWindow, state: &JournalState) {
    let studies = ui.global::<Studies>();
    // The dashboard view state (search/sort/filter) lives on the `Studies` global — read it back and
    // curate the persistence summaries (Story 2.12). Deterministic, pure (`viewmodel::studies::curate`).
    let summaries = state.list_studies();
    let rows: Vec<StudyRow> = viewmodel::studies::curate(
        &summaries,
        studies.get_search_query().as_str(),
        viewmodel::studies::SortKey::from_wire(studies.get_sort_key().as_str()),
        studies.get_sort_descending(),
        viewmodel::studies::StatusFilter::from_wire(studies.get_status_filter().as_str()),
    );
    studies.set_study_count(summaries.len() as i32);
    studies.set_rows(ModelRc::new(VecModel::from(rows)));
    studies.set_read_only(state.is_read_only());
}

/// Mirror a per-study view-state (regime + fold flags) into the `Studies` global and swap the
/// regime-driven token snapshot. The single place the UI's fold/regime state is pushed, so the
/// open-study restore and the toggle/regime callbacks stay consistent (one source of truth).
fn push_view_state(ui: &MainWindow, view_state: &StudyViewState) {
    let studies = ui.global::<Studies>();
    studies.set_regime(view_state.regime.as_str().into());
    studies.set_folds(ModelRc::new(VecModel::from(view_state.folds.to_vec())));
    regime::apply(ui, view_state.regime);
}

/// Rebuild the faithful-form structs from a (re-read) `Study` and push them into the `Studies`
/// global — the single source of truth for the open form (Story 2.3 header + §3 rows, Story 2.4 §2
/// management grid + year headers, **Story 2.6 the engine outputs + judgment inputs + §4 zone bar +
/// verdict**). Called on open and after every persisted edit so the UI always renders exactly what is
/// on disk + the coherent snapshot recomputed from it. Money/ratios cross only as formatted strings
/// (the adapter boundary); the verdict crosses as an enum-derived string.
///
/// The engine call goes through the single construction path [`engine::build_snapshot`] (ONE
/// `StudySnapshot::new`), so the §2–§5 results, the §4 zone bar and the verdict are always one
/// coherent frame. A `NormalizeError` (unreachable from a well-formed manual study, but handled, never
/// `unwrap`) surfaces as a neutral notice and leaves every computed slot the faithful em-dash.
fn push_form(
    ui: &MainWindow,
    state: &JournalState,
    study: &steadyinvest_contract::Study,
    format: NumberFormat,
) {
    use viewmodel::engine;
    let studies = ui.global::<Studies>();
    // Story 2.9 — mirror undo/redo availability so the header controls enable/disable in step with
    // every persisted edit (an edit grows undo + clears redo; undo/redo move between the stacks).
    studies.set_can_undo(state.can_undo());
    studies.set_can_redo(state.can_redo());
    studies.set_form_header(viewmodel::form::header(study));
    studies.set_year_headers(ModelRc::new(VecModel::from(viewmodel::form::year_headers(
        study,
    ))));
    // The current judgment-input values (restored on reopen; "" for a cleared input, never "0").
    studies.set_judgment(engine::judgment_fields(study, format));
    // Story 2.10 — the study-level decision rationale (FR49), restored on reopen; "" when unset
    // (the note re-seeds from this only while it does NOT have focus, the keep-input discipline).
    studies.set_rationale(study.rationale.clone().unwrap_or_default().into());

    let years = viewmodel::form::materialized_year_numbers(study);
    match engine::build_frame(study) {
        Ok(frame) => {
            let snapshot = &frame.snapshot;
            let outputs = snapshot.outputs();
            // Story 2.7 — map BOTH finding sets (input-shape off the frame + calc-time off the
            // outputs) to per-cell / study-level warnings against the SAME materialized window the
            // grids render, so the verdict and the warnings descend from one coherent frame.
            let warnings = engine::plausibility(&frame.plausibility, &outputs.findings, &years);
            studies.set_pe_rows(ModelRc::new(VecModel::from(viewmodel::form::pe_rows(
                study,
                format,
                Some(outputs),
                &warnings,
            ))));
            studies.set_mgmt_rows(ModelRc::new(VecModel::from(viewmodel::form::mgmt_rows(
                study, format, &warnings,
            ))));
            studies.set_growth_computed(engine::growth_computed(outputs, format));
            studies.set_mgmt_computed(engine::mgmt_computed(outputs, &years, format));
            studies.set_pe_computed(engine::pe_computed(outputs, format));
            studies.set_risk_computed(engine::risk_computed(outputs, format));
            studies.set_return_computed(engine::return_computed(outputs, format));
            studies.set_zone_bar(engine::zone_bar(study, snapshot, format));
            studies.set_verdict(engine::verdict_badge(study, snapshot, format));
            // Story 2.8 — the §1 interactive growth chart geometry (from the SAME coherent frame).
            studies.set_growth_chart(viewmodel::chart::growth_chart(&frame, format));
            // The study-level (§4) warning key — `low_price_above_current`, anchored near forecast-low.
            studies.set_section4_warning_key(
                warnings
                    .study_key()
                    .map(|k| k.as_str())
                    .unwrap_or("")
                    .into(),
            );
        }
        Err(error) => {
            // Degraded-but-safe: the form still renders, every computed slot the em-dash; the verdict
            // and zone bar fall back to their calm empty states; no warning channel speaks.
            tracing::warn!("snapshot normalize failed: {error}");
            let no_warnings = engine::PlausibilityWarnings::default();
            studies.set_pe_rows(ModelRc::new(VecModel::from(viewmodel::form::pe_rows(
                study,
                format,
                None,
                &no_warnings,
            ))));
            studies.set_mgmt_rows(ModelRc::new(VecModel::from(viewmodel::form::mgmt_rows(
                study,
                format,
                &no_warnings,
            ))));
            studies.set_growth_computed(GrowthComputed::default());
            studies.set_mgmt_computed(MgmtComputed::default());
            studies.set_pe_computed(PeComputed::default());
            studies.set_risk_computed(RiskComputed::default());
            studies.set_return_computed(ReturnComputed::default());
            studies.set_zone_bar(ZoneBarState::default());
            studies.set_verdict(VerdictState::default());
            studies.set_growth_chart(viewmodel::chart::unavailable());
            studies.set_section4_warning_key(SharedString::new());
            studies.set_notice(state::MSG_NORMALIZE_FAILED.into());
        }
    }
}

/// A LIVE, NON-persisted recompute frame for a §1 judgment-line drag (Story 2.8, NFR-P1). Builds ONE
/// coherent [`engine::build_frame`] from the in-memory (un-saved) study and pushes only the surfaces a
/// drag moves — the judgment fields (so the exact-value field mirrors the line, FR31), the §1 chart
/// line, the §4 zone bar and the verdict. Deliberately does NOT touch the journal or rebuild the whole
/// form (`push_form`'s per-edit `put_study` + full rebuild is far too heavy per `moved` event — the
/// recompute itself is sub-millisecond, the cost to avoid is the per-event write). A transient
/// normalize error mid-drag leaves the last good frame untouched (never a flash of blanked outputs).
fn push_live_preview(ui: &MainWindow, study: &steadyinvest_contract::Study, format: NumberFormat) {
    use viewmodel::engine;
    let studies = ui.global::<Studies>();
    if let Ok(frame) = engine::build_frame(study) {
        let snapshot = &frame.snapshot;
        let outputs = snapshot.outputs();
        let years = viewmodel::form::materialized_year_numbers(study);
        let warnings = engine::plausibility(&frame.plausibility, &outputs.findings, &years);
        studies.set_judgment(engine::judgment_fields(study, format));
        studies.set_growth_chart(viewmodel::chart::growth_chart(&frame, format));
        studies.set_zone_bar(engine::zone_bar(study, snapshot, format));
        studies.set_verdict(engine::verdict_badge(study, snapshot, format));
        // §4/§5 judgment-dependent numbers stay in step with the recolouring bar (review P1) — the
        // forecast high/low + U/D, the projected return, and the §4 study-level warning all move
        // with the est-high-EPS the drag sets, so the §4 surface never disagrees with itself.
        studies.set_risk_computed(engine::risk_computed(outputs, format));
        studies.set_return_computed(engine::return_computed(outputs, format));
        studies.set_section4_warning_key(
            warnings
                .study_key()
                .map(|k| k.as_str())
                .unwrap_or("")
                .into(),
        );
    }
}

/// Build an [`UnlockScope`] from the Slint callback's `(scope-kind, scope-arg)` pair (Story 2.5).
/// `"study"` ignores the arg; `"year"` parses the arg as a year-window index; `"metric"` takes the
/// arg as a field key. `None` for an unknown kind / unparseable index (the caller no-ops safely).
fn parse_unlock_scope(kind: &str, arg: &str) -> Option<UnlockScope> {
    match kind {
        "study" => Some(UnlockScope::Study),
        "year" => arg.parse::<usize>().ok().map(UnlockScope::Year),
        "metric" => Some(UnlockScope::Metric(arg.to_string())),
        _ => None,
    }
}

fn main() -> Result<(), slint::PlatformError> {
    // Minimal logging: tracing carries the events; a real subscriber (ADD15 rotating logs) is
    // deferred — see the Story 2.1 GitHub issue. Until then load warnings also go to stderr.
    // Story 3.1 — install the pure-Rust `ring` crypto provider for rustls before any HTTPS fetch
    // (reqwest uses `rustls-no-provider`). Idempotent; must run before the fetch worker is used.
    steadyinvest_ingestion::install_crypto_provider();

    let config_path = config::default_path();
    if config_path.is_none() {
        eprintln!("steadyinvest: no OS config directory found; preferences are not persisted");
    }
    let loaded = match &config_path {
        Some(path) => config::load(path),
        None => config::Loaded {
            config: AppConfig::default(),
            warning: None,
        },
    };
    if let Some(warning) = &loaded.warning {
        eprintln!("steadyinvest: {warning}");
    }
    let config = Rc::new(RefCell::new(loaded.config));

    // Open the last-used journal (or create the default one), with identity + time from the
    // injected sources (ADD15). This is the first time the app opens the journal — Story 2.1
    // deliberately did not. Failure degrades to a usable journal-less state, never a crash.
    let configured = config.borrow().journal_path.clone();
    let (journal_state, startup_notice) = JournalState::open_or_create(
        configured.as_deref(),
        Box::new(SystemClock),
        Box::new(UuidGen),
    );
    // Persist the resolved path so the same journal reopens next launch (only when it changed).
    {
        let resolved = journal_state.path().map(Path::to_path_buf);
        let mut cfg = config.borrow_mut();
        if cfg.journal_path != resolved {
            cfg.journal_path = resolved;
            drop(cfg);
            persist(config_path.as_ref(), &config.borrow());
        }
    }
    let journal_state = Rc::new(RefCell::new(journal_state));

    // The id (stringified UUID) of the study whose form is currently open, so the fold/regime
    // callbacks know which `study_view_state` entry to mutate. `None` = the dashboard list view.
    let current_study: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    // The scope of a pending "unlock all" (Story 2.5), held between the confirmation overlay being
    // raised (`request-unlock`) and the user pressing Confirmer (`confirm-unlock`). `None` = no
    // pending action; Annuler clears it without mutating anything.
    let pending_unlock: Rc<RefCell<Option<UnlockScope>>> = Rc::new(RefCell::new(None));

    // Story 2.12 — the pending dashboard lifecycle action `(action, study_id)`, held between
    // `request-study-action` raising the confirm overlay and `confirm-study-action`. `None` = no
    // pending action; Annuler clears it without mutating anything.
    let pending_study_action: Rc<RefCell<Option<(String, Uuid)>>> = Rc::new(RefCell::new(None));

    // Story 2.8 — the open study cached for the duration of a §1 judgment-line drag, so each `moved`
    // event recomputes from memory (no journal read/write) under the <100 ms budget. `None` = no drag
    // in flight; set on pointer-down, cleared on release/commit.
    let drag_study: Rc<RefCell<Option<steadyinvest_contract::Study>>> = Rc::new(RefCell::new(None));
    // Whether the in-flight drag actually moved (a `moved` event fired). A pointer-up with no
    // movement is a click, not a drag — it must NOT rewrite the forecast (review P2).
    let drag_moved: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

    // Story 2.9 — the saved study captured while the scenario-compare overlay is open, so the typed
    // alternate is recomputed against it WITHOUT persisting. `None` = the overlay is closed.
    let compare_study: Rc<RefCell<Option<steadyinvest_contract::Study>>> =
        Rc::new(RefCell::new(None));

    let ui = MainWindow::new()?;

    // Initial state, pushed before the window shows: no flash, no restart needed later.
    {
        let cfg = config.borrow();
        theme::apply(&ui, cfg.theme);
        labels::apply(&ui, cfg.label_set);
        let prefs = ui.global::<Prefs>();
        prefs.set_dark_theme(cfg.theme == Theme::Dark);
        prefs.set_label_set(cfg.label_set.as_str().into());
        prefs.set_number_format(cfg.number_format.as_str().into());
        push_samples(&ui, cfg.number_format);
        mirror_provider_prefs(&ui, cfg.preferred_provider);

        // Best-effort restore BEFORE show to minimise the visible jump; the authoritative
        // restore happens again right after show() below — before the window is mapped, winit
        // (X11) has no scale factor yet and silently misapplies a physical size.
        let (width, height) = cfg.sane_window_size();
        ui.window()
            .set_size(slint::PhysicalSize::new(width, height));
        if cfg.maximized {
            ui.window().set_maximized(true);
        }
    }

    // Initial Studies state: the list, the read-only flag, and any startup notice (read-only
    // journal / unreadable configured file). Pushed before the window shows.
    {
        refresh_studies(&ui, &journal_state.borrow());
        if let Some(notice) = &startup_notice {
            ui.global::<Studies>().set_notice(notice.clone().into());
        }
    }

    // ── Story 3.1 — provider auto-fetch (EODHD), off the UI thread ──
    // The worker thread owns the tokio runtime + network I/O; results return via the thread_local
    // handler set below (which holds the `Rc` state and runs on the UI thread).
    let fetch_tx = fetch::spawn_fetch_worker();
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let current_study = Rc::clone(&current_study);
        let config = Rc::clone(&config);
        fetch::set_outcome_handler(move |outcome| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let studies = ui.global::<Studies>();
            match outcome {
                fetch::WorkerOutcome::Fetch(outcome) => {
                    // Clear the in-progress flag ONLY for a study fetch — a key-test (which never
                    // sets `fetching`) must not re-enable the study Fetch button mid-fetch (F5).
                    studies.set_fetching(false);
                    match outcome.result {
                        Ok(fetched) => {
                            let applied = journal_state
                                .borrow_mut()
                                .apply_provider_refresh(outcome.study_id, &fetched);
                            match applied {
                                Ok(report) => {
                                    // Name the recompute cause (FR29): price / fundamentals / both,
                                    // or "no change" when an idempotent re-fetch moved nothing.
                                    studies.set_notice(state::refresh_notice(report).into());
                                    // Re-render the form + recompute if this study is still the open one.
                                    let still_open = current_study
                                        .borrow()
                                        .as_deref()
                                        .and_then(|s| Uuid::parse_str(s).ok())
                                        == Some(outcome.study_id);
                                    if still_open {
                                        if let Some(study) =
                                            journal_state.borrow().get_study(outcome.study_id)
                                        {
                                            let format = config.borrow().number_format;
                                            push_form(&ui, &journal_state.borrow(), &study, format);
                                        }
                                    }
                                    refresh_studies(&ui, &journal_state.borrow());
                                }
                                Err(message) => studies.set_notice(message.into()),
                            }
                        }
                        Err(error) => studies.set_notice(
                            state::MSG_PROVIDER_FAILED
                                .replace("{cause}", &error.to_string())
                                .into(),
                        ),
                    }
                }
                fetch::WorkerOutcome::TestKey(result) => {
                    // The key test (Story 3.2): a verdict, not study data. Surface it as the
                    // provider/key status in Réglages (cause-named on failure).
                    let prefs = ui.global::<Prefs>();
                    let status = match result {
                        Ok(()) => state::MSG_KEY_OK.to_string(),
                        Err(steadyinvest_ingestion::IngestionError::Provider(
                            steadyinvest_ingestion::ProviderError::InvalidOrAbsentKey,
                        )) => state::MSG_KEY_INVALID.to_string(),
                        // 403: the key is valid but the plan/account is not authorized (e.g. EODHD
                        // free tier excludes fundamentals) — say so honestly, not "key invalid".
                        Err(steadyinvest_ingestion::IngestionError::Provider(
                            steadyinvest_ingestion::ProviderError::Forbidden { .. },
                        )) => state::MSG_KEY_FORBIDDEN.to_string(),
                        Err(error) => {
                            state::MSG_PROVIDER_FAILED.replace("{cause}", &error.to_string())
                        }
                    };
                    prefs.set_provider_status(status.into());
                }
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let current_study = Rc::clone(&current_study);
        let config = Rc::clone(&config);
        let fetch_tx = fetch_tx.clone();
        ui.global::<Studies>().on_fetch_provider(move || {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            // The open study (the demo keeps `current_study` None → no fetch on the demo).
            let Some(study_id) = current_study
                .borrow()
                .as_deref()
                .and_then(|s| Uuid::parse_str(s).ok())
            else {
                return;
            };
            let Some(ticker) = journal_state
                .borrow()
                .get_study(study_id)
                .map(|s| s.security_ticker)
            else {
                return;
            };
            // Key source (Story 3.2): the OS keychain for the preferred provider, with an env-var
            // fallback for environments that have no running secret agent (AC5/AC6). A keyless
            // provider fetches with no key; a key-requiring provider with no key found is refused.
            let provider_choice = config.borrow().preferred_provider;
            let api_key = resolve_provider_key(provider_choice);
            if provider_choice.requires_key() && api_key.is_none() {
                studies.set_notice(state::MSG_PROVIDER_NO_KEY.into());
                return;
            }
            studies.set_fetching(true);
            studies.set_notice(state::MSG_PROVIDER_FETCHING.into());
            if fetch_tx
                .send(fetch::WorkerJob::Fetch(fetch::FetchRequest {
                    study_id,
                    ticker,
                    api_key,
                }))
                .is_err()
            {
                // The worker thread is gone (should never happen) — don't latch the in-progress
                // state, which would disable the button for the rest of the session (review P1).
                studies.set_fetching(false);
                studies.set_notice(
                    state::MSG_PROVIDER_FAILED
                        .replace("{cause}", "le service de récupération est indisponible")
                        .into(),
                );
            }
        });
    }

    // Create-study intent: validate + persist via the injected sources, then refresh the list.
    // A refused create (blank input / read-only / save failure) surfaces a neutral banner; a
    // successful one clears it.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Studies>()
            .on_create_study(move |ticker, currency| {
                let ui = ui_weak.unwrap();
                let result = journal_state.borrow_mut().create_study(&ticker, &currency);
                let studies = ui.global::<Studies>();
                let written = result.is_ok();
                match result {
                    Ok(_id) => studies.set_notice(SharedString::new()),
                    Err(message) => studies.set_notice(message.into()),
                }
                refresh_studies(&ui, &journal_state.borrow());
                // Report whether a study was written so the UI keeps the user's input on refusal.
                written
            });
    }

    // Open-study intent: reopen with full state and mount the faithful §1–§5 form (Story 2.3).
    // Money surfaces as formatted strings via the form adapter (the only float→string boundary), and
    // the persisted per-study fold/regime view-state is restored (default = Entry + all open).
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        let compare_study = Rc::clone(&compare_study);
        ui.global::<Studies>().on_open_study(move |id_text| {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            let Some(study) = journal_state.borrow().get_study(id) else {
                return;
            };
            // Story 2.9 — a freshly-opened study starts with an empty undo/redo history (the edit
            // history is per open study, in-memory, never carried across reopen). Reset BEFORE
            // push_form so the mirrored can-undo/can-redo flags read empty.
            journal_state.borrow_mut().reset_undo();
            // Also discard any scenario-compare state from a previous study (review P3) — its overlay
            // and cached baseline must never survive into a different study.
            *compare_study.borrow_mut() = None;
            studies.set_scenario_compare(ScenarioCompareState::default());
            let format = config.borrow().number_format;
            push_form(&ui, &journal_state.borrow(), &study, format);
            // A freshly-opened form has no active entry cell (the cursor appears on first focus).
            studies.set_active_year(-1);
            studies.set_active_field(SharedString::new());
            studies.set_active_source(SharedString::new());
            studies.set_active_warning(SharedString::new());
            // Defensively clear any stuck drag state (review P5): if a previous study was closed
            // mid-drag the `up`/`cancel` may never have fired, which would leave the form's scroll
            // disabled. Opening a study always starts from a clean, scrollable state.
            studies.set_judgment_dragging(false);
            let view_state = config
                .borrow()
                .study_view_state
                .get(id_text.as_str())
                .cloned()
                .unwrap_or_default();
            push_view_state(&ui, &view_state);
            *current_study.borrow_mut() = Some(id_text.to_string());
            studies.set_demo_active(false); // opening a real study leaves the demo (Story 2.13)
            studies.set_study_open(true);
        });
    }

    // Fold a section: mutate this study's `study_view_state` (validate index → mutate → persist), then
    // push the new state back (one source of truth). Mirrors the 2.2 Prefs persistence shape — no
    // silent `.ok()`: a save failure surfaces via `persist`.
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let path = config_path.clone();
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>().on_toggle_fold(move |index, open| {
            let Some(id) = current_study.borrow().clone() else {
                return;
            };
            if !(0..regime::SECTION_COUNT as i32).contains(&index) {
                return;
            }
            let ui = ui_weak.unwrap();
            let new_state = {
                let mut cfg = config.borrow_mut();
                let entry = cfg.study_view_state.entry(id).or_default();
                entry.folds[index as usize] = open;
                entry.clone()
            };
            persist(path.as_ref(), &config.borrow());
            push_view_state(&ui, &new_state);
        });
    }

    // Switch regime: apply the regime's fold preset + swap the regime token snapshot, persist, push.
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let path = config_path.clone();
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>().on_set_regime(move |value| {
            let Some(new_regime) = Regime::parse(&value) else {
                return;
            };
            let Some(id) = current_study.borrow().clone() else {
                return;
            };
            // Selecting the already-active regime is a no-op (AC3: only an actual *switch* applies
            // the fold preset). Re-applying the preset here would silently clobber the user's manual
            // fold edits made within this regime.
            let current_regime = config
                .borrow()
                .study_view_state
                .get(&id)
                .map(|state| state.regime)
                .unwrap_or_default();
            if current_regime == new_regime {
                return;
            }
            let ui = ui_weak.unwrap();
            let new_state = {
                let mut cfg = config.borrow_mut();
                let entry = cfg.study_view_state.entry(id).or_default();
                entry.regime = new_regime;
                entry.folds = new_regime.fold_preset();
                entry.clone()
            };
            persist(path.as_ref(), &config.borrow());
            push_view_state(&ui, &new_state);
        });
    }

    // ── Manual-entry intents (Story 2.4): parse → `Cell::edited` → `put_study` → re-read → re-push
    //    (validate→mutate→persist→rebuild — the 2.3 single-source-of-truth shape). Every refusal
    //    surfaces a neutral banner, never a silent `.ok()`. ──

    // Commit a typed cell: parse the text locale-aware (None for blank/unparseable → a to-fill gap,
    // never 0), edit + persist, then rebuild the form from the re-read study. Returns written? so
    // the cell keeps the user's text on a recoverable refusal.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>()
            .on_commit_cell(move |year_index, field, text| {
                let ui = ui_weak.unwrap();
                let studies = ui.global::<Studies>();
                let Some(id_text) = current_study.borrow().clone() else {
                    return false;
                };
                let Ok(id) = Uuid::parse_str(&id_text) else {
                    return false;
                };
                let format = config.borrow().number_format;
                let value = viewmodel::format::parse_amount(&text, format);
                let result = journal_state.borrow_mut().edit_cell(
                    id,
                    year_index.max(0) as usize,
                    field.as_str(),
                    value,
                );
                match result {
                    Ok(()) => {
                        studies.set_notice(SharedString::new());
                        if let Some(study) = journal_state.borrow().get_study(id) {
                            push_form(&ui, &journal_state.borrow(), &study, format);
                        }
                        true
                    }
                    Err(message) => {
                        studies.set_notice(message.into());
                        false
                    }
                }
            });
    }

    // Paste a clipboard column downward from the active cell (same field). Read the clipboard via
    // `arboard`; a failure is a neutral notice, never a panic. Surplus lines past the grid bottom
    // are dropped with a neutral count notice.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>()
            .on_paste_column(move |year_index, field| {
                let ui = ui_weak.unwrap();
                let studies = ui.global::<Studies>();
                let Some(id_text) = current_study.borrow().clone() else {
                    return;
                };
                let Ok(id) = Uuid::parse_str(&id_text) else {
                    return;
                };
                let format = config.borrow().number_format;
                let text = match arboard::Clipboard::new().and_then(|mut cb| cb.get_text()) {
                    Ok(text) => text,
                    Err(error) => {
                        tracing::warn!("clipboard read failed: {error}");
                        studies.set_notice(state::MSG_CLIPBOARD_UNAVAILABLE.into());
                        return;
                    }
                };
                let values = viewmodel::entry::parse_pasted_column(&text, format);
                if values.is_empty() {
                    return;
                }
                let result = journal_state.borrow_mut().paste_column(
                    id,
                    year_index.max(0) as usize,
                    field.as_str(),
                    &values,
                );
                match result {
                    Ok(filled) => {
                        if filled < values.len() {
                            studies.set_notice(state::MSG_PASTE_CLIPPED.into());
                        } else {
                            studies.set_notice(SharedString::new());
                        }
                        if let Some(study) = journal_state.borrow().get_study(id) {
                            push_form(&ui, &journal_state.borrow(), &study, format);
                        }
                    }
                    Err(message) => studies.set_notice(message.into()),
                }
            });
    }

    // Mark / clear not-available-accepted on a cell (a deliberate, quiet, permanent gap — distinct
    // from a to-fill gap and from a real 0).
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>()
            .on_set_not_available(move |year_index, field, accepted| {
                let ui = ui_weak.unwrap();
                let studies = ui.global::<Studies>();
                let Some(id_text) = current_study.borrow().clone() else {
                    return;
                };
                let Ok(id) = Uuid::parse_str(&id_text) else {
                    return;
                };
                let format = config.borrow().number_format;
                let result = journal_state.borrow_mut().set_not_available(
                    id,
                    year_index.max(0) as usize,
                    field.as_str(),
                    accepted,
                );
                match result {
                    Ok(()) => {
                        studies.set_notice(SharedString::new());
                        if let Some(study) = journal_state.borrow().get_study(id) {
                            push_form(&ui, &journal_state.borrow(), &study, format);
                        }
                    }
                    Err(message) => studies.set_notice(message.into()),
                }
            });
    }

    // Move the cell cursor: pure index math (no persistence). Rust computes the neighbour within the
    // grid and updates `active-year`/`active-field`; the target `EditableCell` then focuses itself.
    {
        let ui_weak = ui.as_weak();
        ui.global::<Studies>()
            .on_cell_move(move |year_index, field, dir| {
                let ui = ui_weak.unwrap();
                let studies = ui.global::<Studies>();
                let year_count = studies.get_pe_rows().row_count();
                let (next_year, next_field) = viewmodel::entry::next_cell(
                    year_index.max(0) as usize,
                    field.as_str(),
                    dir.as_str(),
                    year_count,
                );
                studies.set_active_year(next_year as i32);
                studies.set_active_field(next_field.into());
            });
    }

    // ── Tri-state review tag + soft-lock + bulk unlock (Story 2.5) ──

    // Set a cell's review tag (the cycle none→?→✓→none and the deliberate clear-✓ → ? both land
    // here; the UI computes the target). A review-only change — the value/coverage are never touched;
    // re-read + re-push so the marker reflects exactly what is on disk.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>()
            .on_set_review(move |year_index, field, review| {
                let ui = ui_weak.unwrap();
                let studies = ui.global::<Studies>();
                let Some(id_text) = current_study.borrow().clone() else {
                    return;
                };
                let Ok(id) = Uuid::parse_str(&id_text) else {
                    return;
                };
                let format = config.borrow().number_format;
                let result = journal_state.borrow_mut().set_review(
                    id,
                    year_index.max(0) as usize,
                    field.as_str(),
                    viewmodel::entry::review_from_str(review.as_str()),
                );
                match result {
                    Ok(()) => {
                        studies.set_notice(SharedString::new());
                        if let Some(study) = journal_state.borrow().get_study(id) {
                            push_form(&ui, &journal_state.borrow(), &study, format);
                        }
                    }
                    Err(message) => studies.set_notice(message.into()),
                }
            });
    }

    // A value edit was attempted on a soft-locked (✓) cell: raise the neutral notice (the value is
    // never mutated — the refusal is enforced both here and in `state::edit_cell`).
    {
        let ui_weak = ui.as_weak();
        ui.global::<Studies>().on_notify_soft_lock(move || {
            let ui = ui_weak.unwrap();
            ui.global::<Studies>()
                .set_notice(state::MSG_SOFT_LOCKED.into());
        });
    }

    // Request an "unlock all": count the ✓ cells the chosen scope covers and raise the confirmation
    // overlay (the bulk flip is never silent). The scope is parked in `pending_unlock` until Confirmer.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let current_study = Rc::clone(&current_study);
        let pending_unlock = Rc::clone(&pending_unlock);
        ui.global::<Studies>()
            .on_request_unlock(move |scope_kind, scope_arg| {
                let ui = ui_weak.unwrap();
                let studies = ui.global::<Studies>();
                let Some(id_text) = current_study.borrow().clone() else {
                    return;
                };
                let Ok(id) = Uuid::parse_str(&id_text) else {
                    return;
                };
                let Some(scope) = parse_unlock_scope(scope_kind.as_str(), scope_arg.as_str())
                else {
                    return;
                };
                let count = journal_state.borrow().count_validated(id, &scope);
                *pending_unlock.borrow_mut() = Some(scope);
                studies.set_confirm_message(state::unlock_confirm_message(count).into());
                studies.set_confirm_visible(true);
            });
    }

    // Confirm the pending "unlock all": flip every ✓→? in the parked scope in one upsert, surface a
    // neutral "N flipped" notice, re-read + re-push, and dismiss the overlay.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        let pending_unlock = Rc::clone(&pending_unlock);
        ui.global::<Studies>().on_confirm_unlock(move || {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            studies.set_confirm_visible(false);
            let Some(scope) = pending_unlock.borrow_mut().take() else {
                return;
            };
            let Some(id_text) = current_study.borrow().clone() else {
                return;
            };
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            let format = config.borrow().number_format;
            match journal_state.borrow_mut().unlock_all(id, &scope) {
                Ok(count) => {
                    studies.set_notice(state::unlock_done_message(count).into());
                    if let Some(study) = journal_state.borrow().get_study(id) {
                        push_form(&ui, &journal_state.borrow(), &study, format);
                    }
                }
                Err(message) => studies.set_notice(message.into()),
            }
        });
    }

    // Cancel a pending "unlock all": dismiss the overlay and forget the scope — nothing is mutated.
    {
        let ui_weak = ui.as_weak();
        let pending_unlock = Rc::clone(&pending_unlock);
        ui.global::<Studies>().on_cancel_unlock(move || {
            let ui = ui_weak.unwrap();
            *pending_unlock.borrow_mut() = None;
            ui.global::<Studies>().set_confirm_visible(false);
        });
    }

    // ── Story 2.12 — dashboard search / sort / filter. The view state lives on the `Studies` global;
    //    each control updates it then re-lists + re-curates (pure `viewmodel::studies::curate`). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Studies>().on_set_search(move |text| {
            let ui = ui_weak.unwrap();
            ui.global::<Studies>().set_search_query(text);
            refresh_studies(&ui, &journal_state.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Studies>().on_set_sort(move |key, descending| {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            studies.set_sort_key(key);
            studies.set_sort_descending(descending);
            refresh_studies(&ui, &journal_state.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Studies>().on_set_status_filter(move |filter| {
            let ui = ui_weak.unwrap();
            ui.global::<Studies>().set_status_filter(filter);
            refresh_studies(&ui, &journal_state.borrow());
        });
    }

    // ── Story 2.12 — archive (soft) / un-archive / delete (hard), each behind the confirm overlay
    //    (mirrors the unlock request→confirm→cancel pattern; a SEPARATE `study-action` channel). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let pending_study_action = Rc::clone(&pending_study_action);
        ui.global::<Studies>()
            .on_request_study_action(move |action, id_text| {
                let ui = ui_weak.unwrap();
                let studies = ui.global::<Studies>();
                let Ok(id) = Uuid::parse_str(&id_text) else {
                    return;
                };
                // The ticker for the fact-stating prompt (user data, not scanned); absent study → bail.
                let Some(study) = journal_state.borrow().get_study(id) else {
                    return;
                };
                let action = action.to_string();
                let message = state::study_action_confirm_message(&action, &study.security_ticker);
                let destructive = action == "delete";
                *pending_study_action.borrow_mut() = Some((action, id));
                studies.set_study_action_message(message.into());
                studies.set_study_action_destructive(destructive);
                studies.set_study_action_confirm_visible(true);
            });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let current_study = Rc::clone(&current_study);
        let pending_study_action = Rc::clone(&pending_study_action);
        ui.global::<Studies>().on_confirm_study_action(move || {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            studies.set_study_action_confirm_visible(false);
            let Some((action, id)) = pending_study_action.borrow_mut().take() else {
                return;
            };
            // The ticker for the completion notice, captured before a delete removes the row.
            let ticker = journal_state
                .borrow()
                .get_study(id)
                .map(|s| s.security_ticker)
                .unwrap_or_default();
            let result = {
                let mut st = journal_state.borrow_mut();
                match action.as_str() {
                    "archive" => st.archive_study(id),
                    "unarchive" => st.unarchive_study(id),
                    _ => st.delete_study(id),
                }
            };
            match result {
                Ok(()) => {
                    studies.set_notice(state::study_action_done_message(&action, &ticker).into());
                    // If the affected study is the one currently open, close it back to the dashboard
                    // (a hidden/removed study must not stay mounted).
                    let is_open =
                        current_study.borrow().as_deref() == Some(id.to_string().as_str());
                    if is_open {
                        *current_study.borrow_mut() = None;
                        studies.set_study_open(false);
                    }
                    refresh_studies(&ui, &journal_state.borrow());
                }
                Err(message) => studies.set_notice(message.into()),
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let pending_study_action = Rc::clone(&pending_study_action);
        ui.global::<Studies>().on_cancel_study_action(move || {
            let ui = ui_weak.unwrap();
            *pending_study_action.borrow_mut() = None;
            ui.global::<Studies>()
                .set_study_action_confirm_visible(false);
        });
    }

    // ── Story 2.13 — verify engine (FR9): replay the bundled golden fixtures through core and push the
    //    method identity + per-fixture pass/deviation report into the `Verify` global (Réglages hub). ──
    {
        let ui_weak = ui.as_weak();
        ui.global::<Verify>().on_run(move || {
            let ui = ui_weak.unwrap();
            let report = viewmodel::verify::run();
            let verify = ui.global::<Verify>();
            verify.set_method_version(report.method_version.as_str().into());
            verify.set_method_fingerprint(report.method_fingerprint.as_str().into());
            verify.set_summary(state::verify_summary(report.passed_count, report.total).into());
            verify.set_all_passed(report.all_passed());
            let lines: Vec<FixtureLine> = report
                .results
                .iter()
                .map(|r| FixtureLine {
                    id: r.id.as_str().into(),
                    passed: r.passed,
                    detail: r.deviations.join(" ; ").into(),
                })
                .collect();
            verify.set_results(ModelRc::new(VecModel::from(lines)));
            verify.set_ran(true);
        });
    }

    // ── Story 2.13 — load the read-only demonstration study (FR62): build the bundled worked-example
    //    in memory and render it via `push_form`. `current_study` stays None, so every edit rail
    //    no-ops and nothing reaches the journal. `demo-active` drives the "lecture seule" banner. ──
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Studies>().on_load_demo(move || {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            let format = config.borrow().number_format;
            match viewmodel::verify::demo_study() {
                Ok(study) => {
                    // The demo has no persisted view-state or undo history of its own: render it with
                    // the default (entry regime, all sections open) and an empty undo stack, so it never
                    // inherits the previously-open study's folds/regime or shows enabled undo/redo.
                    journal_state.borrow_mut().reset_undo();
                    push_form(&ui, &journal_state.borrow(), &study, format);
                    push_view_state(&ui, &StudyViewState::default());
                    studies.set_notice(SharedString::new());
                    studies.set_demo_active(true);
                    studies.set_study_open(true);
                }
                Err(_) => studies.set_notice(state::MSG_DEMO_UNAVAILABLE.into()),
            }
        });
    }

    // ── Numeric judgment-input editing + the §4 selector + traceability (Story 2.6) ──

    // Commit a numeric judgment field: parse locale-aware (None for blank/unparseable → cleared,
    // never 0), persist to `Study.judgment`, then re-read + re-push (which recomputes the snapshot).
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>().on_set_judgment(move |field, text| {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            let Some(id_text) = current_study.borrow().clone() else {
                return;
            };
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            let format = config.borrow().number_format;
            let value = viewmodel::format::parse_amount(&text, format);
            let result = journal_state
                .borrow_mut()
                .set_judgment_field(id, field.as_str(), value);
            match result {
                Ok(()) => {
                    studies.set_notice(SharedString::new());
                    if let Some(study) = journal_state.borrow().get_study(id) {
                        push_form(&ui, &journal_state.borrow(), &study, format);
                    }
                }
                Err(message) => studies.set_notice(message.into()),
            }
        });
    }

    // ── Story 2.10 — commit the study-level decision rationale (FR49). Mirrors `on_set_judgment`:
    //    parse-free (it's free text) → `state::set_rationale` (trims → Some/None, atomic, undoable) →
    //    re-read + `push_form` (refreshing the undo flags). Keep-input is the note's own concern. ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>().on_set_rationale(move |text| {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            let Some(id_text) = current_study.borrow().clone() else {
                return;
            };
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            let format = config.borrow().number_format;
            // Pass the raw text; `state::set_rationale` trims and maps empty → None (never Some("")).
            let result = journal_state
                .borrow_mut()
                .set_rationale(id, Some(text.to_string()));
            match result {
                Ok(()) => {
                    studies.set_notice(SharedString::new());
                    if let Some(study) = journal_state.borrow().get_study(id) {
                        push_form(&ui, &journal_state.borrow(), &study, format);
                    }
                }
                Err(message) => studies.set_notice(message.into()),
            }
        });
    }

    // ── Story 2.11 — extend the projection (FR3): the annual roll-forward. Mirrors `on_set_rationale`:
    //    structural (no payload) → `state::extend_history` (appends `latest_year + 1`, atomic, undoable)
    //    → re-read + `push_form` (the grid re-renders with the new ToFill column; undo flags refresh). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>().on_extend_history(move || {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            let Some(id_text) = current_study.borrow().clone() else {
                return;
            };
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            let format = config.borrow().number_format;
            let result = journal_state.borrow_mut().extend_history(id);
            match result {
                Ok(()) => {
                    studies.set_notice(SharedString::new());
                    if let Some(study) = journal_state.borrow().get_study(id) {
                        push_form(&ui, &journal_state.borrow(), &study, format);
                    }
                }
                Err(message) => studies.set_notice(message.into()),
            }
        });
    }

    // ── Story 2.8 — the draggable §1 judgment line (gesture ⇄ exact-value, kept in sync). ──

    // Drag start (pointer-down): cache the open study so each `moved` recomputes from memory — no
    // journal read/write during the drag (the per-event cost `push_form` would otherwise incur).
    {
        let journal_state = Rc::clone(&journal_state);
        let current_study = Rc::clone(&current_study);
        let drag_study = Rc::clone(&drag_study);
        let drag_moved = Rc::clone(&drag_moved);
        ui.global::<Studies>().on_judgment_drag_start(move || {
            *drag_moved.borrow_mut() = false;
            let Some(id_text) = current_study.borrow().clone() else {
                return;
            };
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            *drag_study.borrow_mut() = journal_state.borrow().get_study(id);
        });
    }

    // Drag move: map pointer-y → est-high-EPS, apply it to the CACHED (un-saved) study, push a LIVE
    // recompute frame — NO persistence (NFR-P1). The exact-value field mirrors the line (FR31).
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let drag_study = Rc::clone(&drag_study);
        let drag_moved = Rc::clone(&drag_moved);
        ui.global::<Studies>().on_judgment_moved(move |field, y| {
            let ui = ui_weak.unwrap();
            let Some(mut preview) = drag_study.borrow().clone() else {
                return;
            };
            *drag_moved.borrow_mut() = true;
            let value = Some(viewmodel::chart::judgment_value_for_y(y));
            if !state::apply_judgment_field(&mut preview.judgment, field.as_str(), value) {
                return;
            }
            let format = config.borrow().number_format;
            push_live_preview(&ui, &preview, format);
        });
    }

    // Drag commit (pointer-up): persist the final value ONCE via the SAME rail as the exact-value
    // field (one source of truth), then re-read + full re-push; clear the drag cache. A refused write
    // (read-only / save failure) surfaces a neutral notice and the preview is reconciled to disk.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        let drag_study = Rc::clone(&drag_study);
        let drag_moved = Rc::clone(&drag_moved);
        ui.global::<Studies>().on_judgment_commit(move |field, y| {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            *drag_study.borrow_mut() = None;
            let moved = std::mem::replace(&mut *drag_moved.borrow_mut(), false);
            let Some(id_text) = current_study.borrow().clone() else {
                return;
            };
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            let format = config.borrow().number_format;
            // A pointer-up with no movement is a click, not a drag — it must NOT rewrite the
            // forecast (review P2). No `moved` ran, so the on-screen preview still equals the saved
            // study; there is nothing to persist and nothing to reconcile.
            if !moved {
                return;
            }
            let value = Some(viewmodel::chart::judgment_value_for_y(y));
            let result = journal_state
                .borrow_mut()
                .set_judgment_field(id, field.as_str(), value);
            match result {
                Ok(()) => studies.set_notice(SharedString::new()),
                // The write was refused — surface the notice AND reconcile the (un-saved) preview
                // back to the saved study below, so no phantom line is left on screen (review P3).
                Err(message) => studies.set_notice(message.into()),
            }
            // Re-read + re-push from disk: on success this confirms the saved value; on failure it
            // reverts the live preview to what is actually persisted.
            if let Some(study) = journal_state.borrow().get_study(id) {
                push_form(&ui, &journal_state.borrow(), &study, format);
            }
        });
    }

    // Drag cancel (pointer-event cancel): the gesture was abandoned — revert the live preview to the
    // saved study WITHOUT persisting (review P4). The Slint side clears `judgment-dragging`.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        let drag_study = Rc::clone(&drag_study);
        let drag_moved = Rc::clone(&drag_moved);
        ui.global::<Studies>().on_judgment_cancel(move || {
            let ui = ui_weak.unwrap();
            *drag_study.borrow_mut() = None;
            *drag_moved.borrow_mut() = false;
            let Some(id_text) = current_study.borrow().clone() else {
                return;
            };
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            let format = config.borrow().number_format;
            if let Some(study) = journal_state.borrow().get_study(id) {
                push_form(&ui, &journal_state.borrow(), &study, format);
            }
        });
    }

    // ── Story 2.9 — undo / redo (snapshot stack). Restore the prior/next whole study and re-render
    //    the coherent frame; a no-op when the stack is empty. A refused write surfaces a neutral
    //    notice (the history is preserved, never silently lost). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>().on_undo(move || {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            // Undo/redo is disabled while the scenario-compare overlay is open (review P2) — otherwise
            // it would mutate the study behind the overlay and leave the comparison's baseline stale.
            if studies.get_scenario_compare().visible {
                return;
            }
            let Some(id_text) = current_study.borrow().clone() else {
                return;
            };
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            let format = config.borrow().number_format;
            match journal_state.borrow_mut().undo(id) {
                Ok(true) => {
                    studies.set_notice(SharedString::new());
                    if let Some(study) = journal_state.borrow().get_study(id) {
                        push_form(&ui, &journal_state.borrow(), &study, format);
                    }
                }
                Ok(false) => {} // nothing to undo
                Err(message) => studies.set_notice(message.into()),
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>().on_redo(move || {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            if studies.get_scenario_compare().visible {
                return; // disabled while the scenario-compare overlay is open (review P2)
            }
            let Some(id_text) = current_study.borrow().clone() else {
                return;
            };
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            let format = config.borrow().number_format;
            match journal_state.borrow_mut().redo(id) {
                Ok(true) => {
                    studies.set_notice(SharedString::new());
                    if let Some(study) = journal_state.borrow().get_study(id) {
                        push_form(&ui, &journal_state.borrow(), &study, format);
                    }
                }
                Ok(false) => {}
                Err(message) => studies.set_notice(message.into()),
            }
        });
    }

    // ── Story 2.9 — scenario compare (Phase-1, one alternate). Open caches the saved study and seeds
    //    the alternate = current; set-alternate recomputes the alternate's outcome from a NON-persisted
    //    clone; close discards it (the saved judgment is never overwritten — FR32). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        let compare_study = Rc::clone(&compare_study);
        ui.global::<Studies>().on_open_compare(move || {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            let Some(id_text) = current_study.borrow().clone() else {
                return;
            };
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            let Some(study) = journal_state.borrow().get_study(id) else {
                return;
            };
            let format = config.borrow().number_format;
            // Seed the alternate input from the current est-high-EPS (the alternate starts == current).
            let seed = viewmodel::engine::judgment_fields(&study, format).est_high_eps;
            studies.set_scenario_compare(viewmodel::engine::scenario_compare(
                &study,
                &study,
                seed.as_str(),
                format,
            ));
            *compare_study.borrow_mut() = Some(study);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let compare_study = Rc::clone(&compare_study);
        ui.global::<Studies>().on_set_alternate(move |text| {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            let Some(current) = compare_study.borrow().clone() else {
                return;
            };
            let format = config.borrow().number_format;
            let value = viewmodel::format::parse_amount(&text, format);
            let mut alternate = current.clone();
            // The alternate placement is the user's typed est-high-EPS (the §4-forecast driver).
            state::apply_judgment_field(&mut alternate.judgment, "est_high_eps", value);
            studies.set_scenario_compare(viewmodel::engine::scenario_compare(
                &current,
                &alternate,
                text.as_str(),
                format,
            ));
        });
    }
    {
        let ui_weak = ui.as_weak();
        let compare_study = Rc::clone(&compare_study);
        ui.global::<Studies>().on_close_compare(move || {
            let ui = ui_weak.unwrap();
            *compare_study.borrow_mut() = None;
            // Discard the alternate; the saved judgment was never touched.
            ui.global::<Studies>()
                .set_scenario_compare(ScenarioCompareState::default());
        });
    }

    // Select the §4 forecast-low option (a judgment edit → recompute).
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>()
            .on_set_forecast_low_option(move |key| {
                let ui = ui_weak.unwrap();
                let studies = ui.global::<Studies>();
                let Some(id_text) = current_study.borrow().clone() else {
                    return;
                };
                let Ok(id) = Uuid::parse_str(&id_text) else {
                    return;
                };
                let Some(option) = viewmodel::engine::forecast_low_option_from_key(key.as_str())
                else {
                    return;
                };
                let format = config.borrow().number_format;
                let result = journal_state
                    .borrow_mut()
                    .set_forecast_low_option(id, option);
                match result {
                    Ok(()) => {
                        studies.set_notice(SharedString::new());
                        if let Some(study) = journal_state.borrow().get_study(id) {
                            push_form(&ui, &journal_state.borrow(), &study, format);
                        }
                    }
                    Err(message) => studies.set_notice(message.into()),
                }
            });
    }

    // Open the traceability surface for a result (v1: "verdict"): re-read the study, build the
    // coherent snapshot, and push the inputs → provenance → rule trace (no colour spent).
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>().on_open_traceability(move |_result| {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            let Some(id_text) = current_study.borrow().clone() else {
                return;
            };
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            let Some(study) = journal_state.borrow().get_study(id) else {
                return;
            };
            let format = config.borrow().number_format;
            // The single engine-call site (`state::snapshot_for`): re-read + normalize + one
            // `StudySnapshot::new`. A normalize failure surfaces neutrally, never `unwrap`.
            match journal_state.borrow().snapshot_for(id) {
                Ok(snapshot) => {
                    studies.set_trace(viewmodel::engine::verdict_trace(&study, &snapshot, format));
                }
                Err(message) => studies.set_notice(message.into()),
            }
        });
    }

    // Close the traceability surface.
    {
        let ui_weak = ui.as_weak();
        ui.global::<Studies>().on_close_traceability(move || {
            let ui = ui_weak.unwrap();
            ui.global::<Studies>().set_trace(TraceState::default());
        });
    }

    // Settings intents: apply live (no restart), mirror into Prefs, persist on change.
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let path = config_path.clone();
        ui.global::<Prefs>().on_theme_selected(move |value| {
            let Some(theme) = Theme::parse(&value) else {
                return;
            };
            let ui = ui_weak.unwrap();
            theme::apply(&ui, theme);
            ui.global::<Prefs>().set_dark_theme(theme == Theme::Dark);
            config.borrow_mut().theme = theme;
            persist(path.as_ref(), &config.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let path = config_path.clone();
        ui.global::<Prefs>().on_label_set_selected(move |value| {
            let Some(set) = LabelSet::parse(&value) else {
                return;
            };
            let ui = ui_weak.unwrap();
            labels::apply(&ui, set);
            ui.global::<Prefs>().set_label_set(set.as_str().into());
            config.borrow_mut().label_set = set;
            persist(path.as_ref(), &config.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let path = config_path.clone();
        ui.global::<Prefs>()
            .on_number_format_selected(move |value| {
                let Some(format) = NumberFormat::parse(&value) else {
                    return;
                };
                let ui = ui_weak.unwrap();
                push_samples(&ui, format);
                ui.global::<Prefs>()
                    .set_number_format(format.as_str().into());
                config.borrow_mut().number_format = format;
                persist(path.as_ref(), &config.borrow());
            });
    }
    // ── Story 3.2 — provider selection + key management (FR25/FR63) ──
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let path = config_path.clone();
        ui.global::<Prefs>().on_provider_selected(move |value| {
            let Some(choice) = ProviderChoice::parse(&value) else {
                return;
            };
            let ui = ui_weak.unwrap();
            config.borrow_mut().preferred_provider = choice;
            persist(path.as_ref(), &config.borrow());
            mirror_provider_prefs(&ui, choice);
            // Drop any prior save/test verdict — it referred to the previous provider (F3).
            ui.global::<Prefs>().set_provider_status("".into());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        ui.global::<Prefs>().on_key_saved(move |key| {
            let ui = ui_weak.unwrap();
            let prefs = ui.global::<Prefs>();
            let provider = config.borrow().preferred_provider;
            // A blank field is a no-op (mirror the `set_rationale` empty-is-nothing discipline).
            if key.trim().is_empty() {
                return;
            }
            match keychain::set_key(provider, key.trim()) {
                Ok(()) => {
                    prefs.set_key_configured(true);
                    prefs.set_provider_status(state::MSG_KEY_SAVED.into());
                }
                Err(_) => prefs.set_provider_status(state::MSG_KEYCHAIN_UNAVAILABLE.into()),
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        ui.global::<Prefs>().on_key_deleted(move || {
            let ui = ui_weak.unwrap();
            let prefs = ui.global::<Prefs>();
            let provider = config.borrow().preferred_provider;
            match keychain::delete_key(provider) {
                Ok(()) => {
                    prefs.set_key_configured(false);
                    prefs.set_provider_status(state::MSG_KEY_DELETED.into());
                }
                Err(_) => prefs.set_provider_status(state::MSG_KEYCHAIN_UNAVAILABLE.into()),
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let fetch_tx = fetch_tx.clone();
        ui.global::<Prefs>().on_key_tested(move || {
            let ui = ui_weak.unwrap();
            let prefs = ui.global::<Prefs>();
            let provider = config.borrow().preferred_provider;
            // A keyless provider has nothing to test.
            if !provider.requires_key() {
                prefs.set_provider_status(state::MSG_KEY_OK.into());
                return;
            }
            let api_key = resolve_provider_key(provider);
            if api_key.is_none() {
                prefs.set_provider_status(state::MSG_PROVIDER_NO_KEY.into());
                return;
            }
            // Off the UI thread: a minimal live fetch whose verdict returns via the outcome handler.
            prefs.set_provider_status(state::MSG_KEY_TESTING.into());
            if fetch_tx
                .send(fetch::WorkerJob::TestKey(fetch::TestKeyRequest { api_key }))
                .is_err()
            {
                prefs.set_provider_status(
                    state::MSG_PROVIDER_FAILED
                        .replace("{cause}", "le service de récupération est indisponible")
                        .into(),
                );
            }
        });
    }

    // show() → re-apply the size from inside the running event loop → run: before the window
    // is mapped winit has no scale factor yet and misapplies a physical size (observed on
    // X11/XWayland: width falls back to min-width, height is scaled as if logical), so the
    // restore set before show() is only best-effort. A short timer re-requests the persisted
    // size for ~300 ms — past the map + scale-factor races — then stops for good, so it never
    // fights the user or a tiling window manager. `window().size()` reads back the requested
    // size, not the mapped one, so there is nothing reliable to compare against; the fixed
    // tick count is deliberate.
    ui.show()?;
    let restore_timer = Rc::new(slint::Timer::default());
    {
        let timer = Rc::clone(&restore_timer);
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let attempts = std::cell::Cell::new(0u32);
        restore_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(50),
            move || {
                let ui = ui_weak.unwrap();
                let cfg = config.borrow();
                if cfg.maximized || attempts.get() >= 6 {
                    timer.stop();
                    return;
                }
                attempts.set(attempts.get() + 1);
                let (width, height) = cfg.sane_window_size();
                ui.window()
                    .set_size(slint::PhysicalSize::new(width, height));
            },
        );
    }
    slint::run_event_loop()?;
    restore_timer.stop();
    ui.hide()?;

    // Window geometry is captured at exit (size only while not maximized, so un-maximizing
    // after a relaunch restores the last floating size).
    {
        let mut cfg = config.borrow_mut();
        cfg.maximized = ui.window().is_maximized();
        if !cfg.maximized {
            let size = ui.window().size();
            cfg.window_width = size.width;
            cfg.window_height = size.height;
        }
    }
    persist(config_path.as_ref(), &config.borrow());
    Ok(())
}
