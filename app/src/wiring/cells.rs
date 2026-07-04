//! Cell-editing wiring (Stories 2.4/2.5/3.4): the manual-entry rail (commit / paste-column /
//! not-available / cursor move — parse → `Cell::edited` → `put_study` → re-read → re-push), the
//! tri-state review tag + soft-lock notice + bulk-unlock confirm flow (`parse_unlock_scope`), and
//! the Story 3.4 provider-divergence resolution (accept-provider / keep-manual, FR22). Moved
//! verbatim from `main.rs` — no logic change.

use std::rc::Rc;

use slint::{ComponentHandle, Model, SharedString};
use uuid::Uuid;

use crate::state::UnlockScope;
use crate::wiring::push::push_form;
use crate::wiring::Session;
use crate::{state, viewmodel};
use crate::{MainWindow, Studies};

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

/// Wire the cell-editing domain: commit / paste / not-available / cursor move, review tags +
/// soft-lock + the request → confirm → cancel bulk unlock, and provider-divergence resolution.
pub(crate) fn wire_cells(ui: &MainWindow, s: &Session) {
    let Session {
        journal_state,
        config,
        current_study,
        pending_unlock,
        ..
    } = s;
    // ── Manual-entry intents (Story 2.4): parse → `Cell::edited` → `put_study` → re-read → re-push
    //    (validate→mutate→persist→rebuild — the 2.3 single-source-of-truth shape). Every refusal
    //    surfaces a neutral banner, never a silent `.ok()`. ──

    // Commit a typed cell: parse the text locale-aware (None for blank/unparseable → a to-fill gap,
    // never 0), edit + persist, then rebuild the form from the re-read study. Returns written? so
    // the cell keeps the user's text on a recoverable refusal.
    {
        let ui_weak = ui.as_weak();
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
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
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
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
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
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
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
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
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
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
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
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
        let journal_state = Rc::clone(journal_state);
        let current_study = Rc::clone(current_study);
        let pending_unlock = Rc::clone(pending_unlock);
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
        let journal_state = Rc::clone(journal_state);
        let config = Rc::clone(config);
        let current_study = Rc::clone(current_study);
        let pending_unlock = Rc::clone(pending_unlock);
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
            // Bind first: a `borrow_mut()` in the `match` scrutinee stays alive for the whole
            // `match`, so the `journal_state.borrow()` in the Ok arm would panic "RefCell already
            // borrowed". (Same class as the fetch.rs price-refresh panic.)
            let unlocked = journal_state.borrow_mut().unlock_all(id, &scope);
            match unlocked {
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
        let pending_unlock = Rc::clone(pending_unlock);
        ui.global::<Studies>().on_cancel_unlock(move || {
            let ui = ui_weak.unwrap();
            *pending_unlock.borrow_mut() = None;
            ui.global::<Studies>().set_confirm_visible(false);
        });
    }
}
