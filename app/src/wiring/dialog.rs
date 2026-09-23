//! The modal-dialog rail of the UX pass (2026-09-23): the ONE place Rust routes a REFUSAL to. The
//! `Dialog` global is pure Slint state (its variants and forms live in `components/modal_dialog.slint`);
//! Rust only ever raises a refusal into it. The routing rule (the UX spec §3): a message emitted in
//! response to a gesture whose write did not happen is a refusal → a `notice` the user acknowledges,
//! or — while one of the Dialog's own forms is open — the form's inline `field-error`, so the user
//! sees the cause next to the offending field with their text intact. A persistent STATE never comes
//! here (StatusBand); an OUTCOME (« … a été enregistré. ») stays an inline notice (the F4 slot rule).

use slint::ComponentHandle;

use crate::{Dialog, MainWindow};

/// Raise a refusal: into the open form's `field-error`, else as a notice dialog to acknowledge.
pub(crate) fn refuse(ui: &MainWindow, message: &str) {
    let dialog = ui.global::<Dialog>();
    if dialog.get_kind() == "form" {
        dialog.set_field_error(message.into());
    } else {
        dialog.set_title("".into()); // "" = the overlay's default « Action refusée »
        dialog.set_body(message.into());
        dialog.set_kind("notice".into());
    }
}

/// Raise a confirmation the user must answer: `action` names what the verb does (the overlay
/// derives the title and the verb's label from it — the strings stay in Slint, posture-gated),
/// `body` is the fact-stating prompt already worded by the state layer.
pub(crate) fn confirm(ui: &MainWindow, action: &str, body: &str) {
    let dialog = ui.global::<Dialog>();
    dialog.set_title("".into());
    dialog.set_verb("".into());
    dialog.set_body(body.into());
    dialog.set_action(action.into());
    dialog.set_target_id("".into());
    dialog.set_kind("confirm".into());
}
