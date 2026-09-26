//! The modal-dialog rail of the UX pass (2026-09-23): the ONE place Rust routes a REFUSAL to. The
//! `Dialog` global is pure Slint state (its variants and forms live in `components/modal_dialog.slint`);
//! Rust only ever raises a refusal (or a parked confirmation) into it. The routing rule (the UX spec
//! §3): a message emitted in response to a gesture whose write did not happen is a refusal → a
//! `notice` the user acknowledges, or — when it answers the open form's OWN submit — the form's
//! inline `field-error`, so the user sees the cause next to the offending field with their text
//! intact. A persistent STATE never comes here (StatusBand); an OUTCOME (« … a été enregistré. »)
//! stays an inline notice (the F4 slot rule).
//!
//! G1 review: the overlay shows ONE thing at a time, and nothing arriving later may overwrite it —
//! a refusal landing while a confirm is parked (its Rust-side pending action armed), while an
//! unrelated form is open, or while an unread notice is showing is QUEUED, and shown when the
//! open dialog closes (`Dialog.closed`). Only the open form's own gesture (`Dialog.gesture`, set by
//! the form's submit around its callback) reaches `field-error`.

use std::cell::RefCell;
use std::collections::VecDeque;

use slint::ComponentHandle;

use crate::{Dialog, MainWindow};

/// Something Rust raised while the overlay was busy — shown, in order, as it frees up.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Queued {
    Refusal(String),
    Confirm {
        action: String,
        body: String,
        target: String,
    },
}

thread_local! {
    // The UI is single-threaded (every callback runs on the event loop), so a thread-local queue
    // is the whole synchronisation story.
    static QUEUE: RefCell<VecDeque<Queued>> = const { RefCell::new(VecDeque::new()) };
}

/// Where a refusal goes, given what the overlay shows (`kind`) and whether it answers the open
/// form's own submit (`gesture`). Pure — the routing rule, tested without a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// Inline, next to the fields of the form whose submit it answers.
    FieldError,
    /// The overlay is free: a notice to acknowledge.
    Notice,
    /// The overlay is busy (a parked confirm, another form, an unread notice): wait for it.
    Queue,
}

fn route(kind: &str, gesture: bool) -> Route {
    match kind {
        "" => Route::Notice,
        "form" if gesture => Route::FieldError,
        _ => Route::Queue,
    }
}

/// Raise a refusal: into the open form's `field-error` when it answers that form's submit, as a
/// notice when the overlay is free, else queued behind what is showing (never overwriting it).
pub(crate) fn refuse(ui: &MainWindow, message: &str) {
    let dialog = ui.global::<Dialog>();
    match route(&dialog.get_kind(), dialog.get_gesture()) {
        Route::FieldError => dialog.set_field_error(message.into()),
        Route::Notice => show_notice(ui, message),
        Route::Queue => {
            QUEUE.with(|q| {
                q.borrow_mut()
                    .push_back(Queued::Refusal(message.to_string()))
            });
        }
    }
}

/// Raise a confirmation the user must answer: `action` names what the verb does (the overlay
/// derives the title and the verb's label from it — the strings stay in Slint, posture-gated),
/// `body` is the fact-stating prompt already worded by the state layer.
pub(crate) fn confirm(ui: &MainWindow, action: &str, body: &str) {
    confirm_for(ui, action, body, "");
}

/// [`confirm`] acting on `target` (the portfolio / holding / row the verb applies to). Queued when
/// the overlay is busy — a parked confirm must never be overwritten either.
pub(crate) fn confirm_for(ui: &MainWindow, action: &str, body: &str, target: &str) {
    if ui.global::<Dialog>().get_kind().is_empty() {
        show_confirm(ui, action, body, target);
    } else {
        QUEUE.with(|q| {
            q.borrow_mut().push_back(Queued::Confirm {
                action: action.to_string(),
                body: body.to_string(),
                target: target.to_string(),
            });
        });
    }
}

fn show_notice(ui: &MainWindow, message: &str) {
    let dialog = ui.global::<Dialog>();
    dialog.set_title("".into()); // "" = the overlay's default « Action refusée »
    dialog.set_body(message.into());
    dialog.set_kind("notice".into());
}

fn show_confirm(ui: &MainWindow, action: &str, body: &str, target: &str) {
    let dialog = ui.global::<Dialog>();
    dialog.set_title("".into());
    dialog.set_verb("".into());
    dialog.set_body(body.into());
    dialog.set_action(action.into());
    dialog.set_target_id(target.into());
    dialog.set_kind("confirm".into());
}

/// Wire `Dialog.closed`: every close (Compris, Annuler, a written form, a confirm's verb) shows the
/// next queued item, if any.
pub(crate) fn wire_dialog(ui: &MainWindow) {
    let ui_weak = ui.as_weak();
    ui.global::<Dialog>().on_closed(move || {
        let ui = ui_weak.unwrap();
        if !ui.global::<Dialog>().get_kind().is_empty() {
            return;
        }
        match QUEUE.with(|q| q.borrow_mut().pop_front()) {
            Some(Queued::Refusal(message)) => show_notice(&ui, &message),
            Some(Queued::Confirm {
                action,
                body,
                target,
            }) => show_confirm(&ui, &action, &body, &target),
            None => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{Route, route};

    #[test]
    fn a_refusal_never_overwrites_what_the_overlay_shows() {
        // Free overlay → a notice.
        assert_eq!(route("", false), Route::Notice);
        assert_eq!(route("", true), Route::Notice);
        // The open form's own submit → inline next to its fields.
        assert_eq!(route("form", true), Route::FieldError);
        // An unrelated (async) refusal while a form is open → queued, never in its field-error.
        assert_eq!(route("form", false), Route::Queue);
        // A parked confirm (a pending Rust action) or an unread notice → queued, never replaced.
        assert_eq!(route("confirm", false), Route::Queue);
        assert_eq!(route("confirm", true), Route::Queue);
        assert_eq!(route("notice", false), Route::Queue);
    }
}
