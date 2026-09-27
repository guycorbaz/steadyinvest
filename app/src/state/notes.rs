//! Study notes (Story 8.1, FR78): the owner's dated notes on a study, add / edit / delete.
//!
//! Every rail rides [`JournalState::mutate_study`]: atomic (one `put_study_with_history`, the FR51
//! snapshot included), guarded (read-only / no journal / save failure) and undoable on a real change.
//! Ids and times come from the injected [`IdGen`](crate::clock::IdGen) / [`Clock`](crate::clock::Clock)
//! (ADD15). A note is the owner's own text: trimmed, never posture-scanned (FR13 covers
//! app-generated signals only).

use uuid::Uuid;

use super::{JournalState, MSG_NOTE_EMPTY, MSG_NOTE_GONE};
use steadyinvest_contract::Note;

impl JournalState {
    /// Add a note to the study; returns its id. An empty (or whitespace-only) text is refused with
    /// [`MSG_NOTE_EMPTY`] and nothing is written.
    pub fn add_note(&mut self, study_id: Uuid, text: &str) -> Result<Uuid, String> {
        let text = text.trim().to_string();
        if text.is_empty() {
            return Err(MSG_NOTE_EMPTY.to_string());
        }
        let id = self.idgen.new_id();
        let now = self.clock.now();
        self.mutate_study(study_id, move |study| {
            study.notes.push(Note {
                id,
                text,
                created_at: now.clone(),
                updated_at: now,
                ai_origin: None,
            });
        })?;
        Ok(id)
    }

    /// Replace a note's text. The same text is a no-op (no `updated_at` bump, no undo step, no
    /// history entry); an empty text is refused with [`MSG_NOTE_EMPTY`]; a note that no longer
    /// exists (e.g. removed by an undo) is refused with [`MSG_NOTE_GONE`] — never a panic.
    pub fn edit_note(&mut self, study_id: Uuid, note_id: Uuid, text: &str) -> Result<(), String> {
        let text = text.trim().to_string();
        if text.is_empty() {
            return Err(MSG_NOTE_EMPTY.to_string());
        }
        self.require_note(study_id, note_id)?;
        let now = self.clock.now();
        self.mutate_study(study_id, move |study| {
            if let Some(note) = study.notes.iter_mut().find(|n| n.id == note_id)
                && note.text != text
            {
                note.text = text;
                note.updated_at = now;
            }
        })
    }

    /// Remove a note from the study (it stays readable in the study history, O6). A note that no
    /// longer exists is refused with [`MSG_NOTE_GONE`].
    pub fn delete_note(&mut self, study_id: Uuid, note_id: Uuid) -> Result<(), String> {
        self.require_note(study_id, note_id)?;
        self.mutate_study(study_id, move |study| {
            study.notes.retain(|n| n.id != note_id);
        })
    }

    /// The note must still exist before an edit or delete: naming its absence beats a silent no-op
    /// (review checklist §1 — a misattributed success is a lie). The read-only guard runs first so a
    /// read-only dossier refuses with its own message.
    /// A read FAILURE is named as such (#95), never passed off as a vanished note.
    fn require_note(&self, study_id: Uuid, note_id: Uuid) -> Result<(), String> {
        self.refuse_if_read_only()?;
        match self.try_get_study(study_id)? {
            Some(study) if study.notes.iter().any(|n| n.id == note_id) => Ok(()),
            _ => Err(MSG_NOTE_GONE.to_string()),
        }
    }
}
