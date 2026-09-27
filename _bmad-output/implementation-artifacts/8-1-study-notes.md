# Story 8.1: Study notes

Status: done

<!-- Created 2026-09-27 by the create-story workflow (fork of the G2 session). Source of truth for
     scope: epics.md Story 8.1 + the Epic 8 posture AC; UX: ux-ai-assistance-surfaces.md §3.3 « Notes »
     / « History », §4.3, §5.6, §5.7, §6, §7 (validated by Guy 2026-09-27, Q1–Q16 defaults);
     architecture §Phase 4 A5 (notes in the blob), A6 (types), A12 (history noise).
     OWNER DECISION 2026-09-27 (Guy): « steadyinvest n'est pas en production : pas besoin de migrer
     l'existant. » → NO SCHEMA_VERSION bump, no re-stamp, no import relaxation (see Dev Notes). -->

## Story

As Guy,
I want to attach free notes to a study and create, edit or delete them,
so that my thinking around a study lives beside it (and an AI note draft later has a home).

## Acceptance Criteria

1. **Storage (FR78, NFR-R3, arch A5).** Given an open study, when I add, edit or delete a note
   through the dialogs of AC 6, then the note is stored inside the study blob as
   `#[serde(default)] notes: Vec<Note>` with `Note { id, text, created_at, updated_at, ai_origin:
   Option<AiOrigin> }` — a plain additive field: **no `user_version` migration, no `SCHEMA_VERSION`
   bump** (owner decision, see Dev Notes); a study saved before this story opens unchanged with an
   empty note list (free through `serde(default)` — one cheap test keeps it so).
2. **`AiOrigin` defined here (arch A6).** This story defines the contract type
   `AiOrigin { draft_id, client, model, validated_at }` (used by 8.2b). `Note.ai_origin` is always
   `None` in 8.1.
3. **History (FR49, FR51, O6).** Each note change is saved through the normal study upsert
   (`put_study_with_history`) and appears in the study's history; a deleted note disappears from
   the study but remains readable in the study history (its text is shown in the entry's detail).
4. **History noise (arch A12).** Note-only history entries — identified by comparing consecutive
   snapshots with `notes` ignored (history rows carry no cause column) — are summarised « Note
   ajoutée » / « Note modifiée » / « Note supprimée » and can be hidden with the chip « Masquer les
   notes » / « Afficher les notes » (default: shown), so judgment changes stay readable.
5. **No compatibility work (owner decision 2026-09-27).** `SCHEMA_VERSION` stays 1; saves do not
   re-stamp `schema_version`; the strict import checks stay as they are. The arch A5 / D9 version
   bump is dropped for this story (planning docs aligned by the lead, not here).
6. **Entry UI (UX §5.6, §4.3, 7.0 AC1).**
   - The notes live in a `PanelCard` « Notes » directly below « Justification de la décision »
     (`RationaleNote` unchanged).
   - « Ajouter une note… » opens the form `note-add`, and the row button « Modifier… » opens the form
     `note-edit`. Both are titled dialogs with a multi-line `LabeledTextArea`: **Enter inserts a
     newline, Ctrl+Enter submits**, Esc cancels.
   - An empty note is refused inside the form (`field-error`) with « La note est vide ; rien n'a été
     enregistré. », and the text is kept.
7. **Delete (UX-DR25).** « Supprimer » asks for confirmation (« Supprimer la note ? » / « La note
   quitte l'étude ; elle reste lisible dans l'historique. » / verb « Supprimer ») and is undoable
   within the session (Ctrl+Z / Ctrl+Y cover add, edit and delete).
8. **Round-trip (FR59, FR60, NFR-R5).** A single-study and a whole-dossier export/import round-trip
   preserves notes byte-identically, including their ids and timestamps.
9. **Keyboard (NFR-U2).** Every notes control is Tab-reachable with the visible focus ring; the
   forms trap focus like every 7.0 dialog.
10. **No AI (Epic 8 scope rule).** No MCP code, no draft, no AI label, no `AiFrame`; `ai_origin`
    exists in the type but is always `None`.
11. **Neutrality (FR13).** A note text is plain owner text, not subject to the banned-verb gate
    (FR13 covers app-generated signals only). Every system string — labels, buttons, summaries,
    refusals — is posture-scanned.
12. **Report impact (8.0 §7, Q10).** Notes are **not printed**. The study PDF of a study with notes
    is byte-identical to the same study without notes (asserted).
13. **Epic 8 posture AC** (epics.md, Epic 8 preamble):
    - The `@tr` floor and the `USER_FACING_MESSAGES` count are re-based, with their delta stated in
      `posture.rs` (review checklist §6).
    - Every new string is copied verbatim from the 8.0 wording list (§3.3).
    - New terms get a glossary entry.
    - An IO failure renders « indisponible » with its cause, never an empty-looking notes card.
    - A refused action opens « Action refusée ».
    - Visual verification (DoD) on a **plain temp dossier copy**. `just mcp-seed` does not exist yet;
      it arrives in 8.4.

## Tasks / Subtasks

- [x] **T1 — Contract types** (AC 1, 2)
  - [x] T1.1 `contract/src/study.rs`: add `Note { id: Uuid, text: String, created_at: Timestamp, updated_at: Timestamp, #[serde(default)] ai_origin: Option<AiOrigin> }`; add `#[serde(default)] pub notes: Vec<Note>` on `Study` **after `company_name`, before `created_at`**; `Study::new` sets `notes: Vec::new()`.
  - [x] T1.2 New `AiOrigin { draft_id: Uuid, client: String, model: String, validated_at: Timestamp }` in `contract` (e.g. `contract/src/ai.rs`), re-exported from `lib.rs`; doc-comment it as a *validated* value's origin (A6; `DraftOrigin` is 8.2b's).
  - [x] T1.3 **`SCHEMA_VERSION` stays 1** — do not touch `versioning.rs`, `export.rs` import checks, or `upsert_study_row`.
  - [x] T1.4 Add `notes: Vec::new()` to every `Study { … }` struct literal — the compiler lists them (Dev Notes § Literal sites).
- [x] **T2 — Pinned shape (corpus gate)** (AC 1, NFR-R3)
  - [x] T2.1 `persistence/tests/corpus_gate.rs` gate 1: re-capture `PINNED_CANONICAL_STUDY_JSON` with the new `"notes":[]` (canonical study keeps `notes: vec![]`), and add a doc paragraph in the precedent style (Story 2.2 / 3.4 / `company_name`): an additive `#[serde(default)]` field, **not** a `SCHEMA_VERSION` bump (owner decision 2026-09-27: not in production, no migration of existing data).
  - [x] T2.2 Gate 2 (`v1.db`) must still read back equal via `serde(default)` — no new corpus file, the frozen file untouched.
  - [x] T2.3 **Phantom-history guard (small, recommended).** `put_study_with_history` dedups on the RAW payload string (`persistence/src/studies.rs:122-130`). A study last saved before this story has no `"notes"` key in its latest snapshot, so the first value-identical re-save after the upgrade (e.g. a no-change refresh) would append a snapshot that reads « autres champs modifiés ». Compare structurally instead: parse the latest payload as `Study` and skip when it equals the study being saved (A→B→A still keeps three rows). This is a one-line-of-logic change, not compatibility work; if the dev judges it out of scope, record the one-off phantom entry as accepted in the completion notes.
- [x] **T3 — App state: note rails** (AC 1, 3, 6, 7)
  - [x] T3.1 `app/src/state/cells.rs` (next to `set_rationale`) or a new `app/src/state/notes.rs`:
    - `add_note(study_id, text) -> Result<Uuid, String>`, `edit_note(study_id, note_id, text) -> Result<(), String>`, `delete_note(study_id, note_id) -> Result<(), String>`, all through `mutate_study` (atomic, guarded, undoable-on-real-change).
    - Ids come from the injected `IdGen` and times from the injected `Clock` (ADD15 — never `Uuid::new_v4`).
    - The text is trimmed; empty → refuse with `MSG_NOTE_EMPTY`.
    - An edit that changes nothing is a no-op: no `updated_at` bump and no undo step (the `before != study` guard does this only if `updated_at` is set **after** the equality check on the text).
    - An unknown note id → `MSG_SAVE_FAILED`-class refusal, never a panic; name it (§1 "misattribution is a lie": the note is gone, e.g. after an undo).
  - [x] T3.2 `app/src/state/messages.rs`: `MSG_NOTE_EMPTY = "La note est vide ; rien n'a été enregistré."`, plus a named refusal for an unknown or vanished note if needed. Register them in `USER_FACING_MESSAGES`.
  - [x] T3.3 Newest first: order at render time (the view model sorts by `created_at` desc, then id); the stored order is insertion order.
- [x] **T4 — View model + push** (AC 3, 4, 6)
  - [x] T4.1 `app/src/viewmodel/` (new `notes.rs`): `NoteRowView { id, meta, text }`, where `meta` is « {JJ/MM/AAAA} », plus « · modifiée le {JJ/MM/AAAA} » when `updated_at != created_at`. A small `date_fr(&Timestamp) -> String` helper; the app shows AAAA-MM-JJ elsewhere, and the spec wording uses JJ/MM/AAAA — see Questions.
  - [x] T4.2 `app/src/wiring/push.rs` `push_form`: push `Studies.notes` (a `[NoteRow]` model) next to `rationale` (push.rs:65-67).
  - [x] T4.3 **History** — `app/src/viewmodel/history.rs`:
    - add a notes facet to `Diff`: added / edited / deleted, keyed by **note id** (the §2 discriminator rule, never by position);
    - summaries « Note ajoutée » / « Note modifiée » / « Note supprimée » (new `HIST_NOTE_*` consts in `HISTORY_USER_FACING_LABELS`);
    - detail lines « Note ajoutée : {texte} », « Note modifiée : {avant} → {après} », « Note supprimée : {texte} ». These are user text, so the deleted note stays readable (O6);
    - include the notes facet in the `other` guard (a note change never reads « autres champs modifiés »);
    - `HistoryEntryView` gains `notes_only: bool`.
  - [x] T4.4 `push_history` (push.rs:175-221): filter out `notes_only` entries when `Studies.history-hide-notes` is true, then **recompute `first_of_day` after filtering**. Details still diff against the true predecessor (the unfiltered list) — keep `toggle-history-entry`'s lookup on the unfiltered listing (studies.rs:415+).
- [x] **T5 — Slint UI** (AC 6, 7, 9)
  - [x] T5.1 `app/ui/state.slint` `Studies`:
    - `in-out property <[NoteRow]> notes` (struct `NoteRow { id, meta, text }`);
    - `callback add-note(string) -> bool`, `callback edit-note(string, string) -> bool`, `callback delete-note(string)`;
    - `in-out property <bool> history-hide-notes` plus `callback toggle-history-notes()`.
    - `Dialog` gains `draft-text` (reset in `form()`).
  - [x] T5.2 `app/ui/components/modal_dialog.slint`:
    - new `LabeledTextArea` (UX §4.3): the `RationaleNote` box behaviour — 3–8 rows, then it scrolls. Enter = newline, Ctrl+Enter = `accepted()`, Esc bubbles to the card's cancel;
    - forms `note-add` / `note-edit` with `can-submit` = `Dialog.draft-text != ""`, dispatched to `Studies.add-note` / `edit-note(Dialog.target-id, …)`, closing only on `true`;
    - confirm action `delete-note` in `run-confirm`, `confirm-title` and the verb function;
    - initial focus on the text area; the focus trap as for every form.
  - [x] T5.3 `app/ui/screens/study_screen.slint` — a `PanelCard` « Notes » right after `RationaleNote` (line 1188):
    - « Ajouter une note… » (`ActionButton`) and the empty text « Aucune note pour cette étude. »;
    - rows showing meta, text (3 lines, then « Afficher tout » / « Réduire »), « Modifier… » (opens `note-edit` prefilled via `Dialog.draft-text` + `target-id`) and « Supprimer » (confirm);
    - buttons disabled on the demo (`Studies.demo-active`) and on a read-only dossier, like the other study actions.
  - [x] T5.4 History panel (study_screen.slint:478-547): a `ChoiceChip`/`ActionButton` « Masquer les notes » / « Afficher les notes » under the title, bound to `toggle-history-notes`.
  - [x] T5.5 Wiring (`app/src/wiring/judgment.rs`, mirroring `on_set_rationale` at 83-106): the note callbacks follow `current_study` → state rail → on `Ok`, `study_notice::clear` + `push_form`, returning `true`. On `Err`, route the refusal into the form's `field-error` (the `Dialog.gesture` rule) or `dialog::refuse`, and return `false`. `toggle-history-notes` flips the property and re-runs `push_history`.
- [x] **T6 — Glossary & posture** (AC 11, 13)
  - [x] T6.1 Glossary/help entry « Notes » (where « Justification de la décision » is glossed): a note is dated, stays in the history when deleted, is not printed.
  - [x] T6.2 Probe and set the `@tr` floor (`posture.rs:747`, `total >= N`) and the exact `USER_FACING_MESSAGES` count (`posture.rs:867`, currently 224), and extend the running-tally comment with the delta (checklist §6).
- [x] **T7 — Tests** (all ACs; see Dev Notes § Tests)
- [x] **T8a — Headless verification (done)**: temp copies of the test dossier, provider « none »:
  add / edit / delete, empty-note refusal, history labels + « Masquer les notes » + deleted text in
  the detail, a >8-line note (caret kept in view), Esc / « Annuler » → « Abandonner la note en
  cours ? » and back, unchanged text closes directly, scrim click ignored, Tab leaves the text area
  (no tab character), mouse drag selects, « Afficher tout » kept across a re-push, the read-only
  dossier (write-protected copy: add / edit / delete disabled) and the demo study (add disabled).
- [ ] **T8b — Guy's on-display check** of the Notes card and dialogs (pending).

## Dev Notes

### Scope guardrails
- **No AI.** 8.1 ships no MCP code, no `ai_drafts`, no `AiFrame`, no ★ glyph. `AiOrigin` is defined but only ever `None`; do not render it.
- **No migration, no version work (owner decision, Guy 2026-09-27: « steadyinvest n'est pas en
  production : pas besoin de migrer l'existant. »).** Notes live in the blob (A5) as a plain
  additive `#[serde(default)]` field, exactly like `company_name` (2026-07-12). `user_version` stays
  7, `SCHEMA_VERSION` stays 1, saves do not re-stamp, the strict import checks
  (`contract/src/export.rs:87`, `persistence/src/export.rs:158, :317`) stay strict. This drops the
  arch A5 / D9 `SCHEMA_VERSION` bump (and its re-stamp / import-relaxation mechanics) for 8.1; the
  lead aligns A5/A6/A13 and stories 8.2b/8.8 in the planning docs — not part of this story.
  Consequence accepted: an older build would import a notes-bearing export and drop the notes
  silently; nobody runs an older build.
- **Do not touch `RationaleNote`** (the owner's single free-text « why »). Notes are a separate, dated list (UX §1).

### Files — current state → change → must preserve

- **`contract/src/study.rs`** (213 lines)
  - Current state: `Study`; `company_name` was added 2026-07-12 as a plain additive field (the precedent this story follows).
  - Change: add `notes` + `Note`.
  - Must preserve: `Study` keeps tolerating unknown fields (no `deny_unknown_fields`); existing tests.
- **`contract/src/export.rs`, `persistence/src/export.rs`, `contract/src/versioning.rs`**: **no change** (owner decision). Notes travel inside the `Study` blob, so export/import round-trips carry them for free; verify it with tests.
- **`persistence/src/studies.rs`**: only the T2.3 dedup comparison (structural instead of raw string), if taken. Must preserve: `ON CONFLICT DO UPDATE` (never `INSERT OR REPLACE`), the journal-identity guard, one transaction with the `logical_version` bump, content-derived snapshot ids, the `NewerRowSchema` read gates, A→B→A keeping three rows.
- **`app/src/state/cells.rs`**
  - Current state: `mutate_study` (cells.rs:548-587) — re-read → apply → `put_study_with_history` → record undo only if `before != study`.
  - Change: the note rails ride it.
  - Must preserve: the read-only / no-journal / save-failure guards; undo only on a real change.
- **`app/src/state/undo.rs`**: no change expected. It snapshots whole `Study` clones, so notes are undone for free. Must preserve: reset on open; cap 100.
- **`app/src/viewmodel/history.rs`**
  - Current state: `Diff { cells, judgment, rationale_changed, years_added, other }`, with `other = no named facet && prev != next`.
  - Change: add the notes facet; include it in the `other` guard.
  - Must preserve: consecutive-snapshot diffs (nothing stored twice); the posture inventory `HISTORY_USER_FACING_LABELS`; the `named()` ellipsis.
- **`app/src/wiring/push.rs`**
  - Current state: `push_form` sets the rationale (65-67); `push_history` builds the rows and marks « indisponible » on any read failure (#95).
  - Change: the notes model; the history filter.
  - Must preserve: the #95 discipline — a failed read never shows an empty list.
- **`app/ui/components/modal_dialog.slint`** (761 lines)
  - Current state: three kinds; `can-submit` / `dispatch-submit` per `form-id`; `run-confirm` closes first, then acts; the focus trap sentinels; Esc cancels; "a destructive verb never fires on a plain Enter".
  - Change: `LabeledTextArea`, two forms and one confirm action.
  - Must preserve: all of the above. **Enter must NOT submit the note form** (it inserts a newline); only Ctrl+Enter does.
- **`app/ui/screens/study_screen.slint`**: `RationaleNote` at 1188 and the history panel at 478-547. Must preserve: `RationaleNote` unchanged; the history panel's « indisponible » and empty texts.

### Note-only history detection
- History rows are full-study snapshots with no cause column (A12). The `notes_only` flag is computed in the view model: `diff.notes` non-empty **and** every other facet empty. The `other` guard must include the notes facet (a note change is a named facet, never « autres champs modifiés »).
- Filtering happens at push time; `first_of_day` is recomputed on the filtered list. The detail of an entry always diffs against its real predecessor.

### Literal sites (T1.4)
The `Study { … }` struct literals (the ones setting `schema_version:`) are in `contract/src/{study.rs,export.rs}` (tests), `persistence/src/export.rs` (tests), and `persistence/tests/{readonly_protected,corpus_gate,watchlist,export,e2e_lifecycle,readonly_newer,journal_roundtrip}.rs`. Other `-> Study {` hits in `app`/`report`/`core` are function signatures, and `core`'s golden type is `GoldenStudy` — not affected. Add `notes: Vec::new()`; do not introduce `..Default::default()` (`Study` has no `Default`, on purpose).

### Tests (T7)
- **contract**: `Note` / `Study` round-trip; a JSON without `notes` parses with `notes: []` (the one cheap legacy test).
- **persistence**: existing suites unchanged and green; the corpus gates (T2). If T2.3 is taken: a study whose latest snapshot predates `notes` re-saved unchanged writes **no** new history row, and A→B→A still keeps three rows.
- **Round-trip byte-identical (AC 8)**: a study with two notes (one edited, `updated_at ≠ created_at`) → single-study export → import → equal, **and** the re-export string is byte-identical. Same through the whole-journal export/import.
- **app state**
  - add/edit/delete: persisted, ordered, trimmed;
  - the empty note refused, nothing written;
  - an edit with the same text records no undo step;
  - undo/redo of add, edit and delete;
  - read-only refused up front;
  - the deleted note's text present in the history detail;
  - `IdGen`/`Clock` determinism (fixed ids).
- **history view model**: summaries and details for added, edited and deleted notes; `notes_only` true only for pure note changes; a note change never yields `HIST_OTHER`; the filter plus the `first_of_day` recompute.
- **report**: the PDF bytes for a study with notes equal the bytes without (AC 12).
- **posture**: new `@tr` strings and `MSG_*` scanned, counts exact.
- **corpus gates** (T2).
- **Legacy**: a study stored before this story opens with `notes == []` (the corpus `v1.db` gate does this).

### Posture (checklist §6)
`@tr` floor at `app/src/posture.rs:747` (`total >= 1012` today) and `USER_FACING_MESSAGES` exact count at `:867` (224 today). Probe by setting the floor absurdly high, read the measured total, set it exactly, and extend the running-tally comments with « 8.1: +n ».

### Previous-story intelligence
- **7.0 (dialogs)**:
  - forms close only on a written result; a form's refusal lands in `field-error` only while `Dialog.gesture` is set;
  - `run-confirm` reads the action and target **before** `close()`;
  - focus is trapped with sentinels; initial focus goes through the 30 ms timer (deferred workaround, keep it);
  - Slint cannot return focus to the opener (accepted).
  - Headless: resize to the Xvfb screen and `xdotool windowfocus --sync` before typing.
- **G1 reviews**:
  - never show an IO failure as an empty state (#95) — a failed `get_study` during the notes push must not render « Aucune note pour cette étude. »; mark the card « indisponible »;
  - a read failure on a write rail is `MSG_READ_FAILED`, not « L'enregistrement a échoué. » (`state/mod.rs::read_error`).
- **#34 history**: undo/redo is recorded in history honestly (no special case).
- **Rationale precedent (2.10)**: user text is never posture-scanned; it is trimmed, and empty means absence.

### Project Structure Notes
- The `app` crate holds state (`src/state`), view models (`src/viewmodel`), wiring (`src/wiring`) and Slint (`ui/`). New module files follow `snake_case.rs`; Slint properties follow `kebab-case`.
- No new crate, no new dependency.

### References
- [Source: _bmad-output/planning-artifacts/epics.md#Story 8.1] and the Epic 8 preamble and posture AC
- [Source: _bmad-output/planning-artifacts/ux-ai-assistance-surfaces.md §3.3 Notes/History, §4.3, §5.6, §5.7, §6, §7, §9 Q10]
- [Source: _bmad-output/planning-artifacts/architecture.md §Phase 4 A5, A6, A12]
- [Source: persistence/tests/corpus/README.md] — the corpus rules (no new file: additive field)
- [Source: docs/review-checklist.md §1, §2, §3, §5, §6, §7]

### Questions for Guy (defaults applied unless he objects)
1. **Date format of note meta.** The spec's §3.3 says « JJ/MM/AAAA », but the app shows AAAA-MM-JJ elsewhere (dashboard, history day headers). Default: follow the validated spec (JJ/MM/AAAA) for notes only.
2. **History detail lines for notes.** « Note ajoutée : {texte} », « Note modifiée : {avant} → {après} », « Note supprimée : {texte} » follow the existing « label : avant → après » pattern but are not in §3.3. Default: adopt them.
3. **Notes card read failure.** §3.3 has no string for it. Default: « Les notes n'ont pas pu être lues ; elles sont indisponibles. », mirroring the history panel's « L'historique n'a pas pu être lu ; il est indisponible. ».

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5), fork of the G2 session, 2026-09-27.

### Debug Log References

- Slint binding loop on first build: an `if root.long:` toggle fed the note text's wrap width back
  into the horizontal layout → the « Afficher tout » row is always present, collapsed to height 0.
- Posture « bare literal » gate caught the invisible three-line measuring probe → digits only,
  allow-listed (`"0n0n0"`, the extractor drops `\n`).
- On screen: the note text area overlapped the form buttons (a `FocusScope` is no layout) → explicit
  height (commit « fix(8.1): the note text area sizes its form »).

### Completion Notes List

- Ultimate context engine analysis completed - comprehensive developer guide created (2026-09-27).
- Owner decisions applied: no `SCHEMA_VERSION` bump / re-stamp / import relaxation (Guy
  2026-09-27, « pas en production »); the story's three questions took their defaults (JJ/MM/AAAA,
  the three history detail lines, the read-failure text).
- T2.3 taken: `put_study_with_history` dedups structurally (parse the latest payload as `Study`);
  a pre-notes snapshot never forces a phantom entry; A→B→A still keeps three rows.
- **Deviation (T5.2):** `can-submit` of the note forms is `true`, not `draft-text != ""` — an empty
  note must reach Rust to be refused WITH its reason in `field-error` (AC 6); a greyed silent button
  would hide it (checked on screen).
- **Addition:** `MSG_NOTE_GONE` « Cette note n'existe plus dans l'étude ; rien n'a été enregistré. »
  (not in §3.3) for an edit/delete of a note removed meanwhile (e.g. by an undo); a READ failure on
  that check reports the read failure, never « n'existe plus ».
- The read-failure text is a Slint `@tr` string (shown when the post-write re-read fails); no Rust
  const needed.
- Posture: `@tr` floor 1012 → 1031 (+19), `USER_FACING_MESSAGES` 224 → 226 (+2), history label
  inventory 17 → 20 (+3), each with its tally comment.
- Tests: workspace green (app 537 unit tests incl. 6 state + 3 history + 2 notes-view new; contract,
  persistence corpus/journal/export additions; report PDF byte-identity); clippy `-D warnings` and
  `fmt --check` clean.
- Verification (headless, provider « none », temp copy of the test dossier): empty card, add form
  (Enter = newline, Ctrl+Enter saves), edit form prefilled, empty-note refusal inside the form,
  newest-first rows with « modifiée le », long note clamped with « Afficher tout », delete confirm,
  history « Note ajoutée / modifiée / supprimée », the deleted text in the detail, « Masquer les
  notes » hides them with the day header recomputed. Undo/redo, read-only and demo states are
  covered by unit tests, not driven on screen.
- Seen, not in scope: at 1600 px, opening the history makes the study's action row (« Masquer
  l'historique ») overflow and the screen scroll sideways — the known G6 width debt, not this story.
  Also: the #252 method-change band disappears after the first note write, as after any edit
  (`study_notice::clear(Source::Edit)` path, pre-existing).
- Guy's on-display check of the Notes card and dialogs still to do (DoD).

- **G3 review (3 layers, no high) — all 17 items applied (2026-09-27):**
  - rails: CRLF/CR → LF; whitespace + Unicode Cf only = empty; read failure → `MSG_READ_FAILED` and
    deleted study → new `MSG_STUDY_GONE` before any write (add included); identical edit writes
    nothing (no version bump); no study open → new `MSG_NO_STUDY_OPEN` through `dialog::refuse`;
  - dedup: the latest payload re-serialized, compared as a string (a Money scale change recorded);
  - history: « note ajoutée / modifiée / supprimée » **lower-cased** like every other history clause
    — a wording adjustment of the 8.0 §3.3 list (« Note ajoutée… »), **for Guy**; no « (n) » count;
  - notes card: note dates in the machine's **local** time (history day headers stay UTC,
    app-wide, out of scope); « modifiée le » in a counted posture inventory; « Afficher tout »
    kept across re-pushes (a row flag in the model) and no Tab stop on a short note;
  - a failed re-read after a write mirrors undo/redo and marks an open history unavailable;
  - « Masquer les notes » resets to shown on study open and dossier switch (owner default);
  - text areas (note form + rationale): caret kept in view, the offset clamped when the box grows,
    mouse drag selects (Flickable not interactive, wheel still scrolls); Cmd+Enter = Ctrl+Enter;
  - note forms: scrim click ignored; Esc / « Annuler » with a changed text asks « Abandonner la note
    en cours ? » (« Annuler » returns to the text, « Abandonner » leaves) — owner default;
  - posture: `@tr` 1031 → 1033 (+2), `USER_FACING_MESSAGES` 226 → 228 (+2), notes labels 0 → 1.
  - Deviations D-a (« Enregistrer » always enabled) and D-b (`MSG_NOTE_GONE`) kept, accepted
    pending Guy.
- Headless note: fast `xdotool` typing can leave the software renderer a frame behind under Xvfb (a
  blank box until the next event) — a capture artefact, the state after any event is correct.

### File List

- contract/src/ai.rs (new), contract/src/lib.rs, contract/src/study.rs, contract/tests/roundtrip.rs
- persistence/src/studies.rs; persistence/tests/{corpus_gate,export,journal_roundtrip,e2e_lifecycle,readonly_newer,readonly_protected,watchlist}.rs
- app/src/state/notes.rs (new), app/src/state/{mod,messages,tests}.rs
- app/src/viewmodel/notes.rs (new), app/src/viewmodel/{mod,history}.rs
- app/src/wiring/{judgment,push,studies}.rs
- app/src/posture.rs
- app/ui/state.slint, app/ui/components/modal_dialog.slint, app/ui/components/rationale_note.slint, app/ui/screens/study_screen.slint, app/ui/screens/settings.slint
- report/src/pdf.rs

### Change Log

- 2026-09-27: implemented (commits 0f67a4b, 65e2a93, 460378b); status → review.
- 2026-09-27: G3 review applied (06a02f2, 0a60129, text-area clamp commit); headless re-check; T8 split (8b pending).
