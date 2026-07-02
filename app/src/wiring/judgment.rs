//! Judgment wiring (Stories 2.6/2.8–2.11): the numeric judgment-field commits (locale-parsed,
//! blank → cleared never 0), the decision rationale (FR49), the annual extend-history roll-forward
//! (FR3), the §1 draggable judgment line (start / moved / commit / cancel — live non-persisted
//! preview under NFR-P1, one persisted write on commit, FR31), and undo / redo (Story 2.9 snapshot
//! stack). Moved verbatim from `main.rs` — no logic change.

use std::rc::Rc;

use slint::{ComponentHandle, SharedString};
use uuid::Uuid;

use crate::wiring::push::{push_form, push_live_preview};
use crate::wiring::Session;
use crate::{state, viewmodel};
use crate::{MainWindow, Studies};

/// Wire the judgment domain: field commits, rationale, extend-history, the §1 drag gesture and
/// undo / redo.
pub(crate) fn wire_judgment(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        current_study,
        drag_study,
        drag_moved,
        ..
    } = s;
    // ── Numeric judgment-input editing + the §4 selector + traceability (Story 2.6) ──

    // Commit a numeric judgment field: parse locale-aware (None for blank/unparseable → cleared,
    // never 0), persist to `Study.judgment`, then re-read + re-push (which recomputes the snapshot).
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
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
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
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
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
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
        let journal_state = Rc::clone(journal_state);
        let current_study = Rc::clone(current_study);
        let drag_study = Rc::clone(drag_study);
        let drag_moved = Rc::clone(drag_moved);
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
        let config = Rc::clone(config);
        let drag_study = Rc::clone(drag_study);
        let drag_moved = Rc::clone(drag_moved);
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
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        let drag_study = Rc::clone(drag_study);
        let drag_moved = Rc::clone(drag_moved);
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
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        let drag_study = Rc::clone(drag_study);
        let drag_moved = Rc::clone(drag_moved);
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
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
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
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
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
}
