//! Judgment wiring (Stories 2.6/2.8–2.11): the numeric judgment-field commits (locale-parsed,
//! blank → cleared never 0), the decision rationale (FR49), the annual extend-history roll-forward
//! (FR3), the §1 draggable judgment line (start / moved / commit / cancel — live non-persisted
//! preview under NFR-P1, one persisted write on commit, FR31), and undo / redo (Story 2.9 snapshot
//! stack). Moved verbatim from `main.rs` — no logic change.

use std::cell::RefCell;
use std::rc::Rc;

use slint::ComponentHandle;
use uuid::Uuid;

use crate::state::JournalState;
use crate::viewmodel::format::NumberFormat;
use crate::wiring::Session;
use crate::wiring::push::{push_form, push_live_preview};
use crate::wiring::study_notice::{self, Source};
use crate::{MainWindow, Studies};
use crate::{state, viewmodel};

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

    // Commit a numeric judgment field: read under the user's number format (blank → cleared, never
    // 0), persist to `Study.judgment`, then re-read + re-push (which recomputes the snapshot). G1 I:
    // a text that is no number, or an ambiguous one, is refused with its reason; every refusal goes
    // to the refusal dialog and returns `false`, so the field re-shows its stored value.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>().on_set_judgment(move |field, text| {
            let ui = ui_weak.unwrap();
            let Some(id_text) = current_study.borrow().clone() else {
                return false;
            };
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return false;
            };
            let format = config.borrow().number_format;
            let value = match state::typed_entry(&text, format) {
                Ok(value) => value,
                Err(message) => {
                    crate::wiring::dialog::refuse(&ui, &message);
                    return false;
                }
            };
            let result = journal_state
                .borrow_mut()
                .set_judgment_field(id, field.as_str(), value);
            match result {
                Ok(()) => {
                    study_notice::clear(&ui, Source::Edit);
                    if let Some(study) = journal_state.borrow().get_study(id) {
                        push_form(&ui, &journal_state.borrow(), &study, format);
                    }
                    true
                }
                Err(message) => {
                    crate::wiring::dialog::refuse(&ui, &message);
                    false
                }
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
                    study_notice::clear(&ui, Source::Edit);
                    if let Some(study) = journal_state.borrow().get_study(id) {
                        push_form(&ui, &journal_state.borrow(), &study, format);
                    }
                }
                Err(message) => study_notice::fail(&ui, Source::Edit, &message),
            }
        });
    }

    // ── Story 8.8 (FR68) — « Valider l'étude »: freeze the full verdict (the replace confirm, when
    //    one is stored, was answered on the Slint side). An owner edit: undoable, one FR51 entry;
    //    the outcome goes to the study's notice slot; a refusal is « Action refusée ». ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>().on_freeze_verdict(move || {
            let ui = ui_weak.unwrap();
            let id = match current_study_id(&current_study) {
                Ok(id) => id,
                Err(message) => return crate::wiring::dialog::refuse(&ui, &message),
            };
            let result = journal_state.borrow_mut().freeze_verdict(id);
            match result {
                Ok(at) => {
                    if let Some(study) = journal_state.borrow().get_study(id) {
                        push_form(
                            &ui,
                            &journal_state.borrow(),
                            &study,
                            config.borrow().number_format,
                        );
                    }
                    study_notice::outcome(
                        &ui,
                        Source::Edit,
                        &state::MSG_FREEZE_DONE
                            .replace("{date}", &crate::viewmodel::frozen::day_month(&at)),
                    );
                }
                Err(message) => crate::wiring::dialog::refuse(&ui, &message),
            }
        });
    }

    // ── Story 8.1 (FR78) — the study's notes: the `note-add` / `note-edit` forms and the
    //    `delete-note` confirm. Each rail is atomic + undoable (`mutate_study`); a refusal raised by
    //    a form's own submit lands in its `field-error` (`dialog::refuse` honours `Dialog.gesture`),
    //    and the form closes only on `true`. After a write the form is re-pushed from the dossier;
    //    a failed re-read marks the notes card « indisponible » (#95), never an empty card. ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>().on_add_note(move |text| {
            let ui = ui_weak.unwrap();
            let id = match current_study_id(&current_study) {
                Ok(id) => id,
                Err(message) => {
                    crate::wiring::dialog::refuse(&ui, &message);
                    return false;
                }
            };
            let result = journal_state.borrow_mut().add_note(id, &text).map(|_| ());
            note_outcome(
                &ui,
                &journal_state,
                id,
                config.borrow().number_format,
                result,
            )
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>().on_edit_note(move |note_id, text| {
            let ui = ui_weak.unwrap();
            let id = match current_study_id(&current_study) {
                Ok(id) => id,
                Err(message) => {
                    crate::wiring::dialog::refuse(&ui, &message);
                    return false;
                }
            };
            let Ok(note_id) = Uuid::parse_str(&note_id) else {
                crate::wiring::dialog::refuse(&ui, state::MSG_NOTE_GONE);
                return false;
            };
            let result = journal_state.borrow_mut().edit_note(id, note_id, &text);
            note_outcome(
                &ui,
                &journal_state,
                id,
                config.borrow().number_format,
                result,
            )
        });
    }
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>().on_delete_note(move |note_id| {
            let ui = ui_weak.unwrap();
            let id = match current_study_id(&current_study) {
                Ok(id) => id,
                Err(message) => {
                    crate::wiring::dialog::refuse(&ui, &message);
                    return;
                }
            };
            let Ok(note_id) = Uuid::parse_str(&note_id) else {
                crate::wiring::dialog::refuse(&ui, state::MSG_NOTE_GONE);
                return;
            };
            let result = journal_state.borrow_mut().delete_note(id, note_id);
            note_outcome(
                &ui,
                &journal_state,
                id,
                config.borrow().number_format,
                result,
            );
        });
    }

    // Story 8.1 (G3 B4): « Afficher tout » / « Réduire » flips the row in place, so the choice
    // survives every re-push of the form.
    {
        let ui_weak = ui.as_weak();
        ui.global::<Studies>()
            .on_toggle_note_expanded(move |note_id| {
                use slint::Model;
                let ui = ui_weak.unwrap();
                let notes = ui.global::<Studies>().get_notes();
                if let Some(index) = notes.iter().position(|row| row.id == note_id)
                    && let Some(mut row) = notes.row_data(index)
                {
                    row.expanded = !row.expanded;
                    notes.set_row_data(index, row);
                }
            });
    }

    // ── 2026-07-12 — commit the header card's company name. Exact mirror of `on_set_rationale`:
    //    parse-free free text → `state::set_company_name` (trims → Some/None, atomic, undoable) →
    //    re-read + `push_form`. ──
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>().on_set_company_name(move |text| {
            let ui = ui_weak.unwrap();
            let Some(id_text) = current_study.borrow().clone() else {
                return;
            };
            let Ok(id) = Uuid::parse_str(&id_text) else {
                return;
            };
            let format = config.borrow().number_format;
            let result = journal_state
                .borrow_mut()
                .set_company_name(id, Some(text.to_string()));
            match result {
                Ok(()) => {
                    study_notice::clear(&ui, Source::Edit);
                    if let Some(study) = journal_state.borrow().get_study(id) {
                        push_form(&ui, &journal_state.borrow(), &study, format);
                    }
                }
                Err(message) => study_notice::fail(&ui, Source::Edit, &message),
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
                    study_notice::clear(&ui, Source::Edit);
                    if let Some(study) = journal_state.borrow().get_study(id) {
                        push_form(&ui, &journal_state.borrow(), &study, format);
                    }
                }
                Err(message) => study_notice::fail(&ui, Source::Edit, &message),
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
            // Issue #25: the est-high-EPS handle lives on the EPS series' OWN scale — invert against
            // its bounds (read from the chart state), not a fixed 1→200.
            let chart = ui.global::<Studies>().get_growth_chart();
            let value = Some(viewmodel::chart::judgment_value_for_y(
                y,
                chart.axis_min as f64,
                chart.axis_max as f64,
            ));
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
            // Issue #25: invert the drag against the EPS scale's bounds (as the `moved` handler does).
            let chart = ui.global::<Studies>().get_growth_chart();
            let value = Some(viewmodel::chart::judgment_value_for_y(
                y,
                chart.axis_min as f64,
                chart.axis_max as f64,
            ));
            let result = journal_state
                .borrow_mut()
                .set_judgment_field(id, field.as_str(), value);
            match result {
                Ok(()) => study_notice::clear(&ui, Source::Edit),
                // The write was refused — surface the notice AND reconcile the (un-saved) preview
                // back to the saved study below, so no phantom line is left on screen (review P3).
                Err(message) => study_notice::fail(&ui, Source::Edit, &message),
            }
            // Re-read + re-push from disk: on success this confirms the saved value; on failure it
            // reverts the live preview to what is actually persisted.
            if let Some(study) = journal_state.borrow().get_study(id) {
                push_form(&ui, &journal_state.borrow(), &study, format);
            }
        });
    }

    // Issue #28: the arrow-key equivalent of the drag — one press moves the endpoint by `delta`
    // (viewbox px) and persists immediately (a single click-drag-release, atomically; no separate
    // uncommitted-drag state, unlike the pointer path, since there is nothing to abandon on blur).
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        ui.global::<Studies>()
            .on_judgment_step(move |field, delta| {
                let ui = ui_weak.unwrap();
                let studies = ui.global::<Studies>();
                let chart = studies.get_growth_chart();
                let active_y = if field.as_str() == "est_low_eps" {
                    chart.judgment_low_y
                } else {
                    chart.judgment_y
                };
                // Nothing shown yet (no judgment, no fittable seed) → no position to step FROM; the
                // Slint key handler already gates on this, this is defense in depth.
                if active_y < 0.0 {
                    return;
                }
                let y = (active_y + delta).clamp(0.0, chart.chart_h);
                let value = Some(viewmodel::chart::judgment_value_for_y(
                    y,
                    chart.axis_min as f64,
                    chart.axis_max as f64,
                ));
                let Some(id_text) = current_study.borrow().clone() else {
                    return;
                };
                let Ok(id) = Uuid::parse_str(&id_text) else {
                    return;
                };
                let format = config.borrow().number_format;
                let result =
                    journal_state
                        .borrow_mut()
                        .set_judgment_field(id, field.as_str(), value);
                match result {
                    Ok(()) => {
                        study_notice::clear(&ui, Source::Edit);
                        if let Some(study) = journal_state.borrow().get_study(id) {
                            push_form(&ui, &journal_state.borrow(), &study, format);
                        }
                    }
                    Err(message) => study_notice::fail(&ui, Source::Edit, &message),
                }
            });
    }

    // ── Issue #115 — the draggable §3 judged-P/E lines. drag-start / cancel are shared with §1 (they
    //    only cache / restore the study); move + commit need their OWN inversion because the P/E chart
    //    is a LINEAR scale (read `pe_chart` bounds, invert via `pe_value_for_y`), not the §1 EPS log
    //    scale. The persistence rail (`high_pe`/`low_pe` → judged_avg_*) is the SAME as the fields. ──

    // P/E drag move: map pointer-y → judged P/E on the LINEAR scale, apply to the CACHED study, push a
    // LIVE recompute (no persistence, NFR-P1). The exact-value §3 field mirrors the line (FR31).
    {
        let ui_weak = ui.as_weak();
        let config = Rc::clone(config);
        let drag_study = Rc::clone(drag_study);
        let drag_moved = Rc::clone(drag_moved);
        ui.global::<Studies>()
            .on_pe_judgment_moved(move |field, y| {
                let ui = ui_weak.unwrap();
                let Some(mut preview) = drag_study.borrow().clone() else {
                    return;
                };
                *drag_moved.borrow_mut() = true;
                // The judged-P/E handle lives on the LINEAR P/E scale — invert against the P/E chart's
                // bounds (not the §1 EPS log scale).
                let chart = ui.global::<Studies>().get_pe_chart();
                let value = Some(viewmodel::chart::pe_value_for_y(
                    y,
                    chart.axis_min as f64,
                    chart.axis_max as f64,
                ));
                if !state::apply_judgment_field(&mut preview.judgment, field.as_str(), value) {
                    return;
                }
                let format = config.borrow().number_format;
                push_live_preview(&ui, &preview, format);
            });
    }

    // P/E drag commit (pointer-up): persist ONCE via the same rail as the exact-value field, then
    // re-read + full re-push. A no-move click never rewrites the judgment; a refused write surfaces a
    // neutral notice and reconciles the preview back to disk.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        let drag_study = Rc::clone(drag_study);
        let drag_moved = Rc::clone(drag_moved);
        ui.global::<Studies>()
            .on_pe_judgment_commit(move |field, y| {
                let ui = ui_weak.unwrap();
                *drag_study.borrow_mut() = None;
                let moved = std::mem::replace(&mut *drag_moved.borrow_mut(), false);
                let Some(id_text) = current_study.borrow().clone() else {
                    return;
                };
                let Ok(id) = Uuid::parse_str(&id_text) else {
                    return;
                };
                let format = config.borrow().number_format;
                if !moved {
                    return;
                }
                let chart = ui.global::<Studies>().get_pe_chart();
                let value = Some(viewmodel::chart::pe_value_for_y(
                    y,
                    chart.axis_min as f64,
                    chart.axis_max as f64,
                ));
                let result =
                    journal_state
                        .borrow_mut()
                        .set_judgment_field(id, field.as_str(), value);
                match result {
                    Ok(()) => study_notice::clear(&ui, Source::Edit),
                    Err(message) => study_notice::fail(&ui, Source::Edit, &message),
                }
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
            // Bind first: a `borrow_mut()` in the `match` scrutinee stays alive for the whole
            // `match`, so the `journal_state.borrow()` in the Ok(true) arm would panic "RefCell
            // already borrowed". (Same class as the fetch.rs price-refresh panic.)
            let undone = journal_state.borrow_mut().undo(id);
            stepped_outcome(&ui, &journal_state, id, format, undone, true);
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
            // Bind first: a `borrow_mut()` in the `match` scrutinee stays alive for the whole
            // `match`, so the `journal_state.borrow()` in the Ok(true) arm would panic "RefCell
            // already borrowed". (Same class as the fetch.rs price-refresh panic.)
            let redone = journal_state.borrow_mut().redo(id);
            stepped_outcome(&ui, &journal_state, id, format, redone, false);
        });
    }
}

/// Settle an undo / redo (Story 2.9; Story 8.5b for a draft step): re-render the study; a step over
/// an AI draft's validation says so in the study's notice slot and re-reads the inbox (the app's own
/// writes never move `PRAGMA data_version`); a dropped draft step is a refusal (« Action
/// refusée »); any other failure stays the study's own failure notice (the history is kept).
fn stepped_outcome(
    ui: &MainWindow,
    journal_state: &Rc<RefCell<JournalState>>,
    id: Uuid,
    format: NumberFormat,
    result: Result<state::Stepped, String>,
    undo: bool,
) {
    match result {
        Ok(state::Stepped::Nothing) => {}
        Ok(stepped) => {
            study_notice::clear(ui, Source::Edit);
            // G1 J review: the fetch summary may describe what this step reverted — it goes (its
            // outcome only; a fetch failure stays, F4).
            study_notice::clear_outcome(ui, Source::Fetch);
            if let Some(study) = journal_state.borrow().get_study(id) {
                push_form(ui, &journal_state.borrow(), &study, format);
            }
            // G3: the focused cell's revealed facts follow the restored value.
            refresh_active_facts(ui);
            // Story 8.8: a step over the freeze says so (the redo names the restored date).
            if let state::Stepped::Freeze = stepped {
                let text = if undo {
                    state::MSG_FREEZE_UNDONE.to_string()
                } else {
                    let date = journal_state
                        .borrow()
                        .get_study(id)
                        .and_then(|s| s.frozen_verdict)
                        .map(|f| crate::viewmodel::frozen::day_month(&f.frozen_at))
                        .unwrap_or_default();
                    state::MSG_FREEZE_DONE.replace("{date}", &date)
                };
                study_notice::outcome(ui, Source::Edit, &text);
            }
            if let state::Stepped::Draft(_) = stepped {
                ui.global::<crate::Drafts>()
                    .set_notice(slint::SharedString::new());
                let text = if undo {
                    state::MSG_DRAFT_UNDONE
                } else {
                    state::MSG_DRAFT_REDONE
                };
                study_notice::outcome(ui, Source::Edit, text);
                crate::wiring::drafts::push_drafts(ui, &journal_state.borrow());
            }
        }
        Err(message) if message == state::MSG_UNDO_DRAFT_STEP_DROPPED => {
            // G3: the step left the history — the undo / redo controls follow.
            if let Some(study) = journal_state.borrow().get_study(id) {
                push_form(ui, &journal_state.borrow(), &study, format);
            }
            crate::wiring::dialog::refuse(ui, &message);
            crate::wiring::drafts::push_drafts(ui, &journal_state.borrow());
        }
        Err(message) => study_notice::fail(ui, Source::Edit, &message),
    }
}

/// Re-read the focused grid cell's revealed facts (source, date, pending provider value, AI-draft
/// validation) from the rows just pushed (Story 8.5b G3): an undo / redo changes the cell under
/// the cursor without a new focus event.
fn refresh_active_facts(ui: &MainWindow) {
    use slint::Model;
    let studies = ui.global::<Studies>();
    let (year, field) = (studies.get_active_year(), studies.get_active_field());
    if year < 0 || field.is_empty() {
        return;
    }
    let mut cells = Vec::new();
    let pe = studies.get_pe_rows();
    for i in 0..pe.row_count() {
        if let Some(r) = pe.row_data(i) {
            cells.extend([r.a, r.b, r.c, r.f]);
        }
    }
    let mgmt = studies.get_mgmt_rows();
    for i in 0..mgmt.row_count() {
        if let Some(r) = mgmt.row_data(i) {
            cells.extend(r.cells.iter());
        }
    }
    if let Some(c) = cells
        .into_iter()
        .find(|c| c.year_index == year && c.field == field)
    {
        studies.set_active_source(c.source);
        studies.set_active_timestamp(c.timestamp);
        studies.set_active_pending(c.pending);
        studies.set_active_draft_validated(c.draft_validated);
    }
}

/// The open study's id for a note rail (Story 8.1): with none open (a stale callback — the demo,
/// a closed study) the gesture is refused by name, never dropped in silence (G3 E9).
fn current_study_id(current_study: &Rc<RefCell<Option<String>>>) -> Result<Uuid, String> {
    current_study
        .borrow()
        .as_deref()
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or_else(|| state::MSG_NO_STUDY_OPEN.to_string())
}

/// Settle a note rail's result (Story 8.1): on success clear the edit notice and re-push the form
/// from the dossier (a failed re-read marks the card « indisponible », #95); on a refusal route it
/// through `dialog::refuse` (the open form's `field-error` during its own gesture, else « Action
/// refusée »). Returns whether the write happened — a form closes only then.
fn note_outcome(
    ui: &MainWindow,
    journal_state: &Rc<RefCell<JournalState>>,
    id: Uuid,
    format: NumberFormat,
    result: Result<(), String>,
) -> bool {
    match result {
        Ok(()) => {
            study_notice::clear(ui, Source::Edit);
            let reread = journal_state.borrow().try_get_study(id);
            match reread {
                Ok(Some(study)) => push_form(ui, &journal_state.borrow(), &study, format),
                _ => {
                    // The write happened but the form cannot be rebuilt: say the notes are
                    // unavailable, keep undo / redo in step with the stacks (the write pushed a
                    // step), and an open « Historique » cannot be trusted either (G3 E3, #95).
                    let studies = ui.global::<Studies>();
                    let state = journal_state.borrow();
                    studies.set_notes_unavailable(true);
                    studies.set_can_undo(state.can_undo());
                    studies.set_can_redo(state.can_redo());
                    if studies.get_history_open() {
                        studies.set_history_unavailable(true);
                        studies.set_history_rows(slint::ModelRc::new(slint::VecModel::from(
                            Vec::<crate::HistoryEntryRow>::new(),
                        )));
                    }
                }
            }
            true
        }
        Err(message) => {
            crate::wiring::dialog::refuse(ui, &message);
            false
        }
    }
}
