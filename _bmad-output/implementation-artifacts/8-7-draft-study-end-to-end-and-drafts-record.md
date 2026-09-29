# Story 8.7: Draft study end-to-end & drafts record

Status: review

<!-- Created 2026-09-29 by create-story on branch feat/8-7-draft-study-end-to-end (main with 8.1–8.6).
     Autonomous Epic 8 run (Guy, 2026-09-28): questions resolved with the most conservative option,
     listed under « Decisions taken » for the final report. -->

## Story

As Guy,
I want a validated AI ticker to become an ordinary study, and a record of every proposal and its fate,
so that the AI widens my search without a special path, and I can look back on what it proposed.

## Acceptance Criteria

Source: `epics.md` Story 8.7 (l.1317–1340) + the Epic 8 posture AC (l.1088–1093); UX spec
`ux-ai-assistance-surfaces.md` (validated 2026-09-27, Q1–Q16 defaults) §3.1, §3.2, §3.3, §4.1,
§4.2, §5.2, §5.7, §6, §8, Q14; arch §Phase 4 D2, D8, O7, A4, A7, A8, A9, A12; FR13, FR49, FR51,
FR55, FR64, FR65, FR70, FR74, FR76, FR77, NFR-A3, NFR-U2; PRD Journey 6 (l.424–441) and « AI
Assistance Outcomes » (l.261–274).

### A. Draft study end-to-end

1. **« Valider… » on a draft-study row (Q14).** In « À traiter », a pending draft-study row carries
   an `ActionButton` « Valider… » (focusable, key-repeat ignored, `guard-reflex` like the 8.5b
   decision verbs) in place of the line « La validation d'une proposition d'étude n'est pas encore
   disponible ; la proposition reste en attente. », which is removed. It opens the ordinary
   `study-create` form directly, in its **draft variant** (AC 2). Disabled on a read-only dossier
   (the ◦ band explains).
2. **Draft variant of the create form (spec §4.2).** Same form-id `study-create`, same fields and
   enable rule, prefilled with the proposed ticker, currency and company name (set **after**
   `Dialog.form` resets the `draft-*` fields), with the sentence « Étude proposée par une IA ;
   vérifiez le symbole, la devise et le nom avant de créer. ». The **name field and the AI comment
   are wrapped in an `AiFrame`** (`@children`, header « ★ IA — Proposée par {client} ({modèle}) le
   {date} » + disclaimer); ticker and currency stay plain (exempt, §4.1). The AI name reaches Slint
   only through `ai-*` properties (the AiFrame structural guard must see it); the editable field's
   text is the owner's once « Créer » is pressed (FR74). The submit button stays « Créer ».
   *Corrected at dev (Decision 11): the ordinary form's verb is « Enregistrer » — kept; « Créer »
   in the epics names the action, not the label.*
3. **« Créer » = one transaction (arch A8, FR70, D2).** Confirming creates a new, empty study
   (ticker as typed, currency upper-cased, name blank → absent — the ordinary `create_study_named`
   rules) with its FR51 creation snapshot, and records the draft `validated` with `decided_at` and
   `created_study_id` = the new study — **in one SQLite transaction**, one `data_version` bump.
   Never half-applied (NFR-R2). The study is **not** added to the watchlist; **no provider call**
   (FR76). 2 actions: « Valider… » → « Créer » (spec §8 AC l.417–419).
4. **Duplicate check again (FR70, D2).** Inside that transaction, before any write: if the dossier
   holds a study (archived included, 8.3 Decision 1) with the same ticker (trimmed, ASCII
   case-insensitive — the MCP `same_identifier` rule, shared, not re-implemented) **and** the same
   currency, nothing is written and the form shows, inline in its `field-error` band (§3.1), « Une
   étude {TICKER} en {DEV} existe déjà ; rien n'a été créé. » with the **existing** study's ticker
   and currency spelling. The draft stays pending. The check runs on the values in the form at
   « Créer » (Decision 2).
5. **Cancelling** (« Annuler », Esc, scrim) leaves the draft pending and writes nothing.
6. **The draft changed meanwhile.** If at « Créer » the draft is no longer pending (decided or
   deleted by another writer), nothing is created and the form's `field-error` shows the 8.2b refusal
   « Cette proposition a déjà été traitée ; rien n'a été enregistré. » (existing message); the inbox
   re-reads. Read-only dossier → the existing read-only refusal.
7. **Rejecting a draft study (Journey 6).** Activating a draft-study row (click / Enter / Space on the
   row, as for other kinds) opens the 8.5b decision dialog in a **draft-study variant**: context line
   « Nouvelle étude : {TICKER} ({DEV}) », « Actuel » « — » / « Proposé » « {TICKER} ({DEV}) », the
   proposed company name as `ai-lead` and the comment in the dialog's AiFrame, and the actions
   « Valider… » (closes the dialog and opens the draft create form, AC 2), « Rejeter », « Annuler »
   — no « Modifier avant de valider… » (the create form is the edit). « Rejeter » uses the existing
   rail (`write_rejection`, outcome « Proposition rejetée. »): 2 actions. Q5 initial focus on
   « Annuler » (conservative — a rejection is final for a draft study, Decision 1).
8. **Not on the undo stack (A8, O7).** A draft-study validation is not undoable; the owner reverses it
   by deleting the created study through the existing « Supprimer » confirmation, which already
   deletes its drafts, the draft study included (`Journal::delete_study`, O7 — verified, not
   rebuilt). Ctrl+Z after « Créer » does not touch it (it is not on any study's stack).
9. **Normal path after creation (FR76, NFR-A3).** The created study is an ordinary study: no provider
   call on validation; the owner fetches it as for any study; only then is its data readable through
   MCP. An **end-to-end test on a temp dossier** covers: submit (via `McpAccess`) → validate (app
   state) → MCP read shows the empty study → **stubbed** owner fetch (`apply_provider_refresh` with
   `fetched_for`) → MCP read shows the fetched figures; no special step; no provider reachable from
   MCP.
10. **Second draft study refused (FR70).** Once the study exists, a draft study for the same security
    and currency is refused by MCP (`study_exists`, D2 — existing; covered by the E2E test).
11. **Outcome and refresh.** After « Créer »: the form closes, the Propositions notice shows
    « Proposition validée. » (or « Proposition validée (modifiée avant validation). » when the owner
    changed ticker, currency or name — Decision 2), the inbox and the Études list re-read (the app's
    own write does not move `data_version`, A9), the ★ band count drops. The screen stays
    « Propositions » (8.5b Decision 8); the new study is not opened automatically (the ordinary
    create path does not open it either).

### B. Drafts record (« Registre »)

12. **View chips (spec §3.3, §5.2).** The Propositions card gets view chips « À traiter » ·
    « Registre » (Tab-reachable, selected state visible). « À traiter » is today's inbox, unchanged.
13. **Registre content (FR77).** **Every** draft of the dossier, pending included, read-only, newest
    submission first. Row: submitted date (JJ/MM/AAAA), target (the §3.3 target strings, « Nouvelle
    étude : {TICKER} ({DEV}) » for a draft study), outcome word — « en attente » (+ « · périmée » or
    « · cible disparue » for a pending draft, from `draft_freshness`), « validée », « rejetée »,
    « validée puis annulée » — with the flags « périmée à la décision » and « modifiée avant
    validation » when set, and the decided date. Row button « Détail » / « Masquer le détail »
    expands the origin + comment + proposed value / note text / company name **inside an
    `AiFrame`** (compact), proposed values app-formatted. Draft studies validated show « → étude
    créée » only through their target (no link needed, Decision 6).
14. **Filters.** The kind chips (shared with « À traiter »), « Étude : » (drop-down, « Toutes les
    études ») and the outcome chips « En attente » · « Validées » · « Validées puis annulées » ·
    « Rejetées » (none selected = all; one at a time, Decision 5). « Étude : » matches a draft's
    `study_id` **or** `created_study_id` (Decision 4). Filter-empty text « Aucune proposition ne
    correspond à ce filtre. » (8.5a); dossier-empty « Aucune proposition dans ce dossier. ».
15. **States.** No loading state (local read). A failed read shows the ⊘ band « Les propositions
    n'ont pas pu être lues ; la liste est indisponible ({cause}). » — never an empty list (checklist
    §1). Read-only dossier: the Registre is fully readable. The record is read when the Registre is
    shown and re-read when `data_version` moves while it is shown (the 8.5a poller), never on every
    poll otherwise (8.5a rule: the decided record is not read on every poll).
16. **Keyboard (NFR-U2, §6).** Tab: view chips → kind / study / outcome filters → rows; « Détail »
    toggles with Enter/Space; focus stays on the toggled row.
17. **MCP (FR77, 8.4).** `get_drafts_record` already returns the record with outcomes; its `study_id`
    filter is extended to `created_study_id` too (Decision 4; tool description updated), so the AI
    and the owner see the same record for a study. Tested.

### C. Study history

18. **Processed drafts in the history (FR49, FR51, A12, spec §5.7).** A study's history timeline
    merges its processed drafts (status ≠ pending, `study_id` = the study **or** `created_study_id` =
    the study), each at its `decided_at`, with the app-text summaries « ★ Proposition validée :
    {cible} » · « ★ Proposition validée (modifiée) : {cible} » · « ★ Proposition rejetée : {cible} » ·
    « ★ Proposition validée puis annulée : {cible} » ({cible} = the §3.3 target without the ticker
    prefix: « {champ} · {année} », « {champ} », « nouvelle note », « Nouvelle étude : {TICKER}
    ({DEV}) »). A rejected draft appears although it wrote no snapshot. At the same instant as a
    snapshot, the ★ entry sorts **after** it (the snapshot is the effect, the ★ entry names its
    cause — Decision 7). The snapshot entries are unchanged (plain value-change lines).
19. **Détail of a ★ entry.** Toggling it shows the AI-written parts (comment; proposed note text;
    proposed company name) inside an `AiFrame` with its header and disclaimer, and app-formatted
    « Actuel → Proposé » values plain. The « Masquer les notes » toggle does not hide ★ entries
    (they are not note edits).
20. **History read.** A new persistence read `Journal::list_study_drafts(study_id)` (processed only,
    `study_id = ?1 OR created_study_id = ?1`, ordered by `decided_at, id`); a failed read shows the
    history's existing « indisponible » state, never a timeline silently missing its ★ entries.

### D. Journey 6, FR65, posture

21. **Journey 6 walked end-to-end** (headless, temp dossier copy seeded with `just mcp-seed`, extended
    to submit **three** draft studies): reject one draft study, validate two (« Valider… » →
    « Créer »), a stubbed/fabricated fetch is **not** driven headless (safety rule) — the walk shows
    the created empty studies and the E2E test (AC 9) covers the fetch → MCP read; a note and a
    lower forecast P/E (judgment) on an existing study; growth-judgment lines (8.6): one validated,
    one rejected. The Registre and the histories show every outcome. The PRD « AI Assistance
    Outcomes » (8 criteria) are checked off in the story record with the test or screen that proves
    each.
22. **FR65.** The app remains fully usable with no MCP server registered: every screen works on a
    dossier with no drafts (Registre empty text, no ★ band, history without ★ entries) — a test on a
    fresh dossier + the walk.
23. **Posture (Epic 8 posture AC).** New strings from §3.3 verbatim; `@tr` floor, `MSG_*`
    (`USER_FACING_MESSAGES`), `HISTORY_USER_FACING_LABELS`, `DRAFTS_USER_FACING_LABELS`,
    `DECISION_USER_FACING_LABELS` re-based with the measured delta; banned-verb gate; AiFrame
    structural scan + Rust AI-field guard green (the proposed company name in the create form and the
    Registre / history Détail are `ai-*` inside `AiFrame`s); glossary entry « Registre » (« La liste
    de toutes les propositions d'une IA et de leur issue : en attente, validée, validée puis
    annulée, rejetée. »); headless visual verification (DoD).

## Tasks / Subtasks

- [x] **T1 — Persistence: validate a draft study (AC 3, 4, 6).** `persistence/src/drafts.rs`:
  `Journal::validate_draft_study(&mut self, w: DraftStudyValidation { draft_id, study: Study
  (built by the app), edited: bool, now }) -> Result<(), DraftError>` — one transaction:
  re-read the draft (pending, kind study, else `AlreadyDecided` / `NotFound`); duplicate check via
  the shared identifier rule (move `same_identifier` from `mcp_access.rs:645` to a `pub(crate)`
  helper both use; the check scans every study incl. archived) → `DraftError::StudyExists { ticker,
  currency }` with the existing study's spelling; `write_study_with_snapshot` (studies.rs:79);
  `UPDATE ai_drafts SET status='validated', decided_at, created_study_id, edited_before_validation`;
  one bump. The 8.2a CHECKs hold by construction.
  - [x] Tests in `persistence/tests/draft_decisions.rs` (or a new `draft_study_validation.rs`):
    creates study + snapshot + draft facts atomically; duplicate (case / archived) refused with
    nothing written; decided meanwhile refused; FK/CHECK never violated; one bump; delete of the
    created study cascades the draft (O7, reuse the existing cascade test pattern).
- [x] **T2 — App state (AC 3–8, 11).** `app/src/state/drafts.rs`: `validate_draft_study(&mut self,
  draft_id, ticker, currency, company_name) -> Result<Uuid, String>`: read-only refusal; the same
  trimming / upper-casing / blank rules as `create_study_named` (extract a shared builder, do not
  copy); `edited` = any of ticker (case-insensitive), currency, name differs from the proposal;
  map `StudyExists` → new `MSG_DRAFT_STUDY_EXISTS` « Une étude {} en {} existe déjà ; rien n'a été
  créé. », `AlreadyDecided` → existing message. Not pushed on any undo stack. `decide_draft`'s
  study-kind `Validate` branch stays refused (the create form is the only validation path);
  update the test `a_draft_study_can_be_rejected_here_but_not_validated` doc accordingly.
  - [x] `read_record()` → every draft (`Journal::list_drafts`) + studies + archived for the targets;
    `list_study_drafts(study_id)` for the history (T5).
  - [x] State tests: validate → study created, not in the watchlist, draft validated with
    `created_study_id`, no undo entry; edited flag; duplicate refusal; cancel = nothing; reject.
- [x] **T3 — UI: draft create form + row (AC 1, 2, 5, 7, 11).**
  - [x] `state.slint` `Dialog`: `draft-study-id`, `ai-client`, `ai-model`, `ai-text` (comment),
    `ai-lead` (proposed name, for display), `ai-submitted`, and a `public function form-draft-study(...)`
    that calls `form("study-create", …)` then sets the prefill (the reset order matters). `Drafts`
    callback `validate-study(draft-id) ` opens it from Rust (the AI fields are set only in
    `wiring/drafts.rs`, module rule).
  - [x] `modal_dialog.slint` `study-create`: when `Dialog.draft-study-id != ""`, the sentence and the
    name field + comment inside `AiFrame { … @children }`; submit calls
    `Drafts.create-from-draft(id, ticker, currency, name) -> bool` (stays open on refusal, like
    `create-study`).
  - [x] `propositions.slint`: the study row gets « Valider… » (`ActionButton`, `guard-reflex`),
    remove the placeholder line; the row becomes activatable → decision dialog draft-study variant.
  - [x] `viewmodel/drafts.rs` `dialog_view` + `wiring/drafts.rs` `to_dialog`: the draft-study variant
    (context line, Actuel « — », Proposé « {TICKER} ({DEV}) », actions « Valider… » / « Rejeter » /
    « Annuler », initial focus « Annuler »); « Valider… » in the dialog → close + open the create form.
- [x] **T4 — Registre (AC 12–17).**
  - [x] `viewmodel/drafts.rs`: `RecordFilter { kind, study, outcome }`, `record_rows(drafts, studies,
    archived, format, filter) -> Result<RecordView, Unshowable>` (pure, tested): submitted date,
    target, outcome word + pending freshness word, flags, decided date, and the Détail fields
    (`ai_fields` stays the one AI-text reader). Study filter on `study_id` or `created_study_id`.
  - [x] `state.slint` `Drafts`: `view` (« inbox » / « record »), `record-rows`, `outcome-filter`,
    `pick-view`, `pick-outcome`, `toggle-record-detail`; `RecordRow` struct.
  - [x] `propositions.slint`: view chips, outcome chips (Registre only), record rows with « Détail »
    and the compact AiFrame; ⊘ band / empty texts; keyboard order.
  - [x] `wiring/drafts.rs`: read the record on entering the Registre and on a `data_version` move
    while shown; reset view + filters on a dossier change and on the rail (8.5a G3 item 5 rule).
  - [x] MCP: `DraftFilter.study_id` matches `created_study_id` too (`mcp_access.rs` query_drafts
    `WHERE (?1 IS NULL OR study_id = ?1 OR created_study_id = ?1)`); `get_drafts_record` description;
    test in `persistence/tests/mcp_access.rs`.
- [x] **T5 — History merge (AC 18–20).** `persistence`: `Journal::list_study_drafts(study_id)`.
  `viewmodel/history.rs`: `HistoryItem` = snapshot entry | draft entry; `history_entries` merges by
  (timestamp, snapshot-before-draft, id); new labels « ★ Proposition validée : {} » etc. into
  `HISTORY_USER_FACING_LABELS`; draft entries never `notes_only`. `wiring/push.rs::push_history`
  + `wiring/studies.rs::on_toggle_history_entry`: an entry id prefix (e.g. `draft:`) routes Détail
  to a draft branch (AiFrame lines); `study_screen.slint` history Détail renders the AiFrame for a
  draft entry.
- [x] **T6 — Seed + E2E (AC 9, 10, 21).** `mcp/examples/seed.rs`: submit three draft studies
  (distinct tickers, e.g. SEED.A / SEED.B / SEED.C in USD, refusal-tolerant). App test (temp dossier):
  `McpAccess::submit_draft` (draft study) → `validate_draft_study` → `McpAccess::read_study` (empty,
  no years) → `apply_provider_refresh(id, &fetched_for(..))` → `read_study` (the fetched years) →
  a second `submit_draft` refused `study_exists`; `list_drafts` shows `validated` +
  `created_study_id`; NFR-A3 stays covered by the existing whole-surface test (cite it).
- [x] **T7 — FR65 + posture (AC 22, 23).** A test: a fresh dossier with no drafts reads an empty
  record, no band, no ★ history entry, and every screen's view model builds. Posture deltas
  measured and stated; glossary « Registre ».
- [x] **T8 — Headless walk (AC 21, DoD).** Temp HOME/XDG, provider « none », fresh copy of
  `persistence/tests/corpus/v8.db` (+ the g01 DEMO studies if needed for charts), `just mcp-seed`:
  the steps of AC 21; Registre with every outcome + filters + Détail; a history with ★ entries
  (validated, rejected, undone) and its Détail; the create form's AiFrame; duplicate refusal (create
  the same ticker by hand first); cancel; keyboard; 1280 and 1600. Screens in the session scratchpad
  `v87/`. Real `~/.config/steadyinvest` + `~/.local/share/steadyinvest` checked unchanged.
- [x] **T9 — Record.** Story record, AI-assistance success criteria checklist with evidence, posture
  deltas, decisions, sprint-status 8-7 → review.

## Dev Notes

### What exists (do not rebuild)

- **Create path**: `dashboard.slint:281-290` opens `Dialog.form("study-create", …)`; `state.slint:1201`
  `form()` **resets every `draft-*`** — prefill after it. Form markup `modal_dialog.slint:999-1024`
  (Symbole / Devise des chiffres dropdown / Nom de société), enable rule `:236`, submit `:281-282`
  (`Studies.create-study(...) -> bool`, stays open on refusal). Wiring `wiring/studies.rs:342-365`:
  `create_study_named` → `refresh_studies` (which also pushes the inbox). State
  `state/studies.rs:95-132`: trims, blank refusals, read-only, `currency.to_uppercase()`, ticker kept
  as typed, empty name → `None`, `put_study_with_history`. **No duplicate check on the ordinary
  path** (keep it so — Decision 3). No watchlist write, no fetch.
- **Drafts persistence**: `persistence/src/drafts.rs` — `DraftRecord` (:33, has `created_study_id`),
  `list_drafts` (:332, all), `list_pending_drafts` (:346), `get_draft` (:360), `decide_draft`
  (:376 — cannot validate a draft study: `DraftStudyMismatch` / CHECK), `step_draft_decision`
  (:434, sets `decided_at = now` on undo/redo). Schema CHECKs `schema.rs:149-197`: draft study ⇔
  `study_id IS NULL` and `native_currency IS NOT NULL`; never `validated_undone`; `validated` ⇔
  `created_study_id`. `write_study_with_snapshot` (`studies.rs:79`, `pub(crate)`).
- **App drafts state**: `state/drafts.rs` — `decide_draft` (:411; study kind: only `Reject` →
  `write_rejection`), `precheck_decision` (:580), `draft_for_dialog` (:631), `read_inbox` (:758,
  pending only — « the decided record is 8.7's, never read on every poll »).
- **Inbox UI**: `propositions.slint` — row activatable only for cell/judgment/note (:92-93),
  placeholder line (:72-77), target « Nouvelle étude : {} » (:27), AiFrame `ai-lead` + `ai-text`
  (:78-85), « Nouvelles études » group (:245), kind chips (:162-186), « Étude : » (:187-205),
  comment at :160 « The « Registre » view chip arrives with 8.7 ». Rows built by
  `viewmodel/drafts.rs::inbox_rows` (:406); `ai_fields` (:258) is the ONE AI-text reader.
  `Drafts` global `state.slint:1806-1860`; `wiring/drafts.rs` is the ONLY place `ai_*` are set.
- **Band / navigation**: `dashboard.slint:262-276`, `on_show_draft_studies` (`wiring/drafts.rs:771`).
- **O7 cascade**: `Journal::delete_study` (`persistence/src/studies.rs:378-411`) deletes drafts by
  `study_id OR created_study_id`; test `persistence/tests/drafts.rs:700`. UI confirm
  `wiring/studies.rs:865-950`, `MSG_DELETE_CONFIRM`. **Done — verify only.**
- **MCP**: 8 tools (`mcp/src/tools.rs:26-35`); `get_drafts_record` (:212-233, handler :601-620,
  DTO `dto.rs:187-218` incl. `created_study_id`); `McpAccess::list_drafts(DraftFilter{study_id,
  status}, page)` (`mcp_access.rs:881`, query :1471-1504 filters `study_id` only); D2 `study_exists`
  (:1093-1116, archived included, `same_identifier` :645), D8 `draft_study_pending` (:1117-1145).
- **History**: `viewmodel/history.rs` — labels (:24-47), `HISTORY_USER_FACING_LABELS` (:52, 20),
  `HistoryEntryView` (:77), `history_entries` (:428), `visible_history` (:467), `history_detail`
  (:491); wiring `push.rs:211-260`, toggle `wiring/studies.rs:449` (predecessor by snapshot index);
  Slint `study_screen.slint` ~620-670, `HistoryEntryRow` `state.slint:604-610`.
- **Fetch stub**: `JournalState::apply_provider_refresh` (`state/refresh.rs:78`), fixtures
  `fetched_for` (`state/tests.rs:121`). MCP from app tests: `verify.rs:383-460` uses `McpAccess::at`.
- **Test helpers**: `state/tests.rs` module `drafts_8_2b` (:8791) — `decision_state`, `raw`, `plant`
  (study kind → CHF, ticker `'NESN'` hard-coded — add a ticker parameter for draft studies),
  `facts`, `dref`.
- **Posture baseline (main after 8.6)**: `@tr` floor **1113**; `USER_FACING_MESSAGES` **247**;
  `HISTORY_USER_FACING_LABELS` **20**; `DRAFTS_USER_FACING_LABELS` **6**;
  `DECISION_USER_FACING_LABELS` **4**; `AI_LINES_USER_FACING_LABELS` **1**.

### Guardrails

- One transaction for create + draft update (A8, NFR-R2). No provider call anywhere in 8.7 code paths
  reachable from validation; no new MCP write surface; MCP stays read-only for the record.
- The shared identifier rule is **moved**, not duplicated (MCP and the app must never disagree).
- AI text (comment, company name, note text, client, model) reaches Slint only via `ai-*` inside an
  `AiFrame` (8.5a rule, posture guard). The prefilled editable name is `@children` of the AiFrame.
- Registre reads `Journal::list_drafts` only when shown (8.5a rule); computation view models never
  read drafts (`persistence/tests/drafts.rs` guard).
- Safety (Epic 8 preamble): headless on temp copies, temp HOME/XDG, provider « none », no provider
  fetch driven headless, never register the MCP server, never touch the real config or data dirs.
  The seed runs in the real environment (its guard reads the real config to refuse the real
  dossier). `CARGO_BUILD_JOBS=4`; never `git add -A`.
- Chart / layout lessons from 8.6: count every row in a `min-height` under an `if`; a repeater over
  a new model drops focus — sync models in place when a focused row can be redrawn (Registre rows
  on a poll re-read: keep one model, sync rows); names starting `ai-` are reserved for AI text.

### Decisions taken (owner-pending, for the final report)

1. **Rejecting a draft study**: the row opens a draft-study variant of the decision dialog
   (« Valider… » / « Rejeter » / « Annuler », initial focus « Annuler »); the spec gave the row only
   « Valider… » (Q14) but Journey 6 and the AC require a rejection. 2 actions either way.
2. **The owner may edit ticker, currency, name in the prefilled form**: the study is created with the
   form's values, the duplicate check runs on them, the draft keeps its proposal and is marked
   `edited_before_validation` when any differs (ticker compared case-insensitively); the outcome
   then reads « Proposition validée (modifiée avant validation). ».
3. **No duplicate check added to the ordinary create path** (unchanged behaviour; FR70 asks it for
   draft validation only).
4. **« Étude : » filters (Registre and MCP `get_drafts_record`) match `study_id` or
   `created_study_id`**, so a study's record includes the draft study that created it; a pending or
   rejected draft study appears under « Toutes les études » only.
5. **Outcome chips select one outcome at a time**, none = all; pending sub-states (fresh / stale /
   target gone) are shown on the row, not filtered.
6. **Registre rows are not links** to the created study (read-only record; the Études list has it).
7. **History order at equal timestamps**: snapshot first, then the ★ entry; a `validated_undone`
   draft sits at its `decided_at` (the undo time, 8.2b Decision 5) with « validée puis annulée ».
8. **Deleting the created study deletes the draft-study record** (O7 / FR55, consistent across docs):
   the Registre then no longer lists that proposal.
9. **The submitted date shows on Registre rows** (spec §5.2) although 8.5a shows it only in the
   AiFrame header for « À traiter » rows (8.5a Decision 8 kept there).
10. **Journey 6's provider fetch is not driven headless** (safety rule): the E2E test covers it with a
    stubbed fetch; the walk stops at the created empty studies.
11. **The draft create form keeps the ordinary verb « Enregistrer »** (the « ordinary create-study
    dialog »); the epics' « Créer » names the action.
12. **The study pick of « Étude : » is shared by both views**; the Registre keeps a pick whose study
    has only decided drafts (« À traiter » drops it, 8.5a rule); « Études » kind clears it (both).
13. **A study id already in the dossier is refused** by `validate_draft_study` (never an upsert) —
    found by a test whose fixed id generator reused the open study's id.
14. **« Proposé {} » in a ★ history Détail** is the proposed value only (the value before the
    decision is in the neighbouring snapshot entry).

### Testing standards

Pure view-model functions unit-tested in their module; persistence integration tests in
`persistence/tests/`; state tests in `app/src/state/tests.rs`; MCP tests in
`persistence/tests/mcp_access.rs` / `mcp/tests`; gates: `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `cargo deny check`.

### Previous-story intelligence (8.5a / 8.5b / 8.6)

- Refusals inside an open form go to its `field-error` (8.5b Decision 3); outcomes to
  `Drafts.notice` (Decision 7); the decision verbs are `guard-reflex` (350 ms, no key repeat).
- The app's own write does not move `data_version`: re-read after every in-app decision (A9).
- The inbox filters reset on the rail and a dossier change (8.5a G3 item 5) — extend to the view.
- Headless: resize the window first, focus it before keys, partial-repaint artefacts under Xvfb are
  not bugs until a full repaint confirms them (8.6 walk).

### Project Structure Notes

New: `Journal::validate_draft_study`, `Journal::list_study_drafts`, `RecordRow`/record view model,
maybe `persistence/tests/draft_study_validation.rs`. Touched: `persistence/src/{drafts,mcp_access,
studies}.rs`, `app/src/state/{drafts,studies,messages,tests}.rs`, `app/src/viewmodel/{drafts,
history}.rs`, `app/src/wiring/{drafts,push,studies}.rs`, `app/ui/{state.slint, components/
modal_dialog.slint, screens/propositions.slint, screens/study_screen.slint, screens/settings.slint}`,
`app/src/posture.rs`, `mcp/src/tools.rs`, `mcp/examples/seed.rs`.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 8.7]
- [Source: _bmad-output/planning-artifacts/ux-ai-assistance-surfaces.md#3.3, #4.1, #4.2, #5.2, #5.7, #8, #9 Q14]
- [Source: _bmad-output/planning-artifacts/architecture.md#Phase 4 D2, D8, O7, A4, A7, A8, A9, A12]
- [Source: _bmad-output/planning-artifacts/prd.md#Journey 6, #AI Assistance Outcomes, FR70, FR74, FR76, FR77]
- [Source: _bmad-output/implementation-artifacts/8-2a-drafts-table.md, 8-2b-…, 8-3-…, 8-4-…, 8-5a-…, 8-5b-…, 8-6-…]
- [Source: docs/review-checklist.md]

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5)

### Debug Log References

- The posture « no error Display » scan follows every name bound in an `Err(...)` pattern through
  the file: destructuring `ticker` / `currency` out of `DraftStudyExists` flagged every later use
  of `ticker` → only `study_id` is bound; the existing study is read back to name it.
- A commit (bb85ecf) went in with the AI-field guard failing (a `grep` pipeline hid the test exit
  code); fixed in the next commit (1c9e6d0). Gates are now read from the summary line.

### Completion Notes List

- T1 persistence (b0e9a64, 4e30bd9): `Journal::validate_draft_study` (one transaction, one bump;
  duplicate re-check with the MOVED `same_identifier` / `study_with_identifier`, shared with MCP
  D2; id-collision guard), `Journal::list_study_drafts`, `Error::DraftStudyExists`; MCP
  `get_drafts_record`'s study filter also matches `created_study_id`. 8 persistence tests.
- T2 state (4e30bd9): `validate_draft_study` through `new_study_from_form` (extracted from
  `create_study_named`), `MSG_DRAFT_STUDY_EXISTS`; `read_record`, `try_list_study_drafts`,
  `try_get_draft`.
- T3 UI (5514987, 2009482): draft-study decision variant, « Valider… » row button, the draft
  variant of `study-create` (sentence, AiFrame around the name + comment), `create-from-draft`.
- T4 Registre (bb85ecf, 1c9e6d0, 0ccfc9d): view chips, outcome chips, rows + « Détail », synced
  model, read only while shown.
- T5 history (6331972): ★ entries merged, Détail in an AiFrame.
- T6/T7 (ba7cf4e): end-to-end MCP test, FR65 test, glossary « Registre ».
- **Posture deltas (measured)**: `@tr` floor 1113 → 1146 (+33, itemised in `posture.rs`);
  `USER_FACING_MESSAGES` 247 → 248 (`MSG_DRAFT_STUDY_EXISTS`); `HISTORY_USER_FACING_LABELS` 20 →
  26 (four ★ summaries, « nouvelle note », « Nouvelle étude : {} ({}) »); `DRAFTS_USER_FACING_LABELS`
  6 and `DECISION_USER_FACING_LABELS` 4 unchanged (the new words are `@tr` on the Slint side); the
  AI-field guard knows `record_to_slint` and `history_ai_to_slint`. Gates: fmt, clippy
  `-D warnings`, `cargo test --workspace` 1433 passed, deny ok.
- **T8 headless walk — Journey 6** (Xvfb :94, temp HOME / XDG, provider « none », real
  `~/.config/steadyinvest` + `~/.local/share/steadyinvest` listed + md5 before/after — unchanged).
  Fresh copy of `persistence/tests/corpus/v8.db` migrated by one launch, + DEMO1 / DEMO2 (golden
  g01, Python), `just mcp-seed` (three draft studies, a note, every chart judgment on DEMO1, a
  growth proposal on DEMO2). Screens in the session scratchpad `v87/`:
  - first pass 01–09: draft studies listed with « Valider… » (02: the button covered the target →
    fixed, 03), the prefilled form with the AiFrame (04), created (05, « Proposition validée. »),
    the draft-study decision (06, focus « Annuler »), rejected (07), a duplicate refused inline
    (08: SEED.A edited to « seed.b » → « Une étude SEED.B en USD existe déjà ; rien n'a été
    créé. »), cancel keeps it pending (09);
  - Registre 10–12 (view chips; columns aligned by stretch; « Détail » in an AiFrame);
  - Journey 6, 20–41: the Études ★ band → « Études » kind (21), SEED.C rejected by keyboard
    (22–23), SEED.B and SEED.A validated « Valider… » + Enter (24–25), the two new EMPTY studies
    in the list, the watchlist empty (26–27); DEMO1: est-high validated from its chip (caption),
    est-low rejected (28–30), low P/E validated then Ctrl+Z (32, « validée puis annulée »), the
    note validated (33); Registre filtered on DEMO1 with every outcome (34), « Études » kind (35:
    SEED.C rejected, SEED.A/B validated, the corpus' NESN validated, ASML pending), « Rejetées »
    (36); DEMO1 history with the ★ entries after their snapshots (37) and a ★ Détail in its
    AiFrame (38); Registre at 1280 (39) and by keyboard (40–41: Tab to « Détail », Enter).
  - Not driven headless: the provider fetch (safety rule) — the E2E test covers submit →
    validate → stubbed fetch → MCP read.
- **Found and fixed in the walk**: the « Valider… » button took the row's full width (outside a
  layout); the inbox rows lost their spacing inside the new view wrapper; the Registre's columns
  followed their text widths (target / outcome / dates); the « Détail » label change shifted them.
- **Seen, not changed (pre-existing)**: the study screen is wider than the window once « Masquer
  l'historique » is shown (the toolbar), so the history's « Détail » buttons sit at the edge and
  the page scrolls sideways on focus — the G6 width debt noted in 8.5b.
- **PRD « AI Assistance Outcomes » (l.261–274), checked off**:
  1. 100 % of MCP writes outside the inbox rejected and logged — `mcp/tests/closure.rs`, the
     `persistence/tests/mcp_access.rs` authorizer suite (8.3 / 8.4), unchanged and green.
  2. No MCP resource returns portfolio / watchlist / keys / config — the 8.4 whole-surface test.
  3. No provider call reachable from MCP — `mcp/tests/closure.rs` (NFR-A3); 8.7 adds no MCP write.
  4. A pending draft changes no computed output — the 8.3 + 8.6 metamorphic suites (`verify.rs`).
  5. Every draft carries a non-empty comment and an origin — the 8.2a CHECKs + 8.3 checks.
  6. ≤ 2 actions, current and proposed side by side — the 8.5b dialog; 8.7: « Valider… » +
     « Enregistrer » (walk 24–25), « Rejeter » from the row's dialog (22–23).
  7. A study from a validated draft follows the normal path — `a_draft_study_goes_end_to_end_…`.
  8. Every AI-text surface carries the label and the disclaimer — the AiFrame structural scan and
     the Rust AI-field guard (now incl. the create form, the Registre and the history Détail).
- No UI-level harness exists: the wiring is verified headless only.

### File List

New: `persistence/tests/draft_study_validation.rs`. Modified: `persistence/src/{drafts,error,lib,
mcp_access,studies}.rs`, `mcp/src/tools.rs`, `mcp/examples/seed.rs`, `app/src/state/{drafts,
studies,messages,tests}.rs`, `app/src/viewmodel/{drafts,history}.rs`, `app/src/wiring/{drafts,
push,studies}.rs`, `app/src/posture.rs`, `app/ui/{state.slint, components/modal_dialog.slint,
screens/propositions.slint, screens/study_screen.slint, screens/settings.slint}`,
`_bmad-output/implementation-artifacts/sprint-status.yaml`.

## Change Log

- 2026-09-29 — create-story (cc247fd); dev-story T1–T9 (b0e9a64 … 0ccfc9d); headless walk v87/;
  status → review.
