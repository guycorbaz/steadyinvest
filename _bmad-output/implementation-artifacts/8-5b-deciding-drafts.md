# Story 8.5b: Deciding drafts

Status: ready-for-dev

<!-- Created 2026-09-28 by create-story on branch feat/8-5b-deciding-drafts (main with 8.1–8.5a).
     Autonomous Epic 8 run (Guy, 2026-09-28): questions resolved with the most conservative option,
     listed under « Decisions taken » for the final report. -->

## Story

As Guy,
I want to accept or refuse each AI proposal by my own hand, in two actions at most,
so that nothing enters my dossier without me.

## Acceptance Criteria

Source: `epics.md` Story 8.5b + the Epic 8 posture AC; UX spec `ux-ai-assistance-surfaces.md`
(validated 2026-09-27, Q1–Q16 defaults) §3.1, §3.3 « Decision dialog », §4.1, §4.2, §5.2, §5.8, §6,
§7, §8; arch §Phase 4 A6/A7/A8.

1. **Two actions, one item (FR74).** From « À traiter », a pending **cell, judgment or note** draft
   row is activatable (click, or Enter/Space when focused): it opens the decision dialog (action 1);
   « Valider » or « Rejeter » decides it (action 2). There is no bulk action anywhere. A draft-study
   row and a row of an archived study stay non-activatable in 8.5b (see Decisions 1–2).
2. **The decision dialog (spec §4.2)** — a new `ModalDialog` kind `"decision"` in the same overlay
   (scrim, centred card, focus trap, Esc cancels, scrim click cancels): title « Proposition de
   l'IA »; context line (the row target, plus « Valider ou rejeter ouvre l'étude {TICKER} ({DEV}). »
   when the draft's study is not the open one); a state band (◦ stale / ⊘ target gone / ◦ read-only);
   the grid « Actuel » | « Proposé » (numeric font, the proposed side semibold; a note draft shows
   « — » | the note text **inside an AiFrame**); the `AiFrame` with the comment (full, not compact);
   buttons « Valider » · « Rejeter » · « Modifier avant de valider… » · « Annuler ». Initial focus
   (Q5): « Annuler » when the target cell is `✓`, « Valider » otherwise.
3. **Validation (FR74, FR17, FR20, O5, D5).** « Valider » calls `JournalState::decide_draft`
   (8.2b, unchanged semantics): the value enters as an owner entry with review `?` and a visible AI
   origin; a `✓` target needs no un-validation and moves to `?`. A validated note becomes a study
   note carrying its AI origin (FR71, FR78). A validated judgment writes `ai_placed` on its field
   (rendering is 8.6).
4. **The AI-origin cell mark (spec §5.8, FR17).** A §2/§3 grid cell whose `provenance.ai_origin` is
   set shows « ★ » (`Tokens.ai-glyph`, `mark-font`, ink) in the **trailing column, bottom half** —
   the stale « ◦ » slot (they never coincide: a validated AI value is manual and `Current`; assert it
   in a test against `contract/src/cell.rs`). Focusing the cell adds, beside « Source : … », the app
   text « proposée par l'IA, validée le {JJ/MM/AAAA} » (client/model never shown outside an
   AiFrame). The owner's next edit clears the mark (`Cell::edited` replaces the provenance — 8.2b);
   the snapshot history keeps it (FR51). The confusability gate (UX-DR15) gains « ★ » against ◦, ◆
   and △ at 14 px, shown in the cell context in the headless walk.
5. **Edit before validating (FR74, FR77, D5).** « Modifier avant de valider… » switches the dialog to
   the form variant `form-id = "draft-edit"` (title « Modifier la proposition », sentence « La valeur
   enregistrée sera la vôtre ; la proposition sera notée modifiée avant validation. », button
   « Enregistrer »): a number → a `LabeledField` prefilled with the proposed value **as the grid shows
   it** (owner's number format, millions for sales / pre-tax profit); an option → a `LabeledDropdown`
   of the option labels; a note → a `LabeledTextArea` **as `@children` of the AiFrame** (prefilled
   with the AI text — exempt while inside the frame, spec §4.1). « Enregistrer » runs
   `owner_edit` + `Decision::ValidateEdited`: saved without AI origin, review `?`, the draft recorded
   `edited_before_validation` — unless the edit equals the proposal (plain validation, 8.2b G3 E2).
   A parse refusal (`MSG_VALUE_NOT_A_NUMBER`, ambiguous number, `MSG_VALUE_NOT_AN_OPTION`,
   `MSG_NOTE_EMPTY`) shows inline (`field-error`), the text intact.
6. **Another study (arch A8).** Deciding (validating **or** rejecting) a draft whose study is not
   the open one first opens that study through the ordinary open path (`Studies.open-study`, which
   resets the undo history to that study), then decides. The dialog says so beforehand (the context
   line, AC 2). The screen stays « Propositions »; the study is then the open study (its undo history
   holds the step).
7. **Stale drafts (FR72, O4, A7).** A stale draft's dialog shows the ◦ band « La cible {cible} a
   changé depuis la proposition. Valeur actuelle : {maintenant}. »; « Rejeter » decides at once;
   « Valider » (and « Enregistrer » of the edit form) first opens the confirm « Valider une
   proposition périmée ? » — body « La cible {cible} a changé depuis la proposition (valeur
   actuelle : {maintenant}). La valeur proposée la remplacera. », verb « Valider ». Confirming passes
   the fingerprint the owner **saw** (`seen_fingerprint`, computed when the dialog opened). If the
   target changed again, the rail refuses (`MSG_DECISION_CHANGED`) and the dialog is **shown again**
   with the new current value.
8. **Target gone.** A target-gone draft's dialog shows the ⊘ band « Cible disparue : {raison} ; la
   proposition ne peut qu'être rejetée. » and offers only « Rejeter » and « Annuler » (« Valider »
   and « Modifier… » absent).
9. **Undo / redo (FR32, FR77, A8).** Ctrl+Z on the study after a validation restores the prior value
   and records the draft `validated_undone` (notice « Validation annulée ; la proposition est notée
   validée puis annulée. »); Ctrl+Y re-validates it (« Validation rétablie. »). The inbox and the
   rail count follow at once (the app's own writes do not move `PRAGMA data_version` — every
   decision, undo and redo pushes the inbox explicitly). A dropped draft step
   (`MSG_UNDO_DRAFT_STEP_DROPPED`) is a refusal notice.
10. **Refusals (7.0 AC1, FR58).** Every refusal of a decision — dossier read-only, write failure,
    study deleted or archived since listing, draft gone or already decided, target changed —
    shows inside the open decision dialog as its `field-error` band (the dialog's own gesture,
    exactly as a form's submit — `dialog::route` gains `"decision"`), or as « Action refusée » when
    no dialog is open (undo/redo). Nothing is written; the inbox is re-read after the refusal (the
    row may have left).
11. **Outcomes.** A decision's outcome is an inline notice on the Propositions card (new
    `Drafts.notice` slot, F4 rule): « Proposition validée. » · « Proposition rejetée. » ·
    « Proposition validée (modifiée avant validation). ». Undo/redo outcomes go to the study's own
    notice slot (`study_notice`, a new `Source::Draft`).
12. **Read-only dossier.** On a read-only dossier the dialog opens (consultation), shows the
    existing ◦ read-only band of the overlay, and « Valider », « Rejeter » and « Modifier… » are
    disabled — the band says why (G1 decision 5 precedent, `confirm-enabled`).
13. **Study PDF (spec §7).** A cell value carrying an AI origin prints « † » (WinAnsi 0x86) after
    the figure, with one legend line under its table: « † valeur proposée par une IA et validée par
    l'utilisateur ». A judged value with an AI origin prints « * † » (judgments' AI marks come with
    8.6's `ai_placed` rendering; 8.5b prints the cell marks only). Nothing else of the PDF changes
    (byte-identical for a study without AI origins — test).
14. **AI text containment (spec §4.1, A12).** The dialog's comment, the proposed note text and the
    client/model reach Slint only through `Dialog.ai-*` properties read inside `AiFrame { … }`
    elements; the structural scan (`posture.rs`) covers them unchanged (extend its self-test with a
    stray `Dialog.ai-text` outside a frame). The Rust guard's allow-list gains the one function that
    fills them (`wiring::drafts::to_dialog`).
15. **Keyboard (NFR-U2, spec §6).** Rows Tab-reachable with the focus ring, Enter/Space opens; in the
    dialog, focus trapped, initial focus per AC 2, Enter activates the focused button, Esc cancels;
    the edit form validates with Enter (value) or Ctrl+Enter (note).
16. **Posture (Epic 8 AC).** `@tr` floor and `MSG_*` counts re-based with the delta stated; every
    new string verbatim from §3.3 (or listed under Decisions); glossary entry « Proposition validée
    par l'utilisateur » (the ★ cell mark); no banned verb; visual verification (DoD) on a temp copy
    seeded with `just mcp-seed`.

## Tasks / Subtasks

- [ ] **T1 — State: what the dialog needs (AC 2, 5, 7, 8)** — `app/src/state/drafts.rs`
  - [ ] T1.1 `pub fn seen_fingerprint(study: &Study, payload: &DraftPayload) -> Option<String>`: the
    current fingerprint of a cell/judgment draft's target (wraps the private `current_fingerprint`),
    `None` for a note / gone target. The dialog stores it at open; « Valider » on a stale draft passes
    it as `Decision::Validate { seen_fingerprint: Some(..) }`; a fresh draft passes `None` (8.2b
    contract: `None` = base fingerprint).
  - [ ] T1.2 `JournalState::draft_for_dialog(draft_id) -> Result<DialogDraft, String>`: one read of the
    draft record + its study (+ archived status), mirroring `read_inbox`'s rules (any failure named,
    never guessed). `DialogDraft` carries `DraftRef`, kind, payload, freshness, the study's `✓` state
    of the target cell (for Q5), and `open_study_differs: bool`.
  - [ ] T1.3 Undo/redo report what they stepped over: change `undo`/`redo` to return
    `Result<Stepped, String>` with `enum Stepped { Nothing, Study, Draft(Uuid) }` (was `bool`);
    update the two callers in `wiring/judgment.rs` and every test (`Ok(true)` → `Stepped::Study`/
    `Draft`). No semantic change otherwise.
  - [ ] T1.4 Tests: `seen_fingerprint` equals what `decided_study` accepts for a stale draft
    (stale → confirm with seen → validates; target moved after seen → `Changed{confirmed:true}`);
    `Stepped::Draft(id)` on undo/redo of a draft step.
- [ ] **T2 — View model (AC 2, 4, 5, 7, 8)** — `app/src/viewmodel/drafts.rs`
  - [ ] T2.1 `pub fn dialog_view(d: &DialogDraft, open_study: Option<Uuid>, format) -> DecisionView`
    (pure): target text (same as the row's), context line (other study), `current` / `proposed`
    (reuse `value_display`), state (fresh / stale / gone + the §3.3 band text with `{cible}` and
    `{maintenant}` — `{maintenant}` is the current value as displayed), `initial_focus`
    ("cancel" | "validate"), the edit prefill (number as displayed, option label, note text), the
    option labels list for an option field, the AI fields (client, model, comment, note text).
  - [ ] T2.2 Unit tests: each kind, stale/gone/fresh, millions prefill (« 1 234,5 » for 1 234 500 000
    under a comma format), option labels, the other-study context line, Q5 focus on a `✓` target.
- [ ] **T3 — Dialog UI (AC 2, 5, 7, 8, 12, 14, 15)** — `app/ui/state.slint`, `app/ui/components/modal_dialog.slint`
  - [ ] T3.1 `Dialog` gains: `decision-target`, `decision-context`, `decision-current`,
    `decision-proposed`, `decision-state` (int: 0 fresh · 1 stale · 2 gone), `decision-band`,
    `decision-note` (bool), `decision-focus` ("cancel"|"validate"), `edit-kind`
    ("number"|"option"|"note"), `edit-options: [string]`, and the AI fields `ai-client`, `ai-model`,
    `ai-text`, `ai-lead` (read ONLY inside `AiFrame`); callbacks on `Drafts`: `open-draft(string)`,
    `decide(string /* validate|reject|edit */) -> bool`, `confirm-stale()`.
  - [ ] T3.2 `ModalDialog`: the `"decision"` variant (layout per AC 2 — reuse `StatusBand`,
    `AiFrame`, `ActionButton`, the numeric font); the `"draft-edit"` form (reuses `LabeledField` /
    `LabeledDropdown` / `LabeledTextArea` — the note area as `@children` of the AiFrame); Esc/scrim
    cancel; the stale confirm uses the existing confirm kind with action `"validate-stale"` whose
    « Annuler » returns to the decision dialog (the `discard-note` resume pattern), and whose verb
    calls `Drafts.confirm-stale()`. Buttons disabled on `Holdings.read-only` (AC 12); « Valider » /
    « Modifier… » absent on a gone target (AC 8).
  - [ ] T3.3 Focus trap and initial focus per `decision-focus` (the `cancel-request` /
    `commit-request` counters).
- [ ] **T4 — Inbox rows activatable (AC 1, 15)** — `app/ui/screens/propositions.slint`
  - [ ] T4.1 Wrap `DraftRowView` of kinds cell / judgment / note (state ≠ 3 archived) in a focusable
    activator (FocusScope + TouchArea, focus ring, Enter/Space) → `Drafts.open-draft(r.id)`.
    Draft-study and archived rows unchanged (not activatable).
  - [ ] T4.2 `Drafts.notice` slot under the bands (inline outcome, F4 rule).
- [ ] **T5 — Wiring (AC 3, 6, 7, 9, 10, 11)** — `app/src/wiring/drafts.rs`, `dialog.rs`, `judgment.rs`
  - [ ] T5.1 A `DECISION` thread-local (the open dialog's `DialogDraft` + `seen_fingerprint`).
    `on_open_draft`: `draft_for_dialog` → `dialog_view` → `to_dialog` (the ONLY setter of
    `Dialog.ai-*`) → `Dialog.kind = "decision"`. A read failure → `dialog::refuse`.
  - [ ] T5.2 `on_decide`: `Dialog.gesture = true` around it (set by the Slint side, like forms); if
    the draft's study is not open → `ui.global::<Studies>().invoke_open_study(id)` first (AC 6) — the `Studies.open-study` callback of `state.slint:629` (the one that stays on the current screen), NOT the navigation global's `open-study` at `state.slint:1231` (it switches to Études); stale +
    validate/edit → raise the stale confirm (AC 7) and return false; else build the `Decision`
    (`owner_edit` for edit — map an option LABEL back to its `option_name` first) and call
    `decide_draft`. On Ok: close the dialog, `Drafts.notice` outcome, `push_form` of the open study
    (the grid shows ★), `push_drafts`. On Err: `dialog::refuse` (→ `field-error`), then re-read the
    draft: if still pending, refresh the dialog content (AC 7 « affichée de nouveau »); if gone,
    keep the refusal visible and `push_drafts`.
  - [ ] T5.3 `dialog::route`: `"decision" if gesture => FieldError` (unit test beside the existing
    routing tests).
  - [ ] T5.4 Undo/redo in `wiring/judgment.rs`: on `Stepped::Draft(_)` → `study_notice::outcome`
    (new `Source::Draft`) with the §3.3 undo/redo text, then `push_drafts`; `Err` → as today
    (`MSG_UNDO_DRAFT_STEP_DROPPED` is a refusal → `dialog::refuse`).
- [ ] **T6 — The AI-origin cell mark (AC 4)** — `app/ui/state.slint` `GridCellState`, `viewmodel/form.rs`
  `editable_cell`, `components/editable_cell.slint`, `study_screen.slint`
  - [ ] T6.1 `GridCellState.ai-validated: string` — "" or the JJ/MM/AAAA of `ai_origin.validated_at`
    (local time, `date_fr`) — never the client/model.
  - [ ] T6.2 `editable_cell.slint`: a `★` Text in the trailing column bottom half when
    `ai-validated != ""` (and `trail-col` reserves the column for it); on focus set
    `Studies.active-ai-validated`; `study_screen.slint` shows « proposée par l'IA, validée le {} »
    beside « Source : … ».
  - [ ] T6.3 Test: `Cell::validated_from_draft` yields `Freshness::Current` (the ★/◦ slot is never
    shared); the view model sets `ai_validated` only when `ai_origin` is `Some`; an owner edit clears it.
- [ ] **T7 — Study PDF (AC 13)** — `report/src/pdf.rs` (+ `report::form` if the laid-out rows need
  the provenance)
  - [ ] T7.1 `AI_SIGIL = "†"` after an AI-origin cell figure + the legend line once under the table
    that shows one; WinAnsi check (0x86). Test: a study without AI origin is byte-identical to main's
    output; one with an AI-origin sales cell prints « † » and the legend.
- [ ] **T8 — Posture & glossary (AC 14, 16)** — `app/src/posture.rs`, `messages.rs`, `settings.slint`
  - [ ] T8.1 New Rust consts (outcomes ×5, band/confirm/context texts if built in Rust) join
    `USER_FACING_MESSAGES` (delta stated). Slint `@tr` floor re-measured (tally).
  - [ ] T8.2 Extend the AiFrame scan self-test (`Dialog.ai-text` outside a frame fails) and the Rust
    guard allow-list (`to_dialog`).
  - [ ] T8.3 Glossary « Proposition validée par l'utilisateur » (★ on a cell: a value an AI proposed
    that you validated; your next edit removes the mark).
- [ ] **T9 — Verification** — gates + headless walk (below); story record with screenshot paths under
  the session scratchpad `v85b/`.

## Dev Notes

### Scope guardrails
- **No change to 8.2b semantics.** `decide_draft`, `owner_edit`, `decided_study`, `draft_freshness` are
  called, not rewritten (T1 only ADDS `seen_fingerprint`, `draft_for_dialog`, `Stepped`).
- **Not in 8.5b:** the « Registre » view and draft-study validation (8.7); the AI judgment lines,
  judgment chips, « placée par l'IA » captions and judgment PDF marks (8.6); history ★ summaries
  merged into the timeline (8.7 per spec §5.7). Decisions from the chart / study bands are 8.6.
- The app reads drafts through `Journal`, never `McpAccess` (8.5a rule).

### Files — current state → change → must preserve
- `app/src/state/drafts.rs` — 8.2b rail (`decide_draft` guards: read-only · journal · draft gone ·
  already decided · owner study (`history.owner()` must equal the draft's study) · archived · gone
  study · target gone · changed). → add T1 helpers. **Preserve** every guard and message.
- `app/src/state/undo.rs` — `undo`/`redo` return `bool`; a draft step moves the draft status in one
  transaction; a mismatched draft step is dropped with `MSG_UNDO_DRAFT_STEP_DROPPED`. → return
  `Stepped`. **Preserve** push-back on write failure, owner guard, the drop rule.
- `app/src/wiring/drafts.rs` — 8.5a read side, the only setter of `DraftRow.ai_*`; `push_drafts`,
  poller, filters. → add the dialog wiring and `to_dialog`. **Preserve** the ⊘ rules, unchanged-model
  skip, filters.
- `app/src/wiring/dialog.rs` — `route(kind, gesture)`: notice / field-error / queue. → `"decision"`.
  **Preserve** the queue (a refusal never overwrites an open dialog).
- `app/ui/components/modal_dialog.slint` — kinds notice/confirm/form; forms' submit closes only on
  success; `cancel()` resume pattern for `discard-note`; read-only band `if Dialog.kind != "notice"
  && Holdings.read-only`. → add `"decision"` and `"draft-edit"`. **Preserve** focus trap sentinels,
  Esc, scrim rules.
- `app/ui/screens/propositions.slint` — rows display-only. → activatable rows + notice slot.
- `app/ui/components/editable_cell.slint` — marker columns (lead: ?/✓ + lock; trail: △ top, ◦
  bottom), `trail-col` reservation. → ★ in trail bottom. **Preserve** the marker-space rule and
  constant geometry.
- `app/src/viewmodel/form.rs` `editable_cell` → `ai_validated`.
- `app/src/wiring/judgment.rs` undo/redo handlers (~l.545–620) — bind the result before `match`
  (the RefCell comment there). → `Stepped`.
- `report/src/pdf.rs` — judged `*` sigil + `JUDGED_SIGIL`; `winansi_byte`. → `†` for AI-origin cells.

### Flows
- **Open:** row → `open-draft(id)` → `draft_for_dialog` (fresh read; never the row's cached text) →
  `dialog_view` → Dialog props → `kind = "decision"`.
- **Validate fresh / reject:** (open study if needed) → `decide_draft` → close, notice, `push_form`,
  `push_drafts`.
- **Validate stale:** « Valider » → confirm `validate-stale` (Annuler → back to the decision dialog)
  → verb → `decide_draft(Validate{seen: Some(fp_at_open)})`.
- **Edit:** « Modifier… » → `form-id "draft-edit"` in the same overlay → « Enregistrer » →
  (stale? → the same confirm) → `owner_edit` → `ValidateEdited`.
- **Refusal:** `field-error` in the dialog; re-read; show again or keep refusal + `push_drafts`.
- **Undo/redo:** study screen Ctrl+Z/Y → `Stepped::Draft` → study notice + `push_drafts`.

### Why explicit pushes
`PRAGMA data_version` moves only on ANOTHER connection's commit (8.5a T1 test). The app's own
decisions, undos and redos never move it: without an explicit `push_drafts` the rail count and the
rows would lag until the next foreign write.

### Values and units
- Prefill and « Actuel / Proposé » use `value_display` (millions for sales/pre-tax, the owner's
  number format, option chip labels). `owner_edit` reads the typed text back with the same format
  and scale (8.2b G3 E1) — a round trip test is part of T2.2.
- Options: the dropdown shows `option_label`; map the picked label → `option_name` before
  `owner_edit` (which matches wire names).

### Testing standards
- Pure view-model tests (`viewmodel/drafts.rs`), state tests (`state/tests.rs` patterns with the
  temp default dossier — **never** `open_or_create` with the real default path; the 8.4 guard test
  enforces it), routing unit test (`dialog.rs`), posture scans, PDF byte-identity.
- No UI-level harness exists (8.5a T9.5): wiring is verified headless; say so honestly.

### Verification (headless)
Follow 8.5a's recipe (story 8.5a Dev Notes « Verification »): copy `persistence/tests/corpus/v8.db`
to the scratchpad, open once with the app (temp `XDG_CONFIG_HOME`/`XDG_DATA_HOME`, provider
« none ») to migrate, quit by PID, `just mcp-seed <copy>`, relaunch. Never a provider fetch, never
the real rfd portal, never register the MCP server, never touch `~/.config/steadyinvest` or
`~/.local/share/steadyinvest` (check before/after). Kill by PID only. Capture (`v85b/`):
1. dialog of a fresh cell draft (Actuel/Proposé, AiFrame, focus on « Valider »);
2. validate → notice + rail count −1 + the study grid cell with « ★ » and the focused-cell line;
3. a `✓` target → initial focus on « Annuler »; after validation the cell is `?` + ★;
4. note draft dialog (« — » | text in AiFrame) → validated note in the Notes card (AI origin in its
   AiFrame, 8.1);
5. judgment draft → validated (field value changed; 8.6 renders the mark);
6. edit before validating: number (comma format, millions field), option (dropdown), note;
   a refused edit (« abc ») inline;
7. stale: edit the target cell in the app after seeding → dialog band → « Valider » → confirm →
   validated; and « changed again » (edit between open and confirm) → refusal + dialog shown again;
8. target gone (payload year edited in the copy with Python) → only « Rejeter »;
9. other study: with study A open, decide a draft of study B → context line → B becomes open;
10. undo / redo on the study → notices, inbox count restored / decreased;
11. read-only copy → dialog with ◦ band, buttons disabled;
12. refusal: archive the study from another launch? — simpler: decide a draft whose study was
    deleted after listing (delete in the app, then act on a still-open dialog is not possible —
    use a second app-less writer: delete the study row with Python on the copy while the dialog is
    open) → refusal inline;
13. keyboard: Tab to a row, Enter opens, Tab through the dialog buttons, Esc;
14. ★ vs ◦ ◆ △ at 14 px in the cell context; the PDF export with « † » (use the export path
    without the rfd portal — the 7.x headless PDF env-var bypass, reverted after).

### Posture (checklist §6)
Expected: Rust `USER_FACING_MESSAGES` +5..+9 (outcomes, undo/redo, bands/confirm body/context if
formatted in Rust); `@tr` floor + ~15..25 (title, columns, buttons, edit form, stale confirm title,
focused-cell line, glossary). Measure, tally in `posture.rs`, state the delta.

### Previous-story intelligence
- 8.5a: rows/`DraftRow.ai_*` set only in `wiring/drafts.rs::to_slint`; the posture guard strips only
  the trailing test module and restricts AI-field reads to `ai_fields`, `to_slint` and the 8.2b rail —
  add `to_dialog`. The AiFrame scan is comment/string-aware, `_`≡`-`, and forbids relays
  (`property`/`<=>`) inside AiFrame elements — do not bind `Dialog.ai-*` through a local property.
- 8.5a: the Études card is over-wide (G6 debt); « Historique » clips at 1280 px in the study action
  row — do not add buttons to that row in 8.5b.
- 8.2b: `decide_draft` requires the draft's study to own the undo history (open study). `Stepped`
  must keep the G3 owner guard (`restored.id == study_id`).
- 8.1: note forms: Ctrl+Enter submits, the resume pattern for an abandoned text; reuse it for the
  stale confirm's « Annuler ».
- Builds: `CARGO_BUILD_JOBS=4`; add files by name (untracked PDFs at the repo root).

### Project Structure Notes
- All UI in `app/ui` (components/screens/state.slint), wiring in `app/src/wiring`, pure logic in
  `app/src/viewmodel`, state in `app/src/state`; the PDF in `report`. No new crate, no migration, no
  `SCHEMA_VERSION` change (not in production).

### References
- [Source: _bmad-output/planning-artifacts/epics.md#Story 8.5b]
- [Source: _bmad-output/planning-artifacts/ux-ai-assistance-surfaces.md §3.1, §3.3, §4.1, §4.2, §5.2, §5.8, §6, §7, §8, §9 Q5]
- [Source: _bmad-output/planning-artifacts/architecture.md §Phase 4 A6, A7, A8]
- [Source: app/src/state/drafts.rs, app/src/state/undo.rs, app/src/wiring/{drafts,dialog,judgment,studies}.rs]
- [Source: app/ui/components/{modal_dialog,ai_frame,editable_cell}.slint, app/ui/screens/propositions.slint]
- [Source: _bmad-output/implementation-artifacts/8-5a-ai-frame-and-draft-inbox.md, 8-2b-ai-origin-staleness-and-decisions.md]
- [Source: docs/review-checklist.md]

### Decisions taken (owner-pending, lead defaults — autonomous run)
1. **Draft-study rows stay non-activatable in 8.5b** (no « Rejeter » either): their validation and
   the « Valider… » button are 8.7's; the row keeps the « pas encore disponible » line.
2. **Rows of an archived study are not activatable**: the state word « étude archivée » explains;
   the rail would refuse both verbs (8.2b guard).
3. **Refusals inside the open decision dialog** route to its `field-error` band (the dialog's own
   gesture), like a form — the spec's §3.1 allows it; « Action refusée » when no dialog is open.
4. **Read-only: the dialog opens for consultation, verbs disabled with the band** (G1 decision 5
   precedent) — a refusal only covers the race where the dossier became read-only meanwhile.
5. **The cell provenance line is the focused-cell « Source » line**, not the verdict traceability
   overlay (which lists verdict inputs, not grid cells): « proposée par l'IA, validée le … » beside
   « Source : manuel ».
6. **The PDF « † » for cells lands in 8.5b** (the first story that can create AI-origin cells — a
   report must not launder an AI value); judgments' « † » with 8.6.
7. **Undo/redo outcomes go to the study notice slot**, decision outcomes to a new `Drafts.notice`
   on the Propositions card.
8. **The screen stays « Propositions » after deciding a draft of another study** (the study is
   opened in the background; its ★ mark shows when the owner goes to it).

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

- Ultimate context engine analysis completed - comprehensive developer guide created (2026-09-28).

### File List
