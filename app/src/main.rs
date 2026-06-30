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
/// Rebuild the watchlist surface from persistence (Story 4.1): rows ordered by position, each
/// resolving its optional study link to that study's ticker (the buy-zone source for Story 4.2).
fn refresh_watchlist(ui: &MainWindow, state: &JournalState) {
    let watchlist = ui.global::<Watchlist>();
    let by_id: std::collections::HashMap<Uuid, String> = state
        .list_studies()
        .into_iter()
        .map(|s| (s.id, s.security_ticker))
        .collect();
    let items = state.list_watch_items();
    let mut in_buy_zone_count = 0i32;
    let rows: Vec<WatchRow> = items
        .iter()
        .map(|w| {
            // Story 4.2: a linked study whose current price is in its §4 buy zone flags a neutral
            // alert (one `build_snapshot` per linked study; unlinked entries are never in a zone).
            let in_buy_zone = w
                .study_id
                .and_then(|sid| state.get_study(sid))
                .is_some_and(|study| viewmodel::engine::study_in_buy_zone(&study));
            if in_buy_zone {
                in_buy_zone_count += 1;
            }
            WatchRow {
                id: w.id.to_string().into(),
                ticker: w.security_ticker.clone().into(),
                // `linked` is authoritative (the cell carries a study_id); `study_link` is the
                // resolved ticker for display (may be "" if the linked study no longer resolves).
                linked: w.study_id.is_some(),
                study_link: w
                    .study_id
                    .and_then(|sid| by_id.get(&sid))
                    .cloned()
                    .unwrap_or_default()
                    .into(),
                in_buy_zone,
            }
        })
        .collect();
    watchlist.set_count(items.len() as i32);
    watchlist.set_in_buy_zone_count(in_buy_zone_count);
    watchlist.set_rows(ModelRc::new(VecModel::from(rows)));
    watchlist.set_read_only(state.is_read_only());
}

/// Surface a watchlist write's outcome (neutral notice on refusal) and re-render the list.
fn apply_watch_result(ui: &MainWindow, state: &JournalState, result: Result<(), String>) {
    let watchlist = ui.global::<Watchlist>();
    match result {
        Ok(()) => watchlist.set_notice(SharedString::new()),
        Err(message) => watchlist.set_notice(message.into()),
    }
    refresh_watchlist(ui, state);
}

/// Transient (NOT persisted) per-ticker price-refresh freshness for the holdings register (Story
/// 4.4, FR40): the outcome of the last manual refresh. Keyed by **upper-cased** ticker so it joins
/// the case-insensitive `study_id_for_ticker` match; rebuilt each session (display-time only — no
/// schema change). `as_of` is the display stamp of the last *successful* refresh.
#[derive(Clone, Default)]
struct HoldingFreshness {
    stale: bool,
    as_of: Option<String>,
}
type HoldingFreshnessMap = std::collections::HashMap<String, HoldingFreshness>;

/// Flag a holding's transient freshness `stale` (Story 4.4, AC4) after a failed / no-data refresh,
/// **preserving** the last successful `as_of` so the row still states when it was last fresh while
/// keeping its last-known zone visibly marked stale — never a fresh-looking wrong zone.
fn mark_holding_stale(freshness: &Rc<RefCell<HoldingFreshnessMap>>, key: &str) {
    let prev = freshness.borrow().get(key).and_then(|f| f.as_of.clone());
    freshness.borrow_mut().insert(
        key.to_string(),
        HoldingFreshness {
            stale: true,
            as_of: prev,
        },
    );
}

/// A compact display form of an RFC3339 timestamp for the holdings freshness caption (Story 4.4):
/// `YYYY-MM-DD HH:MM` (drop seconds + zone; the journal stores the full RFC3339 string).
fn display_timestamp(ts: &steadyinvest_contract::Timestamp) -> String {
    let s = &ts.0;
    s.get(..16).unwrap_or(s).replacen('T', " ", 1)
}

/// Drop transient freshness entries for tickers no longer held by ANY holding (issue #51). Called
/// after every holdings mutation: a removed (or ticker-edited-away) holding's stale `as_of` must not
/// resurface if that ticker is later re-added, and the map must not grow unbounded. Keyed by
/// upper-cased ticker — the same join as `study_id_for_ticker`. A ticker still held by a *sibling*
/// holding legitimately keeps its entry (the freshness is the ticker's, shared across its rows).
fn retain_held_freshness(freshness: &Rc<RefCell<HoldingFreshnessMap>>, state: &JournalState) {
    let held: std::collections::HashSet<String> = state
        .list_holdings()
        .iter()
        .map(|h| h.security_ticker.to_uppercase())
        .collect();
    freshness
        .borrow_mut()
        .retain(|ticker, _| held.contains(ticker));
}

/// Rebuild the holdings register from the journal (Story 4.3, FR36). Reference-currency labelling is
/// set separately (from app-config, on startup + on change) — a holdings mutation doesn't touch it.
/// Story 4.4 (FR40): each row also carries its auto-matched study's §4 zone (neutral key) + present
/// price + transient freshness, so the register shows Achat/Neutre/Vente + à jour/périmé per holding.
/// Write a study's export envelope to a file (Story 5.2, FR59) and return its path. The file lands in
/// an `exports/` folder under the OS data dir — **never** beside the live journal DB (ADD7/8
/// sync-safety; the native picker + a user-chosen sync target is Story 5.5). Named by the study id
/// (stable, unique). `app` owns the file I/O — `contract` only produced the string.
fn write_study_export(id: Uuid, json: &str) -> std::io::Result<std::path::PathBuf> {
    let dir = directories::ProjectDirs::from("", "", "steadyinvest")
        .map(|d| d.data_dir().join("exports"))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no OS data directory"))?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("study-{id}.json"));
    std::fs::write(&path, json)?;
    Ok(path)
}

/// Write the whole-journal export envelope to a file (Story 5.3, FR60) and return its path. Like the
/// single-study export, it lands in the `exports/` folder under the OS data dir — **never** beside the
/// live journal DB (ADD7/8 sync-safety; the native picker + a user-chosen sync target is Story 5.5).
/// Named by the journal id (stable, unique). `app` owns the file I/O.
fn write_journal_export(journal_id: Uuid, json: &str) -> std::io::Result<std::path::PathBuf> {
    let dir = directories::ProjectDirs::from("", "", "steadyinvest")
        .map(|d| d.data_dir().join("exports"))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no OS data directory"))?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("journal-{journal_id}.json"));
    std::fs::write(&path, json)?;
    Ok(path)
}

fn refresh_holdings(
    ui: &MainWindow,
    state: &JournalState,
    freshness: &HoldingFreshnessMap,
    dismissed: &std::collections::HashSet<String>,
    format: NumberFormat,
) {
    use steadyinvest_core::rounding::DisplayField;
    let holdings = ui.global::<Holdings>();
    let items = state.list_holdings();
    let rows: Vec<HoldingRow> = items
        .iter()
        .map(|h| {
            // Auto-match the holding to the most-recent saved study of the SAME ticker (the watchlist
            // rule); `None` → a neutral "no linked study" row, never an error.
            let study = state
                .study_id_for_ticker(&h.security_ticker)
                .and_then(|sid| state.get_study(sid));
            let f = freshness
                .get(&h.security_ticker.to_uppercase())
                .cloned()
                .unwrap_or_default();
            let (zone, current_price, study_link) = match &study {
                Some(s) => (
                    viewmodel::engine::zone_key(viewmodel::engine::study_zone(s)).to_string(),
                    s.judgment
                        .current_price
                        .map(|p| {
                            viewmodel::format::format_scaled(
                                p.as_decimal(),
                                DisplayField::Price,
                                format,
                            )
                        })
                        .unwrap_or_default(),
                    s.security_ticker.clone(),
                ),
                None => (String::new(), String::new(), String::new()),
            };
            // Story 4.5 (FR42): the trailing stop. `stop_breached` is a neutral fact — current price
            // ≤ the ratcheted level — computed only when both are known (no action; that's Story 4.7).
            let stop_level_dec = h
                .trailing_stop_level
                .as_deref()
                .and_then(|s| rust_decimal::Decimal::from_str_exact(s).ok());
            let current_price_dec = study
                .as_ref()
                .and_then(|s| s.judgment.current_price)
                .map(|m| m.as_decimal());
            let stop_breached = match (stop_level_dec, current_price_dec) {
                (Some(level), Some(price)) => steadyinvest_core::risk::stop_breached(level, price),
                _ => false,
            };
            let stop_level_display = stop_level_dec
                .map(|l| viewmodel::format::format_scaled(l, DisplayField::Price, format))
                .unwrap_or_default();
            // The margin above the stop (AC4 neutral fact) — only when a stop + price are known and
            // the price is NOT at/below the stop (a breach shows "◆ sous le stop" instead).
            let stop_distance = match (stop_level_dec, current_price_dec) {
                (Some(level), Some(price)) if price > level => {
                    viewmodel::format::format_scaled(price - level, DisplayField::Price, format)
                }
                _ => String::new(),
            };
            // Story 4.7 (FR46/FR47): the neutral trigger — the stop takes priority over the Sell
            // zone. The kind is computed in `core::risk` from the very same `stop_breached` + zone the
            // row already carries (no second source of truth); the per-row action panel is geofenced
            // to a still-shown (not dismissed) trigger.
            let trigger_kind =
                match steadyinvest_core::risk::trigger_state(stop_breached, zone == "sell") {
                    Some(steadyinvest_core::risk::TriggerKind::Stop) => "stop",
                    Some(steadyinvest_core::risk::TriggerKind::Sell) => "sell",
                    None => "",
                };
            let id_text = h.id.to_string();
            let dismissed = dismissed.contains(&id_text);
            HoldingRow {
                id: id_text.into(),
                ticker: h.security_ticker.clone().into(),
                quantity: h.quantity.clone().into(),
                purchase_price: h.purchase_price.clone().into(),
                linked: study.is_some(),
                study_link: study_link.into(),
                zone: zone.into(),
                current_price: current_price.into(),
                stale: f.stale,
                as_of: f.as_of.unwrap_or_default().into(),
                has_stop: h.trailing_stop_pct.is_some(),
                stop_pct: h.trailing_stop_pct.clone().unwrap_or_default().into(),
                stop_level: stop_level_display.into(),
                stop_breached,
                stop_distance: stop_distance.into(),
                trigger_kind: trigger_kind.into(),
                dismissed,
            }
        })
        .collect();
    holdings.set_holding_count(items.len() as i32);
    holdings.set_rows(ModelRc::new(VecModel::from(rows)));
    holdings.set_read_only(state.is_read_only());

    // Story 4.6 (FR43): the portfolio capital-at-risk figure + its share of invested capital,
    // recomputed with the register (so a price refresh → stop ratchet → CaR all flow through here).
    let (car, invested) = state.portfolio_capital_at_risk();
    holdings.set_capital_at_risk(
        viewmodel::format::format_scaled(car, DisplayField::Price, format).into(),
    );
    let pct = if invested > rust_decimal::Decimal::ZERO {
        // Format the percent through the same locale-aware path as the figure (Percent = 1 dp), so a
        // comma-preset display doesn't mix separators (e.g. "1 234,57 CHF (12,5 %)", not "(12.5 %)").
        viewmodel::format::format_scaled(
            car / invested * rust_decimal::Decimal::from(100),
            DisplayField::Percent,
            format,
        )
    } else {
        String::new()
    };
    holdings.set_capital_at_risk_pct(pct.into());
}

/// Surface a holdings write's outcome (neutral notice on refusal) and re-render the register.
fn apply_holdings_result(
    ui: &MainWindow,
    state: &JournalState,
    result: Result<(), String>,
    freshness: &HoldingFreshnessMap,
    dismissed: &std::collections::HashSet<String>,
    format: NumberFormat,
) {
    let holdings = ui.global::<Holdings>();
    match result {
        Ok(()) => holdings.set_notice(SharedString::new()),
        Err(message) => holdings.set_notice(message.into()),
    }
    refresh_holdings(ui, state, freshness, dismissed, format);
}

/// Link a watchlist entry to a saved study of the SAME ticker (the most recent), or a neutral
/// "no study for this ticker" notice if none exists (Story 4.1 — an explicit picker is a later
/// refinement; auto-match by ticker covers the common case).
fn link_watch_to_same_ticker_study(state: &mut JournalState, id: Uuid) -> Result<(), String> {
    let Some(item) = state.list_watch_items().into_iter().find(|w| w.id == id) else {
        return Ok(()); // entry gone — nothing to link
    };
    match state.study_id_for_ticker(&item.security_ticker) {
        Some(sid) => state.update_watch_item(id, &item.security_ticker, Some(sid)),
        None => Err(state::MSG_WATCH_NO_STUDY.to_string()),
    }
}

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

    // Story 4.4 — transient per-ticker holdings price-refresh freshness (NOT persisted; display-time
    // only). Populated by the off-thread refresh outcomes; read when rebuilding the register.
    let holding_freshness: Rc<RefCell<HoldingFreshnessMap>> =
        Rc::new(RefCell::new(HoldingFreshnessMap::new()));

    // Story 4.7 — holding ids whose neutral-trigger action panel the user has dismissed this session
    // (transient; never persisted — a dismissed trigger re-appears next launch if still firing).
    let holding_dismissed: Rc<RefCell<std::collections::HashSet<String>>> =
        Rc::new(RefCell::new(std::collections::HashSet::new()));
    // Issue #52 — outstanding holdings-refresh jobs in flight. Set to the enqueued count when a
    // refresh starts; each outcome decrements it; the button is disabled (`Holdings.refreshing`)
    // until it returns to zero, so a double-click can't enqueue duplicate jobs.
    let refresh_pending: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));

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
        // Story 4.3 (FR63): mirror the reference currency (validated) into Prefs + the holdings
        // register, so the picker reflects it and amounts are labelled before the window shows.
        let currency = cfg.reference_currency_or_default();
        prefs.set_reference_currency(currency.clone().into());
        ui.global::<Holdings>()
            .set_reference_currency(currency.into());
        // Story 4.5 (FR42): mirror the default trailing-stop % (validated; "" when none) so the
        // set-stop control pre-fills it.
        prefs.set_default_trailing_stop_pct(
            cfg.default_trailing_stop_pct_or_none()
                .unwrap_or_default()
                .into(),
        );

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
        refresh_watchlist(&ui, &journal_state.borrow());
        refresh_holdings(
            &ui,
            &journal_state.borrow(),
            &holding_freshness.borrow(),
            &holding_dismissed.borrow(),
            config.borrow().number_format,
        );
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
        let holding_freshness = Rc::clone(&holding_freshness);
        let holding_dismissed = Rc::clone(&holding_dismissed);
        let refresh_pending = Rc::clone(&refresh_pending);
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
                    // Re-render the open study (the stale murmur + degraded verdict show here) and
                    // refresh the dashboard — shared by the success, empty-payload, and failure arms.
                    let render_open = || {
                        let still_open = current_study
                            .borrow()
                            .as_deref()
                            .and_then(|s| Uuid::parse_str(s).ok())
                            == Some(outcome.study_id);
                        if still_open {
                            if let Some(study) = journal_state.borrow().get_study(outcome.study_id)
                            {
                                let format = config.borrow().number_format;
                                push_form(&ui, &journal_state.borrow(), &study, format);
                            }
                        }
                        refresh_studies(&ui, &journal_state.borrow());
                    };
                    match outcome.result {
                        // Story 3.5 / #46: a transport-success that returned ZERO usable years is
                        // "no data", NOT "no change" — report it honestly and flag last-known
                        // provider data stale (never apply an empty refresh as if nothing changed).
                        Ok(fetched) if fetched.canonical.years.is_empty() => {
                            studies.set_notice(state::MSG_PROVIDER_NO_DATA.into());
                            let _ = journal_state
                                .borrow_mut()
                                .mark_provider_stale(outcome.study_id);
                            render_open();
                        }
                        Ok(fetched) => {
                            let applied = journal_state
                                .borrow_mut()
                                .apply_provider_refresh(outcome.study_id, &fetched);
                            match applied {
                                Ok(report) => {
                                    // Name the recompute cause (FR29) and, after an annual update
                                    // (Story 3.6), the re-validation scope ("N à revérifier").
                                    studies.set_notice(state::refresh_summary(report).into());
                                    render_open();
                                }
                                Err(message) => studies.set_notice(message.into()),
                            }
                        }
                        // Story 3.5 (FR23/FR24/NFR-R1): name the cause, RETAIN last-known values, and
                        // flag the open study's provider cells stale (degrading the verdict). Never
                        // clear data; the user keeps working offline and retries later.
                        Err(error) => {
                            studies.set_notice(state::provider_failure_notice(&error).into());
                            let _ = journal_state
                                .borrow_mut()
                                .mark_provider_stale(outcome.study_id);
                            render_open();
                        }
                    }
                }
                fetch::WorkerOutcome::HoldingFetch(outcome) => {
                    // Story 4.4 (FR40): a holdings price-refresh result → the holdings surface (NOT
                    // the study screen). Success fills `current_price` (the §4 zone recomputes) +
                    // stamps a fresh `as_of`; a failure / no-data flags the ticker `périmé`, keeping
                    // its last-known zone visibly stale (AC4) — never a fresh-looking wrong zone.
                    let holdings = ui.global::<Holdings>();
                    let format = config.borrow().number_format;
                    let key = outcome.ticker.to_uppercase();
                    match outcome.result {
                        // The price arrived: fill `current_price` (price-only — never the yearly
                        // cells, issue #50) so the §4 zone recomputes, and stamp a fresh `as_of`.
                        Ok(Some(price)) => {
                            match journal_state
                                .borrow_mut()
                                .apply_holding_price(outcome.study_id, price)
                            {
                                Ok(()) => {
                                    // Story 4.5 (FR42): ratchet every same-ticker holding's stop level
                                    // up against the fresh price (a falling price writes nothing).
                                    let _ = journal_state
                                        .borrow_mut()
                                        .ratchet_trailing_stops_for_study(outcome.study_id, price);
                                    let now = display_timestamp(&journal_state.borrow().now());
                                    holding_freshness.borrow_mut().insert(
                                        key,
                                        HoldingFreshness {
                                            stale: false,
                                            as_of: Some(now),
                                        },
                                    );
                                    // Clear ONLY the in-progress banner — don't wipe a sibling
                                    // ticker's failure notice that resolved earlier. (Review F4.)
                                    if holdings.get_notice().as_str()
                                        == state::MSG_HOLDINGS_REFRESHING
                                    {
                                        holdings.set_notice(SharedString::new());
                                    }
                                }
                                // The study vanished / went read-only between enqueue and outcome:
                                // surface the cause AND flag the ticker stale, like the other failure
                                // arms (don't leave it falsely fresh). (Review F3.)
                                Err(message) => {
                                    holdings.set_notice(message.into());
                                    mark_holding_stale(&holding_freshness, &key);
                                }
                            }
                        }
                        // Transport-success but the provider exposed no current close → "no data":
                        // flag `périmé`, keep the last-known zone (AC4); never stamp "à jour" when the
                        // price the user asked to refresh did not come back.
                        Ok(None) => {
                            holdings.set_notice(state::MSG_PROVIDER_NO_DATA.into());
                            mark_holding_stale(&holding_freshness, &key);
                        }
                        Err(error) => {
                            holdings.set_notice(state::provider_failure_notice(&error).into());
                            mark_holding_stale(&holding_freshness, &key);
                        }
                    }
                    // Re-render the open study too (a holding may BE the open study — its §4 zone bar
                    // must reflect the just-filled current_price), then rebuild the register.
                    if current_study
                        .borrow()
                        .as_deref()
                        .and_then(|s| Uuid::parse_str(s).ok())
                        == Some(outcome.study_id)
                    {
                        if let Some(study) = journal_state.borrow().get_study(outcome.study_id) {
                            push_form(&ui, &journal_state.borrow(), &study, format);
                        }
                    }
                    refresh_holdings(
                        &ui,
                        &journal_state.borrow(),
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                    // One job resolved — clear the in-flight latch when the batch is fully drained,
                    // re-enabling the refresh button (issue #52).
                    {
                        let mut pending = refresh_pending.borrow_mut();
                        *pending = pending.saturating_sub(1);
                        if *pending == 0 {
                            ui.global::<Holdings>().set_refreshing(false);
                        }
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

    // ── Story 5.2 (FR59) — export / import a single study as a portable file. The envelope is the
    // serialized data contract + schema_version + integrity hash (NOT a raw .db); `contract` owns the
    // envelope, `app` owns the file I/O. Path-based for now — the native picker is Story 5.5. ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Studies>().on_export_study(move |id| {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            let Ok(uuid) = Uuid::parse_str(&id) else {
                return;
            };
            let notice = match journal_state.borrow().export_study(uuid) {
                Ok(json) => match write_study_export(uuid, &json) {
                    Ok(path) => format!("{} {}", state::MSG_STUDY_EXPORTED, path.display()),
                    Err(e) => format!("{} {e}", state::MSG_SAVE_FAILED),
                },
                Err(message) => message,
            };
            studies.set_notice(notice.into());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Studies>().on_import_study(move |path| {
            let ui = ui_weak.unwrap();
            let notice = match std::fs::read_to_string(path.as_str()) {
                Ok(json) => match journal_state.borrow_mut().import_study(&json) {
                    // Surface an overwrite of a pre-existing study distinctly from a fresh import.
                    Ok((_id, true)) => state::MSG_STUDY_UPDATED.to_string(),
                    Ok((_id, false)) => state::MSG_STUDY_IMPORTED.to_string(),
                    Err(message) => message,
                },
                // An unreadable path is the malformed/unreadable case — a neutral refusal, no panic.
                Err(_) => state::MSG_IMPORT_MALFORMED.to_string(),
            };
            ui.global::<Studies>().set_notice(notice.into());
            refresh_studies(&ui, &journal_state.borrow());
        });
    }

    // ── Story 5.3 (FR60) — export / import the WHOLE journal as a portable file. Scales the 5.2
    // envelope to every entity + the (journal_id, version, hash) identity tuple; import verifies and
    // applies atomically (never partially). Path-based for now — the native picker is Story 5.5. The
    // actions live in Réglages (the Prefs global). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Prefs>().on_export_journal(move || {
            let ui = ui_weak.unwrap();
            let state = journal_state.borrow();
            let notice = match state.export_journal() {
                Ok(json) => match state.journal_id() {
                    Some(jid) => match write_journal_export(jid, &json) {
                        Ok(path) => format!("{} {}", state::MSG_JOURNAL_EXPORTED, path.display()),
                        Err(e) => format!("{} {e}", state::MSG_SAVE_FAILED),
                    },
                    None => state::MSG_NO_JOURNAL.to_string(),
                },
                Err(message) => message,
            };
            ui.global::<Prefs>().set_journal_status(notice.into());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let holding_freshness = Rc::clone(&holding_freshness);
        let holding_dismissed = Rc::clone(&holding_dismissed);
        ui.global::<Prefs>().on_import_journal(move |path| {
            let ui = ui_weak.unwrap();
            let notice = match std::fs::read_to_string(path.as_str()) {
                Ok(json) => match journal_state.borrow_mut().import_journal(&json) {
                    Ok(summary) => state::journal_imported_message(&summary),
                    Err(message) => message,
                },
                // An unreadable path is the malformed/unreadable case — a neutral refusal, no panic.
                Err(_) => state::MSG_IMPORT_MALFORMED.to_string(),
            };
            ui.global::<Prefs>().set_journal_status(notice.into());
            // A whole-journal import can touch every surface — re-render them all (dashboard,
            // watchlist, portfolio). Prune any stale per-holding freshness for tickers no longer held.
            let state = journal_state.borrow();
            let format = config.borrow().number_format;
            retain_held_freshness(&holding_freshness, &state);
            refresh_studies(&ui, &state);
            refresh_watchlist(&ui, &state);
            refresh_holdings(
                &ui,
                &state,
                &holding_freshness.borrow(),
                &holding_dismissed.borrow(),
                format,
            );
        });
    }

    // ── Story 5.4 (FR61) — backup / restore the raw .db. Create a self-contained .db backup; validate
    // a candidate backup (integrity + schema-version + identity) BEFORE any overwrite, surface its
    // (journal_id, version) + a stale/foreign warning, and apply only on explicit confirm (never
    // silently). Path-based for now — the native picker is Story 5.5. ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Prefs>().on_create_backup(move || {
            let ui = ui_weak.unwrap();
            let notice = match journal_state.borrow().create_backup() {
                Ok(path) => format!("{} {}", state::MSG_BACKUP_CREATED, path.display()),
                Err(message) => message,
            };
            ui.global::<Prefs>().set_restore_status(notice.into());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Prefs>().on_request_restore(move |path| {
            let ui = ui_weak.unwrap();
            let prefs = ui.global::<Prefs>();
            match journal_state.borrow_mut().request_restore(path.as_str()) {
                // A confirmable restore is parked — reveal the confirm banner with the identity/warning.
                Ok(assessment) => {
                    prefs.set_restore_confirm(state::restore_confirm_message(&assessment).into());
                    prefs.set_restore_status("".into());
                }
                // A hard refusal — show the cause, no banner.
                Err(message) => {
                    prefs.set_restore_confirm("".into());
                    prefs.set_restore_status(message.into());
                }
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let holding_freshness = Rc::clone(&holding_freshness);
        let holding_dismissed = Rc::clone(&holding_dismissed);
        let current_study = Rc::clone(&current_study);
        ui.global::<Prefs>().on_confirm_restore(move || {
            let ui = ui_weak.unwrap();
            let result = journal_state.borrow_mut().confirm_restore();
            let prefs = ui.global::<Prefs>();
            prefs.set_restore_confirm("".into());
            // A successful restore replaces the whole journal — close any open study editor first so a
            // stale in-memory form can't be saved back into the restored journal (an old study_id would
            // otherwise be written into the new journal).
            if result.is_ok() {
                *current_study.borrow_mut() = None;
                ui.global::<Studies>().set_study_open(false);
            }
            let notice = match result {
                Ok(()) => state::MSG_RESTORE_DONE.to_string(),
                Err(message) => message,
            };
            prefs.set_restore_status(notice.into());
            // The whole journal changed — re-render every surface.
            let state = journal_state.borrow();
            let format = config.borrow().number_format;
            retain_held_freshness(&holding_freshness, &state);
            refresh_studies(&ui, &state);
            refresh_watchlist(&ui, &state);
            refresh_holdings(
                &ui,
                &state,
                &holding_freshness.borrow(),
                &holding_dismissed.borrow(),
                format,
            );
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Prefs>().on_cancel_restore(move || {
            let ui = ui_weak.unwrap();
            journal_state.borrow_mut().cancel_restore();
            ui.global::<Prefs>().set_restore_confirm("".into());
        });
    }

    // ── Watchlist intents (Story 4.1, FR34) ── add / remove / move / link / unlink, each persisted
    // then re-rendered with a neutral notice on refusal. The link callbacks attach/clear a
    // same-ticker saved study (its buy zone — the seam Story 4.2 reads).
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Watchlist>().on_add_watch(move |ticker| {
            let ui = ui_weak.unwrap();
            let result = journal_state.borrow_mut().add_watch_item(&ticker, None);
            apply_watch_result(&ui, &journal_state.borrow(), result);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Watchlist>().on_remove_watch(move |id| {
            let ui = ui_weak.unwrap();
            let Ok(id) = Uuid::parse_str(&id) else {
                return;
            };
            let result = journal_state.borrow_mut().delete_watch_item(id);
            apply_watch_result(&ui, &journal_state.borrow(), result);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Watchlist>().on_move_watch(move |id, up| {
            let ui = ui_weak.unwrap();
            let Ok(id) = Uuid::parse_str(&id) else {
                return;
            };
            let result = journal_state.borrow_mut().move_watch_item(id, up);
            apply_watch_result(&ui, &journal_state.borrow(), result);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Watchlist>().on_link_watch(move |id| {
            let ui = ui_weak.unwrap();
            let Ok(id) = Uuid::parse_str(&id) else {
                return;
            };
            let result = link_watch_to_same_ticker_study(&mut journal_state.borrow_mut(), id);
            apply_watch_result(&ui, &journal_state.borrow(), result);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        ui.global::<Watchlist>().on_unlink_watch(move |id| {
            let ui = ui_weak.unwrap();
            let Ok(id) = Uuid::parse_str(&id) else {
                return;
            };
            // Clear the link by re-saving the entry's ticker with no study.
            let ticker = journal_state
                .borrow()
                .list_watch_items()
                .into_iter()
                .find(|w| w.id == id)
                .map(|w| w.security_ticker);
            let result = match ticker {
                Some(t) => journal_state.borrow_mut().update_watch_item(id, &t, None),
                None => Ok(()),
            };
            apply_watch_result(&ui, &journal_state.borrow(), result);
        });
    }

    // ── Holdings intents (Story 4.3, FR36) ── add / edit / remove a holding, each validated +
    // persisted then re-rendered with a neutral notice on refusal (invalid number / empty symbol /
    // read-only / no journal). Amounts are in the global reference currency (no FX in Epic 4).
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let holding_freshness = Rc::clone(&holding_freshness);
        let holding_dismissed = Rc::clone(&holding_dismissed);
        ui.global::<Holdings>()
            .on_add_holding(move |ticker, quantity, price| {
                let ui = ui_weak.unwrap();
                let result = journal_state
                    .borrow_mut()
                    .add_holding(&ticker, &quantity, &price);
                let written = result.is_ok();
                let format = config.borrow().number_format;
                retain_held_freshness(&holding_freshness, &journal_state.borrow());
                apply_holdings_result(
                    &ui,
                    &journal_state.borrow(),
                    result,
                    &holding_freshness.borrow(),
                    &holding_dismissed.borrow(),
                    format,
                );
                // Report whether the holding was written so the UI keeps the user's input on refusal.
                written
            });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let holding_freshness = Rc::clone(&holding_freshness);
        let holding_dismissed = Rc::clone(&holding_dismissed);
        ui.global::<Holdings>()
            .on_edit_holding(move |id, ticker, quantity, price| {
                let ui = ui_weak.unwrap();
                let Ok(id) = Uuid::parse_str(&id) else {
                    return false;
                };
                let result = journal_state
                    .borrow_mut()
                    .update_holding(id, &ticker, &quantity, &price);
                let written = result.is_ok();
                let format = config.borrow().number_format;
                retain_held_freshness(&holding_freshness, &journal_state.borrow());
                apply_holdings_result(
                    &ui,
                    &journal_state.borrow(),
                    result,
                    &holding_freshness.borrow(),
                    &holding_dismissed.borrow(),
                    format,
                );
                written
            });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let holding_freshness = Rc::clone(&holding_freshness);
        let holding_dismissed = Rc::clone(&holding_dismissed);
        ui.global::<Holdings>().on_remove_holding(move |id| {
            let ui = ui_weak.unwrap();
            let Ok(id) = Uuid::parse_str(&id) else {
                return;
            };
            let result = journal_state.borrow_mut().delete_holding(id);
            // The holding is gone — drop any dismiss entry so the session set can't grow unbounded
            // (mirrors the sell path). `id.to_string()` is the same canonical key the rows use.
            holding_dismissed.borrow_mut().remove(&id.to_string());
            let format = config.borrow().number_format;
            retain_held_freshness(&holding_freshness, &journal_state.borrow());
            apply_holdings_result(
                &ui,
                &journal_state.borrow(),
                result,
                &holding_freshness.borrow(),
                &holding_dismissed.borrow(),
                format,
            );
        });
    }
    // ── Story 4.4 (FR40) — manual price refresh for every linked holding, off the UI thread. One
    // job per UNIQUE linked ticker (reusing the Epic-3 worker); holdings with no matching study are
    // skipped. Only ever user-initiated (FR65 — no background polling). Outcomes route to the
    // holdings surface via `WorkerOutcome::HoldingFetch` (the transient-freshness handler above). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let fetch_tx = fetch_tx.clone();
        let refresh_pending = Rc::clone(&refresh_pending);
        ui.global::<Holdings>().on_refresh_prices(move || {
            let ui = ui_weak.unwrap();
            let holdings = ui.global::<Holdings>();
            let jobs: Vec<(Uuid, String)> = {
                let state = journal_state.borrow();
                let mut seen = std::collections::HashSet::new();
                state
                    .list_holdings()
                    .into_iter()
                    .filter_map(|h| {
                        state
                            .study_id_for_ticker(&h.security_ticker)
                            .map(|sid| (sid, h.security_ticker))
                    })
                    .filter(|(_, ticker)| seen.insert(ticker.to_uppercase()))
                    .collect()
            };
            if jobs.is_empty() {
                holdings.set_notice(state::MSG_HOLDINGS_REFRESH_NONE.into());
                return;
            }
            let provider_choice = config.borrow().preferred_provider;
            let api_key = resolve_provider_key(provider_choice);
            if provider_choice.requires_key() && api_key.is_none() {
                holdings.set_notice(state::MSG_PROVIDER_NO_KEY.into());
                return;
            }
            // Count only jobs the worker actually accepted — if the worker is gone, don't latch
            // `refreshing` (which would disable the button for the rest of the session). (Issue #52.)
            let mut enqueued = 0usize;
            for (study_id, ticker) in jobs {
                if fetch_tx
                    .send(fetch::WorkerJob::RefreshHolding(fetch::FetchRequest {
                        study_id,
                        ticker,
                        api_key: api_key.clone(),
                    }))
                    .is_ok()
                {
                    enqueued += 1;
                }
            }
            if enqueued == 0 {
                return;
            }
            // Latch the in-flight state: the button is disabled while `refreshing` (no double-click
            // → no duplicate jobs). The outcome handler decrements the pending count and clears the
            // flag when the last job resolves. No race — outcomes are marshalled to THIS (UI) thread.
            *refresh_pending.borrow_mut() = enqueued;
            holdings.set_refreshing(true);
            holdings.set_notice(state::MSG_HOLDINGS_REFRESHING.into());
        });
    }
    // ── Story 4.5 (FR42) — set / clear a holding's trailing-stop percentage. ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let holding_freshness = Rc::clone(&holding_freshness);
        let holding_dismissed = Rc::clone(&holding_dismissed);
        ui.global::<Holdings>()
            .on_set_trailing_stop(move |id, pct| {
                let ui = ui_weak.unwrap();
                let Ok(id) = Uuid::parse_str(&id) else {
                    return;
                };
                let result = journal_state
                    .borrow_mut()
                    .set_holding_trailing_stop(id, &pct);
                let format = config.borrow().number_format;
                apply_holdings_result(
                    &ui,
                    &journal_state.borrow(),
                    result,
                    &holding_freshness.borrow(),
                    &holding_dismissed.borrow(),
                    format,
                );
            });
    }
    // ── Story 4.7 (FR46/FR47) — record a sell on a neutral trigger / dismiss a trigger's panel. The
    // app never auto-acts: it only persists a sell the user explicitly chose, or hides a trigger. ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let holding_freshness = Rc::clone(&holding_freshness);
        let holding_dismissed = Rc::clone(&holding_dismissed);
        ui.global::<Holdings>()
            .on_sell_holding(move |id, rationale| {
                let ui = ui_weak.unwrap();
                let Ok(uuid) = Uuid::parse_str(&id) else {
                    return;
                };
                let currency = config.borrow().reference_currency_or_default();
                let result = journal_state
                    .borrow_mut()
                    .sell_holding(uuid, &rationale, &currency);
                // The holding is gone on success — drop any stale dismiss entry so the id can't leak.
                if result.is_ok() {
                    holding_dismissed.borrow_mut().remove(id.as_str());
                }
                let format = config.borrow().number_format;
                retain_held_freshness(&holding_freshness, &journal_state.borrow());
                // A neutral confirmation on success; the guarded refusal notice otherwise.
                let result = result.map(|()| {
                    ui.global::<Holdings>()
                        .set_notice(state::MSG_HOLDING_SOLD.into());
                });
                if result.is_ok() {
                    refresh_holdings(
                        &ui,
                        &journal_state.borrow(),
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                } else {
                    apply_holdings_result(
                        &ui,
                        &journal_state.borrow(),
                        result,
                        &holding_freshness.borrow(),
                        &holding_dismissed.borrow(),
                        format,
                    );
                }
            });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let holding_freshness = Rc::clone(&holding_freshness);
        let holding_dismissed = Rc::clone(&holding_dismissed);
        ui.global::<Holdings>().on_dismiss_trigger(move |id| {
            let ui = ui_weak.unwrap();
            holding_dismissed.borrow_mut().insert(id.to_string());
            let format = config.borrow().number_format;
            refresh_holdings(
                &ui,
                &journal_state.borrow(),
                &holding_freshness.borrow(),
                &holding_dismissed.borrow(),
                format,
            );
        });
    }
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

    // Story 3.4 — resolve a pending provider divergence (FR22, AC4): accept the provider value, or
    // keep the manual value and dismiss the pending. Both mirror the `on_set_review` rail.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>()
            .on_accept_provider(move |year_index, field| {
                let ui = ui_weak.unwrap();
                let studies = ui.global::<Studies>();
                let Some(id_text) = current_study.borrow().clone() else {
                    return;
                };
                let Ok(id) = Uuid::parse_str(&id_text) else {
                    return;
                };
                let format = config.borrow().number_format;
                let result = journal_state.borrow_mut().accept_provider_value(
                    id,
                    year_index.max(0) as usize,
                    field.as_str(),
                );
                match result {
                    Ok(()) => {
                        studies.set_notice(SharedString::new());
                        // The divergence is resolved — hide the reveal + resolve controls (the
                        // reveal is set only on focus, so clear it explicitly here).
                        studies.set_active_pending(SharedString::new());
                        if let Some(study) = journal_state.borrow().get_study(id) {
                            push_form(&ui, &journal_state.borrow(), &study, format);
                        }
                    }
                    Err(message) => studies.set_notice(message.into()),
                }
            });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(&journal_state);
        let config = Rc::clone(&config);
        let current_study = Rc::clone(&current_study);
        ui.global::<Studies>()
            .on_keep_manual(move |year_index, field| {
                let ui = ui_weak.unwrap();
                let studies = ui.global::<Studies>();
                let Some(id_text) = current_study.borrow().clone() else {
                    return;
                };
                let Ok(id) = Uuid::parse_str(&id_text) else {
                    return;
                };
                let format = config.borrow().number_format;
                let result = journal_state.borrow_mut().keep_manual_value(
                    id,
                    year_index.max(0) as usize,
                    field.as_str(),
                );
                match result {
                    Ok(()) => {
                        studies.set_notice(SharedString::new());
                        // The divergence is dismissed — hide the reveal + resolve controls.
                        studies.set_active_pending(SharedString::new());
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
                    // A delete clears any watchlist soft link to this study (Story 4.1) — re-render
                    // the watchlist so a linked row drops its (now-cleared) study link.
                    refresh_watchlist(&ui, &journal_state.borrow());
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
    // ── Story 4.3 — reference currency (FR63) ── persist the chosen code and re-label the register.
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let path = config_path.clone();
        ui.global::<Prefs>()
            .on_reference_currency_selected(move |value| {
                // Validate the shape (3 uppercase letters); ignore a malformed pick rather than
                // persist garbage (mirrors the parse-guard of the other Prefs callbacks).
                if !config::is_valid_currency_code(&value) {
                    return;
                }
                let ui = ui_weak.unwrap();
                config.borrow_mut().reference_currency = value.to_string();
                persist(path.as_ref(), &config.borrow());
                ui.global::<Prefs>().set_reference_currency(value.clone());
                // Re-label the holdings amounts immediately (no conversion — Epic 4 is single-currency).
                ui.global::<Holdings>().set_reference_currency(value);
            });
    }
    // ── Story 4.5 (FR42/FR63) — the default trailing-stop %. "" clears; else validate (0,100). ──
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(&config);
        let path = config_path.clone();
        ui.global::<Prefs>()
            .on_default_trailing_stop_pct_changed(move |value| {
                let value = value.trim();
                // Empty clears the default; otherwise ignore a malformed percent (parse-guard).
                let stored = if value.is_empty() {
                    None
                } else if config::is_valid_trailing_stop_pct(value) {
                    Some(value.to_string())
                } else {
                    return;
                };
                let ui = ui_weak.unwrap();
                config.borrow_mut().default_trailing_stop_pct = stored;
                persist(path.as_ref(), &config.borrow());
                ui.global::<Prefs>().set_default_trailing_stop_pct(
                    config
                        .borrow()
                        .default_trailing_stop_pct_or_none()
                        .unwrap_or_default()
                        .into(),
                );
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
