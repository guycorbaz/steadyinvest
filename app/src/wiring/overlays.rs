//! Overlay wiring: the Story 5.1 (FR50/ADD13) confront overlay (read-only — the frozen §4 band
//! beside the cached actual closes), the Story 2.9 scenario compare (one non-persisted alternate,
//! FR32), the §4 forecast-low option selector (Story 2.6), and the verdict traceability surface
//! (inputs → provenance → rule trace). Moved verbatim from `main.rs` — no logic change.

use std::rc::Rc;

use slint::{ComponentHandle, SharedString};
use uuid::Uuid;

use crate::wiring::Session;
use crate::wiring::push::push_form;
use crate::{Confront, MainWindow, ScenarioCompareState, Studies, TraceState};
use crate::{state, viewmodel};

/// Wire the overlay domain: confront request / dismiss, scenario compare open / set-alternate /
/// close, the forecast-low option, and traceability open / close.
pub(crate) fn wire_overlays(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        current_study,
        compare_study,
        ..
    } = s;
    // ── Story 5.1 (FR50) — confront a saved study: reopen its recorded §4 forecast band beside the
    // security's actual close trajectory since the decision. Read-only — `confront()` never writes; it
    // reads the cached `price_history` closes the price refreshes have been accumulating. ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        ui.global::<Confront>().on_request(move |id| {
            let ui = ui_weak.unwrap();
            let confront = ui.global::<Confront>();
            let Ok(uuid) = Uuid::parse_str(&id) else {
                return;
            };
            let format = config.borrow().number_format;
            let view = journal_state.borrow().confront(uuid);
            confront.set_state(viewmodel::chart::confront_chart(&view, format));
            confront.set_open(true);
        });
    }
    {
        let ui_weak = ui.as_weak();
        ui.global::<Confront>().on_dismiss(move || {
            ui_weak.unwrap().global::<Confront>().set_open(false);
        });
    }

    // ── Story 2.9 — scenario compare (Phase-1, one alternate). Open caches the saved study and seeds
    //    the alternate = current; set-alternate recomputes the alternate's outcome from a NON-persisted
    //    clone; close discards it (the saved judgment is never overwritten — FR32). ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        let compare_study = Rc::clone(compare_study);
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
        let config = Rc::clone(config);
        let compare_study = Rc::clone(compare_study);
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
        let compare_study = Rc::clone(compare_study);
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
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
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
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
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
}
