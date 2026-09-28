# Story 8.5a: AI frame and draft inbox (read)

Status: ready-for-dev

<!-- Epic 8 [P4] — AI assistance over MCP (human-gated drafts). First UI story reading drafts.
     Branch: feat/8-5a-draft-inbox (main with 8.1, 8.2a, 8.2b, 8.3, 8.4 merged). -->

## Story

As Guy,
I want to see every AI proposal in one inbox, framed as AI, with current and proposed values side by side,
so that I know what the AI proposed before I decide anything.

## Acceptance Criteria

1. **Inbox, rail, reminders (FR73, O2, arch A9, spec §5.1–5.4).** Given pending drafts in the dossier
   (submitted while the app was open or closed), when I open the app or drafts arrive while it is
   open, then:
   - a sixth nav destination « Propositions » (between « Revue » and « Réglages ») lists them in one
     `PanelCard` « Propositions » (view « À traiter »), grouped by study (group header
     « {TICKER} ({DEV}) », newest group first), draft studies last under « Nouvelles études », rows
     newest first;
   - the rail label reads « Propositions · {n} » while n > 0 pending, « Propositions » when none,
     « Propositions · ⊘ » when the last poll or read failed — the top-bar title stays plain
     « Propositions » (it reads `destinations[current-screen]`, `app.slint:87`);
   - every open study with pending drafts shows the ★ reminder band in its pinned area (verdict bar →
     [8.8 strip slot] → ★ band → notice slot) with the action « Voir les propositions »;
   - the Études card shows the ★ band « {n} proposition(s) d'étude de l'IA en attente. » + « Voir les
     propositions » while draft studies are pending, and each list row of a study with pending drafts
     carries « ★ {n} » after its name;
   - new drafts appear within ~3 s via `PRAGMA data_version` polling (a Slint `Timer`) and on opening
     the inbox — no file watcher, no dialog/toast opens by itself.
2. **Row content (FR72, FR73, spec §5.2).** Each row shows: its target as app text (« {TICKER} ·
   {champ} · {année} » cell, « {TICKER} · {champ} » judgment, « {TICKER} · nouvelle note » note,
   « Nouvelle étude : {TICKER} ({DEV}) » draft study); « Actuel {x} → Proposé {y} » (note: « Nouvelle
   note »); the state word (« périmée » when stale, « cible disparue » when the target is gone — per
   `draft_freshness`, 8.2b); the submitted date JJ/MM/AAAA (local time); and the compact `AiFrame`
   (AI origin client + model, the comment on one elided line, the disclaimer).
3. **Draft studies (spec §3.3).** A draft-study row shows ticker and currency as app text and, inside
   the compact `AiFrame`, « {IA:nom} — {IA:commentaire} » (no name: the comment alone), and the line
   « La validation d'une proposition d'étude n'est pas encore disponible ; la proposition reste en
   attente. » (8.7 replaces it with « Valider… »).
4. **AiFrame containment (FR13, FR64, arch A12, spec §4.1).** All AI-written text (comment, proposed
   note text, proposed company name, origin client, origin model) is rendered only through the new
   `AiFrame` component (label « ★ IA — Proposée par … », disclaimer on every instance), and a
   structural test scans `app/ui/**/*.slint` and fails if an `ai-*` property/field is read outside an
   `AiFrame { … }` element or `ai_frame.slint`, with the §4.1 exemptions listed and justified in the
   test.
5. **Unreadable ≠ empty (arch A9, checklist §1).** A failed poll or drafts read shows the ⊘ band « Les
   propositions n'ont pas pu être lues ; la liste est indisponible ({cause}). » on the Propositions
   card, the rail « · ⊘ », the study reminder switches to its ⊘ variant (« Les propositions de cette
   étude n'ont pas pu être lues ; état indisponible. ») — never an empty inbox, never a vanishing
   band; the last good count is not kept.
6. **Concurrency (FR67 [P4], NFR-R2, arch A9).** A test holds a read transaction open on a
   DELETE-mode dossier from a second connection (the shape of an MCP read) while the app's `Journal`
   commits a study edit: the commit succeeds within the app's `busy_timeout` (5000 ms), and the edit
   is persisted.
7. **Empty & keyboard (FR58, NFR-U2).** The empty « À traiter » view shows « Aucune proposition à
   traiter. Une IA enregistrée comme client MCP peut en déposer ici ; rien n'est appliqué sans votre
   validation. »; a read-only dossier shows the ◦ band « Dossier en lecture seule : les propositions
   sont consultables, aucune décision n'est possible. ». The rail item, kind chips, the « Étude : »
   filter and every band action button are Tab-reachable with a visible focus ring and operable with
   Enter/Space.
8. **Epic 8 posture AC.** `@tr` floor and `MSG_*` counts re-based with their delta stated; every new
   string French and verbatim from spec §3.3 (or recorded as a decision); new terms get a glossary
   entry; banned-verb gate passes; « indisponible » with its cause; visual verification on a temp
   dossier seeded with `just mcp-seed`.

## Tasks / Subtasks

- [ ] **T1 — Persistence: `data_version` (AC 1, 5)**
  - [ ] T1.1 `Journal::data_version(&self) -> Result<i64>` = `PRAGMA data_version` on the app's own
        connection (`persistence/src/journal.rs`, next to `logical_version`). Doc: changes only when
        ANOTHER connection commits (A9); the app's own writes do not move it.
  - [ ] T1.2 Unit test: a second connection's commit moves it; the owning connection's own commit
        does not.
- [ ] **T2 — App state: fallible reads (AC 1, 2, 5)**
  - [ ] T2.1 `JournalState::try_data_version() -> Result<Option<i64>, String>` (`None` = no journal) and
        `try_list_drafts() -> Result<Vec<DraftRecord>, String>` in `app/src/state/drafts.rs`, the
        `try_list_studies` pattern (`super::read_failure(MSG_SUBJECT_DRAFTS, e)` — new subject const « la liste des
        propositions » beside `MSG_SUBJECT_STUDIES` (`messages.rs:100`), counted in
        `USER_FACING_MESSAGES` (+1, delta stated)).
  - [ ] T2.2 Keep `#[cfg_attr(not(test), expect(dead_code))]` on `mod drafts` only while something
        stays unused (`decide_draft` is 8.5b's) — if the expectation becomes unfulfilled, narrow it to
        the item level; never a module-wide `allow`.
- [ ] **T3 — View model `app/src/viewmodel/drafts.rs` (new, pure, no Slint) (AC 1, 2, 3)**
  - [ ] T3.1 `inbox_rows(drafts, studies_by_id, format, kind_filter, study_filter) -> InboxView`:
        pending only; groups by `study_id` (header « {TICKER} ({DEV}) » from the study), group order =
        newest draft first, draft studies last under « Nouvelles études »; rows newest first
        (`list_drafts` is ascending — reverse).
  - [ ] T3.2 Target and values: reuse the history view's field labels and value display
        (`viewmodel/history.rs`: `CELL_FIELDS`, `cell_value_display` — millions + locale, em-dash when
        absent —, `judgment_value_display`); make them `pub(crate)`, do not duplicate. Map
        `DraftField` → the entry wire key/label once (the wire→field direction exists as `judgment_draft_field`,
        `app/src/state/cells.rs:684`; add the field→wire/label direction here, once). Proposed value: parse the payload's proposed value with the registry
        (`DraftField::parse_value` — the AI text was validated at submission), then format with the
        same display path. Option values: the option's French label as the §4 chips show it.
  - [ ] T3.3 State word from `state::draft_freshness(study, kind, payload)`: Fresh → "", Stale →
        « périmée », TargetGone → « cible disparue ». Current value for a stale draft = today's value.
  - [ ] T3.4 `pending_by_study(drafts) -> HashMap<Uuid, usize>` and `pending_draft_studies(drafts)`
        for the rail, the reminder and the Études rows.
  - [ ] T3.5 Dates: JJ/MM/AAAA in local time (reuse `viewmodel/notes.rs::date_fr`, local time since the 8.1 review).
  - [ ] T3.6 Unit tests: grouping/order, every kind's target + values (incl. a millions field and a
        percent judgment), stale and target-gone words, draft-study row, filters, counts.
- [ ] **T4 — Components (AC 4, 1)**
  - [ ] T4.1 `app/ui/components/ai_frame.slint` (new) per spec §4.1: `surface-alt`, `separator` border,
        3 px left rule in `text-mid`; header « ★ IA — Proposée par {client} ({modèle}) le {JJ/MM/AAAA} »
        (`@tr` with placeholders), the AI text (`text-high`, word-wrap, plain), `@children`, the
        disclaimer « Texte rédigé par une IA, non vérifié — ne constitue pas un conseil financier. »
        (caption, `text-low`). Properties: `ai-client`, `ai-model`, `ai-text`, `submitted`, `compact`
        (header + one elided text line; disclaimer kept).
  - [ ] T4.2 `Tokens.ai-glyph: "★"` next to `gap-glyph` / `warn-glyph` (`tokens.slint`); every ★ reads it.
  - [ ] T4.3 `StatusBand` action slot (spec §4.5): `in property <string> action-label` (empty = no
        button) + `callback action()`, a trailing `ActionButton` (Tab after the text). Existing bands
        unchanged (empty label) — verify visually on one existing band.
- [ ] **T5 — Slint state + screen (AC 1, 2, 3, 5, 7)**
  - [ ] T5.1 `state.slint`: `export struct DraftRow { id, group, target, change, state-word,
        submitted, is-study, study-line, ai-client, ai-model, ai-text }` (only the `ai-*` fields carry
        AI-written text; `change` = « Actuel {x} → Proposé {y} » / « Nouvelle note », app text) and
        `export global Drafts { rows, pending-count, draft-study-count, rail-label, read-failed,
        failure-cause, read-only, kind-filter, study-filter options/value, open-study-pending,
        callbacks: pick-kind(string), pick-study(int), show-for-study(), show-draft-studies() }`.
        `StudyRow` gains `pending-drafts: int`. Re-export from `app.slint`.
  - [ ] T5.2 `app/ui/screens/propositions.slint` (new): one `PanelCard` « Propositions » (subtitle
        §3.3), kind chips « Toutes · Valeurs · Jugements · Notes · Études » (`ChoiceChip`), « Étude : »
        `Dropdown` (« Toutes les études » + one per group), bands (⊘ unreadable / ◦ read-only), group
        headers, rows (target, change, state word, date, compact `AiFrame`), empty text. Rows are
        display-only in 8.5a (the decision dialog is 8.5b); no « Registre » chip yet (8.7).
  - [ ] T5.3 `app.slint`: `destinations` gains `@tr("Propositions")` at index 4; the screen block
        gets `current-screen == 4: PropositionsScreen`, Réglages moves to 5; the rail's `NavItem`
        label is `index == 4 ? Drafts.rail-label : destination` (the title bar keeps
        `destinations[…]`). Check no other code sets `current-screen` to 4 or assumes Réglages = 4
        (grep: today only 0/1/2/3 are set from Rust; `REVIEW_SCREEN = 3`).
  - [ ] T5.4 Study pinned area (`study_screen.slint` ~l.445–472): after `VerdictBar` + separator, the
        ★ band (`Drafts.open-study-pending > 0`) or its ⊘ variant (`Drafts.read-failed`), action
        « Voir les propositions » → `Drafts.show-for-study()`; then the existing notice slot. Leave a
        comment marking where 8.8's `FrozenVerdictStrip` goes (between bar and ★ band).
  - [ ] T5.5 Études (`dashboard.slint`): the ★ band at the top of the « Études » card while
        `Drafts.draft-study-count > 0` (action → `show-draft-studies()`: Propositions, kind « Études »);
        list rows show « ★ {n} » after the name when `entry.pending-drafts > 0`.
- [ ] **T6 — Wiring `app/src/wiring/drafts.rs` (new) (AC 1, 5)**
  - [ ] T6.1 `push_drafts(ui, state, format)`: one read (`try_list_drafts` + the needed studies via
        `try_get_study`), builds the view model, sets the `Drafts` global, the rail label, the open
        study's count, `StudyRow.pending-drafts` (or re-run `refresh_studies`), the draft-study
        count. On ANY read failure: `read-failed = true`, cause set, rows cleared, counts 0, rail
        « · ⊘ » — never an empty-looking success.
  - [ ] T6.2 Poller: an `Rc<slint::Timer>` in `TimerMode::Repeated`, 2500 ms, started in `main.rs`
        after `show()` (the `restore_timer` pattern) and kept alive for the event loop. Pure decision
        in the view model: `PollState { last: Option<i64>, failed: bool }` +
        `fn on_tick(&mut self, read: Result<Option<i64>, String>) -> PollAction { Reread, Nothing,
        Failed(cause) }` — re-read when the version changed, after a failure, or on the first tick;
        unit-test it without a clock (feed results).
  - [ ] T6.3 Also push on: `screen-activated(4)` (new arm in `wiring/mod.rs`), study open, dossier
        open/switch, import, restore, and after `delete_study` (the O7 cascade deletes drafts through
        the app's own connection, which does not move `data_version`).
  - [ ] T6.4 Window focus: Slint 1.17 has no public window-activation callback (`i_slint_core`
        `Window::active` is internal; winit access needs an unstable feature) — do NOT add an unstable
        feature; the 2.5 s timer bounds the latency. Record this as a deviation from A9's "on window
        focus" in the story record.
  - [ ] T6.5 Navigation from bands: `show-for-study` sets `ui.set_current_screen(4)`, the study filter
        to the open study, and pushes; `show-draft-studies` sets kind « Études ». Setting
        `current-screen` from Rust does not fire `screen-activated` — call `push_drafts` explicitly.
- [ ] **T7 — Import message (8.2a deferral)**
  - [ ] T7.1 `MSG_JOURNAL_IMPORTED` names the drafts: « …, {txns} mouvement(s), {drafts}
        proposition(s). (source : …) » filled from `ImportSummary.ai_drafts`; update its test. Count
        unchanged (same const) — state it.
- [ ] **T8 — Glossary (posture AC)**
  - [ ] T8.1 `settings.slint` glossary: « Propositions » (where AI proposals wait; nothing applied
        without validation, one by one), « Proposition périmée » (the target changed since; can still
        be validated after confirmation), « Cible disparue » (only rejection possible). Neutral
        wording, no banned verb.
- [ ] **T9 — Tests (AC 4, 5, 6, 8)**
  - [ ] T9.1 AiFrame structural test (`app/src/posture.rs` or a new `app/src/ai_frame_scan.rs` test):
        read every `app/ui/**/*.slint`; fail on any `ai-` property/field access (`.ai-…`, `ai-…:`
        binding reads) that is neither inside `components/ai_frame.slint` nor lexically inside an
        `AiFrame { … }` element (brace-depth scan), nor the `DraftRow` field declarations in
        `state.slint`. Exemptions documented in the test with their §4.1 justification (proposed
        values = app-formatted numbers; ticker/currency = `identifier_invalid`-checked at
        submission; the prefilled edit field = AiFrame `@children`, 8.5b). Include a self-test
        (a synthetic Slint snippet with a stray `row.ai-text` must fail).
  - [ ] T9.2 Rust-side guard: the `ai_*` fields of `DraftRow` are set only in `wiring/drafts.rs`
        (grep test) — AI text never enters any other model.
  - [ ] T9.3 Concurrency (AC 6): in `persistence/tests/` — create a journal with
        `JournalMode::Delete`, open a raw `rusqlite` connection read-only, `BEGIN; SELECT count(*) FROM
        studies;` (holds SHARED), a thread releases it after ~1 s; meanwhile `Journal` upserts a study
        → Ok, elapsed < 5 s, the study reads back. Plus the control: a read held longer than
        `busy_timeout` yields a named busy error, not a hang.
  - [ ] T9.4 Poller unit tests (T6.2), view-model tests (T3.6), `data_version` test (T1.2).
  - [ ] T9.5 Wiring test (headless model level if the repo has the pattern): a failed read yields
        `read-failed` + rail « · ⊘ » + no rows; a success clears it.
- [ ] **T10 — Posture & verification (AC 8)**
  - [ ] T10.1 Re-base the `@tr` floor (count measured, tally comment) and `USER_FACING_MESSAGES`
        (only if a Rust const is added); banned-verb gate green.
  - [ ] T10.2 Confusability entry (UX-DR15): a headless 14 px capture of « ★ ◦ ◆ △ » side by side in
        the band/row context, filed in the story record (the gate is "by construction + visual" in
        this repo — `2-5` record; there is no automated perceptual test).
  - [ ] T10.3 Headless walk (verify skill) on a temp dossier seeded by `just mcp-seed` (see Dev Notes
        §Verification): screenshots of the rail with count, the Propositions card (every kind, a
        stale and a target-gone row, a draft study), the empty state, the ⊘ state, the study
        reminder band, the Études band + row marker, a draft arriving while the app is open
        (appears within ~3 s), keyboard Tab through chips/dropdown/band buttons.
  - [ ] T10.4 Gates: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D
        warnings`, `cargo fmt --check`, `cargo deny check` (CARGO_BUILD_JOBS=4).

## Dev Notes

### Scope guardrails
- **Read only.** No decision, no decision dialog, no row activation (8.5b). No « Registre » view,
  no « Valider… » on draft-study rows (8.7). No chart lines/chips (8.6). No frozen strip (8.8) — only
  leave its slot comment. No MCP code change.
- The app reads drafts through its own `Journal` (`list_drafts`, `get_study`), **never** through
  `McpAccess` (that is the MCP server's surface).
- No computation path reads drafts (FR72): the view models of the engine (`viewmodel/engine.rs`,
  `form.rs`, `chart.rs`, …) must not call `list_drafts` — `persistence/tests/drafts.rs` asserts it;
  keep the new code in `viewmodel/drafts.rs` + `wiring/drafts.rs`.
- Not in production: no compat work; additive only.

### Files — current state → change → must preserve
- `app/ui/app.slint` — 5 destinations, top-bar title reads `destinations[current-screen]` (l.87),
  screen blocks l.147–151, rail `for destination[index]` with the index-0 close-study rule. → add
  « Propositions » at 4, Réglages to 5, rail label override for 4. **Preserve** the index-0 close
  behaviour and `screen-activated(index)`.
- `app/src/wiring/mod.rs::wire_navigation` — arms 0–3, `_ => {}`. → arm 4 pushes drafts. Réglages (5)
  keeps falling through.
- `app/ui/components/status_band.slint` — `text` + `icon` only. → optional action slot; every current
  band renders identically.
- `app/ui/screens/study_screen.slint` l.443–472 — VerdictBar, separator, notice slot, then the
  Flickable. → insert the ★ band between; **preserve** the notice slot rule (F4, `wiring::study_notice`).
- `app/ui/screens/dashboard.slint` — « Études » `PanelCard` (l.245) with its notice band; `StudyRow`
  rows. → band + « ★ {n} »; `StudyRow` gains a field → every Rust construction site of `StudyRow`
  must set it (compile will list them).
- `app/src/state/drafts.rs` — 8.2b API (`draft_freshness`, `GoneReason`, `decide_draft` …) under
  `expect(dead_code)`. → add the two `try_*` reads; do not touch the decision rail.
- `persistence/src/journal.rs` — `logical_version()` l.704. → add `data_version()`.
- `app/src/state/messages.rs` l.686 `MSG_JOURNAL_IMPORTED`. → drafts count.
- `app/src/main.rs` l.341–363 — the `restore_timer` pattern (Rc<Timer>, Repeated, stopped after the
  loop). → the drafts poller, same lifetime handling.

### Polling (arch A9)
- `data_version` is per-connection and changes only on another connection's commit (MCP inserts,
  another process) — exactly the signal needed; one cheap pragma per tick.
- The app's own draft-affecting writes (delete_study cascade, import, restore, decisions in 8.5b) do
  not move it → push explicitly after them (T6.3).
- A failed tick (dossier gone, locked beyond busy_timeout, I/O) → ⊘ state with the cause, retried
  next tick; recovery clears it.
- No journal open → no rows, rail plain « Propositions », no ⊘ (a true absence, the
  `try_list_studies` rule).

### Values and labels
- Reuse, never re-derive: history's `CELL_FIELDS` labels + `cell_value_display` (grid millions +
  locale) and `judgment_value_display`. Sales / pre-tax proposals are stored absolute (8.2b, #117) —
  display them through the same millions path as the grid so « Actuel » and « Proposé » are in the
  same unit.
- Judgment labels: the history `LBL_*` constants + the runtime `Labels` set where the grid uses it.
- A judgment's « Actuel » is the study's current judgment value (em-dash when unset).

### Verification (headless)
- No real dossier exists on this machine anymore (Guy deleted them 2026-09-27). Build a temp one:
  copy `persistence/tests/corpus/v8.db` into the session scratchpad, open it once with the app
  (temp `XDG_CONFIG_HOME`/`XDG_DATA_HOME`, provider « none ») to migrate it to v9, quit (by PID), then
  `just mcp-seed <copy>` (the seed guard refuses real dossiers). Relaunch on the copy.
- To show a stale draft: after seeding, edit the target cell in the app (the fingerprint changes).
  Target gone: remove the year (if the UI allows) or edit the copy's payload with Python.
- Draft arriving while open: run `just mcp-seed` again (or a single stdio submit) with the app open;
  capture before/after within 3 s.
- ⊘ state: `chmod 000` the copy while the app runs (then restore) — or rename it away; capture.
- Never `pkill -f steadyinvest-app` (kills Guy's instance); kill by PID. Resize the window to the Xvfb
  screen right after launch (verify skill gotcha). `-u WAYLAND_DISPLAY`, `SLINT_BACKEND=winit-software`.

### Posture (checklist §6)
- Every new user string is an `@tr` literal from §3.3; the AiFrame header uses `@tr` placeholders; AI
  text never passes through `@tr` and is never posture-scanned (it is third-party content, FR13).
- New Rust consts (if any, e.g. the read-failure subject) join `USER_FACING_MESSAGES`, delta stated.

### Previous-story intelligence
- 8.1: a greyed button hides its reason — refusals go through the dialog; text areas/Flickables need
  care; « indisponible » must never look like empty; posture tallies are measured, not guessed;
  fast xdotool typing can leave the software renderer one frame behind under Xvfb.
- 8.2b: `draft_freshness` needs the study read today; `DraftRef` carries the ticker so refusals can
  name a gone study; `MSG_DECISION_*` exist for 8.5b.
- 8.4: a test once created a real default dossier — every app test must use a temp default
  (`open_or_create_with_default`, guard test in place). Keep it that way.
- 8.3: never open a second file descriptor on the dossier while a SQLite connection is open in the
  same process (POSIX lock loss) — the poller uses the existing connection only.

### Project Structure Notes
- New: `app/ui/components/ai_frame.slint`, `app/ui/screens/propositions.slint`,
  `app/src/viewmodel/drafts.rs`, `app/src/wiring/drafts.rs`, a persistence test file for AC 6.
- Naming follows the existing screens/wiring pairs (`review.slint` ↔ `wiring/review.rs`).

### References
- epics.md Story 8.5a + Epic 8 posture AC; ux-ai-assistance-surfaces.md §3.3, §4.1, §4.4, §4.5,
  §5.1–5.4, §6, §8, §10; architecture.md §Phase 4 A9, A12; docs/review-checklist.md §1, §6.

### Decisions taken (owner-pending, lead defaults)
1. Window-focus polling trigger omitted (no public Slint API); 2.5 s timer + inbox open instead.
2. The « Étude : » filter (spec: Registre) is also used in « À traiter » for « Voir les propositions »
   from a study — no new string.
3. Rows are display-only in 8.5a (activation = 8.5b).
4. A study read failure while building the inbox makes the whole read « indisponible » (no partial
   list with guessed values).

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

- Ultimate context engine analysis completed - comprehensive developer guide created (2026-09-28).

### File List
