//! Studies wiring: dashboard + open-form lifecycle — create (Story 2.2), open + fold/regime
//! restore (Stories 2.3/2.5-view), search / sort / filter + archive / un-archive / delete behind
//! the confirm overlay (Story 2.12), the single-study JSON export/import (Story 5.2, FR59) and
//! faithful PDF export (Story 5.6, FR52) with their `exports/`-folder file writers (ADD7/8 —
//! never beside the live journal), the verify-engine replay (Story 2.13, FR9) and the read-only
//! demo study (FR62), plus the `refresh_studies` dashboard re-render. Moved verbatim from
//! `main.rs` — no logic change.

use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use uuid::Uuid;

use crate::config::StudyViewState;
use crate::regime::Regime;
use crate::state::JournalState;
use crate::wiring::push::{push_form, push_view_state};
use crate::wiring::watchlist::refresh_watchlist;
use crate::wiring::{Session, persist};
use crate::{FixtureLine, MainWindow, Prefs, ScenarioCompareState, Studies, StudyRow, Verify};
use crate::{regime, state, viewmodel};

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

/// The default `exports/` folder under the OS data dir — the native PDF save picker (issue #106)
/// opens here, but the user is free to save anywhere. `None` when the OS exposes no data directory.
fn default_exports_dir() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("", "", "steadyinvest").map(|d| d.data_dir().join("exports"))
}

/// A filesystem-safe default file stem from a ticker (e.g. `AAPL.US` → `AAPL.US`, `BRK/B` → `BRK_B`).
/// Path separators and control/space characters become `_`; the common `.`/`-` are kept.
fn safe_stem(ticker: &str) -> String {
    let stem: String = ticker
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if stem.is_empty() {
        "etude".to_string()
    } else {
        stem
    }
}

/// Rebuild the dashboard list from the journal and mirror the read-only flag into the `Studies`
/// global. Called on startup and after every create.
pub(crate) fn refresh_studies(ui: &MainWindow, state: &JournalState) {
    use steadyinvest_core::rounding::DisplayField;
    let studies = ui.global::<Studies>();
    // The dashboard view state (search/sort/filter) lives on the `Studies` global — read it back and
    // curate the persistence summaries (Story 2.12). Deterministic, pure (`viewmodel::studies::curate`).
    let summaries = state.list_studies();
    // Issue #107: the estimated potential per study (§5 projected total annualized return). Computed
    // app-side via `build_snapshot` (Cardinal Rule kept: `curate` only READS this) and formatted with
    // the user's number format (mirrored on `Prefs`, as `replacement.rs` already reads it). A study
    // that does not compute / withholds the return contributes an absent value → sorts last, "—" shown.
    // Personal scale: one snapshot per study per refresh is sub-ms (the holdings register already does
    // the same per row).
    let format =
        crate::viewmodel::format::NumberFormat::parse(&ui.global::<Prefs>().get_number_format())
            .unwrap_or_default();
    let mut returns: std::collections::HashMap<uuid::Uuid, viewmodel::studies::StudyReturn> =
        std::collections::HashMap::new();
    for summary in &summaries {
        let Some(study) = state.get_study(summary.id) else {
            continue;
        };
        // ONE snapshot per study feeds both the §5 potential (issue #107) and the issue #148
        // "à compléter" flag — no second engine pass. A study that fails to normalize is "unknown":
        // absent potential ("—") and NOT flagged incomplete (a broken normalize must not shout).
        let snapshot = crate::viewmodel::engine::build_snapshot(&study).ok();
        let value = snapshot
            .as_ref()
            .and_then(|snap| snap.outputs().returns.projected_total_annualized_return_pct);
        let display = match value {
            Some(v) => format!(
                "{} %",
                crate::viewmodel::format::format_scaled(v, DisplayField::Percent, format)
            ),
            None => crate::viewmodel::form::EMPTY_SLOT.to_string(),
        };
        let incomplete = snapshot
            .as_ref()
            .is_some_and(crate::viewmodel::engine::study_incomplete);
        returns.insert(
            summary.id,
            viewmodel::studies::StudyReturn {
                value,
                display,
                incomplete,
            },
        );
    }
    let rows: Vec<StudyRow> = viewmodel::studies::curate(
        &summaries,
        studies.get_search_query().as_str(),
        viewmodel::studies::SortKey::from_wire(studies.get_sort_key().as_str()),
        studies.get_sort_descending(),
        viewmodel::studies::StatusFilter::from_wire(studies.get_status_filter().as_str()),
        &returns,
    );
    studies.set_study_count(summaries.len() as i32);
    studies.set_rows(ModelRc::new(VecModel::from(rows)));
    studies.set_read_only(state.is_read_only());
}

/// Wire the studies domain: create / open (with per-study view-state restore) / fold / regime,
/// the dashboard search / sort / filter + lifecycle actions behind their confirm overlay, the
/// study JSON export/import + PDF export, and the verify / demo surfaces.
pub(crate) fn wire_studies(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        config_path,
        current_study,
        compare_study,
        pending_study_action,
        ..
    } = s;
    // Create-study intent: validate + persist via the injected sources, then refresh the list.
    // A refused create (blank input / read-only / save failure) surfaces a neutral banner; a
    // successful one clears it.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
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

    // ── Issue #34 (FR51, PR 2) — the durable « Historique » timeline of the open study: open /
    // close the read-only panel, expand one entry to its avant → après detail. The rows rebuild
    // on open and, while open, after every mutation (via `push_form` — the panel never shows a
    // stale timeline beside a fresh form). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>().on_open_history(move || {
            let ui = ui_weak.unwrap();
            let Some(id) = current_study.borrow().clone() else {
                return;
            };
            let Ok(id) = Uuid::parse_str(&id) else {
                return;
            };
            ui.global::<Studies>().set_history_open(true);
            crate::wiring::push::push_history(
                &ui,
                &journal_state.borrow(),
                id,
                config.borrow().number_format,
            );
        });
    }
    {
        let ui_weak = ui.as_weak();
        ui.global::<Studies>().on_close_history(move || {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            studies.set_history_open(false);
            studies.set_history_rows(ModelRc::new(VecModel::from(
                Vec::<crate::HistoryEntryRow>::new(),
            )));
            studies.set_history_detail_id(SharedString::new());
            studies
                .set_history_detail_lines(ModelRc::new(VecModel::from(Vec::<SharedString>::new())));
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>().on_toggle_history_entry(move |id| {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            // A second click on the expanded entry collapses it.
            if studies.get_history_detail_id() == id {
                studies.set_history_detail_id(SharedString::new());
                studies.set_history_detail_lines(ModelRc::new(VecModel::from(
                    Vec::<SharedString>::new(),
                )));
                return;
            }
            let Some(study_id) = current_study.borrow().clone() else {
                return;
            };
            let (Ok(study_id), Ok(snapshot_id)) =
                (Uuid::parse_str(&study_id), Uuid::parse_str(&id))
            else {
                return;
            };
            let state = journal_state.borrow();
            let format = config.borrow().number_format;
            // The detail diffs THIS snapshot against its predecessor — located by the listing
            // order (oldest first). Any read failure marks the whole panel « indisponible »
            // (the #95 discipline), never a silently empty detail.
            let detail = (|| -> Result<Vec<String>, String> {
                let summaries = state.try_list_study_history(study_id)?;
                let index = summaries
                    .iter()
                    .position(|s| s.id == snapshot_id)
                    .ok_or_else(String::new)?; // vanished between renders — treat as unavailable
                let next = state
                    .try_get_history_snapshot(snapshot_id)?
                    .ok_or_else(String::new)?;
                let prev = match index.checked_sub(1) {
                    Some(i) => Some(
                        state
                            .try_get_history_snapshot(summaries[i].id)?
                            .ok_or_else(String::new)?,
                    ),
                    None => None,
                };
                Ok(viewmodel::history::history_detail(
                    prev.as_ref(),
                    &next,
                    format,
                ))
            })();
            match detail {
                Ok(lines) => {
                    studies.set_history_detail_id(id);
                    studies.set_history_detail_lines(ModelRc::new(VecModel::from(
                        lines
                            .into_iter()
                            .map(SharedString::from)
                            .collect::<Vec<_>>(),
                    )));
                }
                Err(_) => {
                    studies.set_history_unavailable(true);
                    studies.set_history_detail_id(SharedString::new());
                    studies
                        .set_history_detail_lines(ModelRc::new(VecModel::from(
                            Vec::<SharedString>::new(),
                        )));
                }
            }
        });
    }

    // ── Story 5.2 (FR59) — export / import a single study as a portable file. The envelope is the
    // serialized data contract + schema_version + integrity hash (NOT a raw .db); `contract` owns the
    // envelope, `app` owns the file I/O. Path-based for now — the native picker is Story 5.5. ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
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
        // Story 5.6 (FR52) + issue #106: export a study's faithful, neutral, greyscale PDF via the
        // `report` crate (UI-independent, from `core`/`contract`). A native `rfd` save picker lets the
        // user choose the destination directory + filename (defaulting into exports/ with a ticker-
        // named file); cancel is a silent no-op. Read-only — rendering writes no journal.
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Studies>().on_export_study_pdf(move |id| {
            let ui = ui_weak.unwrap();
            let studies = ui.global::<Studies>();
            let Ok(uuid) = Uuid::parse_str(&id) else {
                return;
            };
            // Fetch + render BEFORE opening a dialog, so a study that does not compute never prompts
            // for a destination it can't fill.
            let Some(study) = journal_state.borrow().get_study(uuid) else {
                studies.set_notice(state::MSG_SAVE_FAILED.into());
                return;
            };
            let Ok(bytes) = steadyinvest_report::render_study_pdf(&study) else {
                // The study does not compute as entered — a neutral refusal, no panic, no leak.
                studies.set_notice(state::MSG_SAVE_FAILED.into());
                return;
            };
            // Native save picker on the UI thread (modal — the established `rfd` pattern, cf. the
            // journal export/create rails). Cancel → no notice, nothing written.
            let mut dialog = rfd::FileDialog::new()
                .set_title("Exporter l'étude en PDF")
                .add_filter("PDF", &["pdf"])
                .set_file_name(format!("etude-{}.pdf", safe_stem(&study.security_ticker)));
            if let Some(dir) = default_exports_dir() {
                let _ = std::fs::create_dir_all(&dir); // best-effort so the dialog opens there
                dialog = dialog.set_directory(dir);
            }
            let Some(path) = dialog.save_file() else {
                return;
            };
            // rfd does not force the filter extension on every platform — ensure `.pdf`.
            let path = if path.extension().is_some() {
                path
            } else {
                path.with_extension("pdf")
            };
            let notice = match std::fs::write(&path, &bytes) {
                Ok(()) => format!("{} {}", state::MSG_STUDY_EXPORTED, path.display()),
                Err(e) => format!("{} {e}", state::MSG_SAVE_FAILED),
            };
            studies.set_notice(notice.into());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
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

    // Money surfaces as formatted strings via the form adapter (the only float→string boundary), and
    // the persisted per-study fold/regime view-state is restored (default = Entry + all open).
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        let compare_study = Rc::clone(compare_study);
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
        let config = Rc::clone(config);
        let path = config_path.clone();
        let current_study = Rc::clone(current_study);
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
        let config = Rc::clone(config);
        let path = config_path.clone();
        let current_study = Rc::clone(current_study);
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

    // ── Story 2.12 — dashboard search / sort / filter. The view state lives on the `Studies` global;
    //    each control updates it then re-lists + re-curates (pure `viewmodel::studies::curate`). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        ui.global::<Studies>().on_set_search(move |text| {
            let ui = ui_weak.unwrap();
            ui.global::<Studies>().set_search_query(text);
            refresh_studies(&ui, &journal_state.borrow());
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
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
        let journal_state = Rc::clone(journal_state);
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
        let journal_state = Rc::clone(journal_state);
        let pending_study_action = Rc::clone(pending_study_action);
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
        let journal_state = Rc::clone(journal_state);
        let current_study = Rc::clone(current_study);
        let pending_study_action = Rc::clone(pending_study_action);
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
        let pending_study_action = Rc::clone(pending_study_action);
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
        let config = Rc::clone(config);
        let journal_state = Rc::clone(journal_state);
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
}
