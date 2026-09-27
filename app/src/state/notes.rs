//! Study notes (Story 8.1, FR78): the owner's dated notes on a study, add / edit / delete.
//!
//! Every rail rides [`JournalState::mutate_study`]: atomic (one `put_study_with_history`, the FR51
//! snapshot included), guarded (read-only / no journal / save failure) and undoable on a real change.
//! Ids and times come from the injected [`IdGen`](crate::clock::IdGen) / [`Clock`](crate::clock::Clock)
//! (ADD15). A note is the owner's own text: normalised, trimmed, never posture-scanned (FR13 covers
//! app-generated signals only).

use uuid::Uuid;

use super::{JournalState, MSG_NOTE_EMPTY, MSG_NOTE_GONE, MSG_READ_FAILED, MSG_STUDY_GONE};
use steadyinvest_contract::{Note, Study};

/// A note's text as stored: line breaks normalised to `\n` (a pasted CRLF or lone CR never
/// survives), then trimmed; `None` when nothing visible remains — whitespace and invisible format
/// characters (Unicode Cf: zero-width spaces, bidi marks, BOM…) alone make an empty note.
pub(crate) fn normalized_note_text(raw: &str) -> Option<String> {
    let text = raw.replace("\r\n", "\n").replace('\r', "\n");
    let text = text.trim();
    if text.chars().all(|c| c.is_whitespace() || is_format_char(c)) {
        None
    } else {
        Some(text.to_string())
    }
}

/// Unicode general category Cf (format characters), the ranges that can appear in typed or pasted
/// text — std has no category table.
fn is_format_char(c: char) -> bool {
    matches!(
        c as u32,
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

impl JournalState {
    /// Add a note to the study; returns its id. An empty (or invisible-only) text is refused with
    /// [`MSG_NOTE_EMPTY`] and nothing is written.
    pub fn add_note(&mut self, study_id: Uuid, text: &str) -> Result<Uuid, String> {
        let text = normalized_note_text(text).ok_or_else(|| MSG_NOTE_EMPTY.to_string())?;
        self.require_study(study_id)?;
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

    /// Replace a note's text. The same text (after normalisation) is a no-op — nothing written, no
    /// `updated_at` bump, no undo step, no version bump; an empty text is refused with
    /// [`MSG_NOTE_EMPTY`]; a note that no longer exists (e.g. removed by an undo) is refused with
    /// [`MSG_NOTE_GONE`] — never a panic.
    pub fn edit_note(&mut self, study_id: Uuid, note_id: Uuid, text: &str) -> Result<(), String> {
        let text = normalized_note_text(text).ok_or_else(|| MSG_NOTE_EMPTY.to_string())?;
        let study = self.require_study(study_id)?;
        match study.notes.iter().find(|n| n.id == note_id) {
            None => return Err(MSG_NOTE_GONE.to_string()),
            Some(note) if note.text == text => return Ok(()),
            Some(_) => {}
        }
        let now = self.clock.now();
        self.mutate_study(study_id, move |study| {
            if let Some(note) = study.notes.iter_mut().find(|n| n.id == note_id) {
                note.text = text;
                note.updated_at = now;
            }
        })
    }

    /// Remove a note from the study (it stays readable in the study history, O6). A note that no
    /// longer exists is refused with [`MSG_NOTE_GONE`].
    pub fn delete_note(&mut self, study_id: Uuid, note_id: Uuid) -> Result<(), String> {
        let study = self.require_study(study_id)?;
        if !study.notes.iter().any(|n| n.id == note_id) {
            return Err(MSG_NOTE_GONE.to_string());
        }
        self.mutate_study(study_id, move |study| {
            study.notes.retain(|n| n.id != note_id);
        })
    }

    /// The study a note rail writes into, read before the write so every refusal names its real
    /// cause: read-only first, then a READ failure (#95 — never passed off as an absence), then a
    /// study deleted meanwhile.
    fn require_study(&self, study_id: Uuid) -> Result<Study, String> {
        self.refuse_if_read_only()?;
        match self.try_get_study(study_id) {
            Ok(Some(study)) => Ok(study),
            Ok(None) => Err(MSG_STUDY_GONE.to_string()),
            Err(_) => Err(MSG_READ_FAILED.to_string()),
        }
    }
}
