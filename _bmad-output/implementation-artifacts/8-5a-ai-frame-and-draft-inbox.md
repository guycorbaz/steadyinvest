# Story 8.5a: AI frame and draft inbox (read)

Status: review

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

- [x] **T1 — Persistence: `data_version` (AC 1, 5)**
  - [x] T1.1 `Journal::data_version(&self) -> Result<i64>` = `PRAGMA data_version` on the app's own
        connection (`persistence/src/journal.rs`, next to `logical_version`). Doc: changes only when
        ANOTHER connection commits (A9); the app's own writes do not move it.
  - [x] T1.2 Unit test: a second connection's commit moves it; the owning connection's own commit
        does not.
- [x] **T2 — App state: fallible reads (AC 1, 2, 5)**
  - [x] T2.1 `JournalState::try_data_version() -> Result<Option<i64>, String>` (`None` = no journal) and
        `try_list_drafts() -> Result<Vec<DraftRecord>, String>` in `app/src/state/drafts.rs`, the
        `try_list_studies` pattern (`super::read_failure(MSG_SUBJECT_DRAFTS, e)` — new subject const « la liste des
        propositions » beside `MSG_SUBJECT_STUDIES` (`messages.rs:100`), counted in
        `USER_FACING_MESSAGES` (+1, delta stated)).
  - [x] T2.2 Keep `#[cfg_attr(not(test), expect(dead_code))]` on `mod drafts` only while something
        stays unused (`decide_draft` is 8.5b's) — if the expectation becomes unfulfilled, narrow it to
        the item level; never a module-wide `allow`.
- [x] **T3 — View model `app/src/viewmodel/drafts.rs` (new, pure, no Slint) (AC 1, 2, 3)**
  - [x] T3.1 `inbox_rows(drafts, studies_by_id, format, kind_filter, study_filter) -> InboxView`:
        pending only; groups by `study_id` (header « {TICKER} ({DEV}) » from the study), group order =
        newest draft first, draft studies last under « Nouvelles études »; rows newest first
        (`list_drafts` is ascending — reverse).
  - [x] T3.2 Target and values: reuse the history view's field labels and value display
        (`viewmodel/history.rs`: `CELL_FIELDS`, `cell_value_display` — millions + locale, em-dash when
        absent —, `judgment_value_display`); make them `pub(crate)`, do not duplicate. Map
        `DraftField` → the entry wire key/label once (the wire→field direction exists as `judgment_draft_field`,
        `app/src/state/cells.rs:684`; add the field→wire/label direction here, once). Proposed value: parse the payload's proposed value with the registry
        (`DraftField::parse_value` — the AI text was validated at submission), then format with the
        same display path. Option values: the option's French label as the §4 chips show it.
  - [x] T3.3 State word from `state::draft_freshness(study, kind, payload)`: Fresh → "", Stale →
        « périmée », TargetGone → « cible disparue ». Current value for a stale draft = today's value.
  - [x] T3.4 `pending_by_study(drafts) -> HashMap<Uuid, usize>` and `pending_draft_studies(drafts)`
        for the rail, the reminder and the Études rows.
  - [x] T3.5 Dates: JJ/MM/AAAA in local time (reuse `viewmodel/notes.rs::date_fr`, local time since the 8.1 review).
  - [x] T3.6 Unit tests: grouping/order, every kind's target + values (incl. a millions field and a
        percent judgment), stale and target-gone words, draft-study row, filters, counts.
- [x] **T4 — Components (AC 4, 1)**
  - [x] T4.1 `app/ui/components/ai_frame.slint` (new) per spec §4.1: `surface-alt`, `separator` border,
        3 px left rule in `text-mid`; header « ★ IA — Proposée par {client} ({modèle}) le {JJ/MM/AAAA} »
        (`@tr` with placeholders), the AI text (`text-high`, word-wrap, plain), `@children`, the
        disclaimer « Texte rédigé par une IA, non vérifié — ne constitue pas un conseil financier. »
        (caption, `text-low`). Properties: `ai-client`, `ai-model`, `ai-text`, `submitted`, `compact`
        (header + one elided text line; disclaimer kept).
  - [x] T4.2 `Tokens.ai-glyph: "★"` next to `gap-glyph` / `warn-glyph` (`tokens.slint`); every ★ reads it.
  - [x] T4.3 `StatusBand` action slot (spec §4.5): `in property <string> action-label` (empty = no
        button) + `callback action()`, a trailing `ActionButton` (Tab after the text). Existing bands
        unchanged (empty label) — verify visually on one existing band.
- [x] **T5 — Slint state + screen (AC 1, 2, 3, 5, 7)**
  - [x] T5.1 `state.slint`: `export struct DraftRow { id, group, target, change, state-word,
        submitted, is-study, study-line, ai-client, ai-model, ai-text }` (only the `ai-*` fields carry
        AI-written text; `change` = « Actuel {x} → Proposé {y} » / « Nouvelle note », app text) and
        `export global Drafts { rows, pending-count, draft-study-count, rail-label, read-failed,
        failure-cause, read-only, kind-filter, study-filter options/value, open-study-pending,
        callbacks: pick-kind(string), pick-study(int), show-for-study(), show-draft-studies() }`.
        `StudyRow` gains `pending-drafts: int`. Re-export from `app.slint`.
  - [x] T5.2 `app/ui/screens/propositions.slint` (new): one `PanelCard` « Propositions » (subtitle
        §3.3), kind chips « Toutes · Valeurs · Jugements · Notes · Études » (`ChoiceChip`), « Étude : »
        `Dropdown` (« Toutes les études » + one per group), bands (⊘ unreadable / ◦ read-only), group
        headers, rows (target, change, state word, date, compact `AiFrame`), empty text. Rows are
        display-only in 8.5a (the decision dialog is 8.5b); no « Registre » chip yet (8.7).
  - [x] T5.3 `app.slint`: `destinations` gains `@tr("Propositions")` at index 4; the screen block
        gets `current-screen == 4: PropositionsScreen`, Réglages moves to 5; the rail's `NavItem`
        label is `index == 4 ? Drafts.rail-label : destination` (the title bar keeps
        `destinations[…]`). Check no other code sets `current-screen` to 4 or assumes Réglages = 4
        (grep: today only 0/1/2/3 are set from Rust; `REVIEW_SCREEN = 3`).
  - [x] T5.4 Study pinned area (`study_screen.slint` ~l.445–472): after `VerdictBar` + separator, the
        ★ band (`Drafts.open-study-pending > 0`) or its ⊘ variant (`Drafts.read-failed`), action
        « Voir les propositions » → `Drafts.show-for-study()`; then the existing notice slot. Leave a
        comment marking where 8.8's `FrozenVerdictStrip` goes (between bar and ★ band).
  - [x] T5.5 Études (`dashboard.slint`): the ★ band at the top of the « Études » card while
        `Drafts.draft-study-count > 0` (action → `show-draft-studies()`: Propositions, kind « Études »);
        list rows show « ★ {n} » after the name when `entry.pending-drafts > 0`.
- [x] **T6 — Wiring `app/src/wiring/drafts.rs` (new) (AC 1, 5)**
  - [x] T6.1 `push_drafts(ui, state, format)`: one read (`try_list_drafts` + the needed studies via
        `try_get_study`), builds the view model, sets the `Drafts` global, the rail label, the open
        study's count, `StudyRow.pending-drafts` (or re-run `refresh_studies`), the draft-study
        count. On ANY read failure: `read-failed = true`, cause set, rows cleared, counts 0, rail
        « · ⊘ » — never an empty-looking success.
  - [x] T6.2 Poller: an `Rc<slint::Timer>` in `TimerMode::Repeated`, 2500 ms, started in `main.rs`
        after `show()` (the `restore_timer` pattern) and kept alive for the event loop. Pure decision
        in the view model: `PollState { last: Option<i64>, failed: bool }` +
        `fn on_tick(&mut self, read: Result<Option<i64>, String>) -> PollAction { Reread, Nothing,
        Failed(cause) }` — re-read when the version changed, after a failure, or on the first tick;
        unit-test it without a clock (feed results).
  - [x] T6.3 Also push on: `screen-activated(4)` (new arm in `wiring/mod.rs`), study open, dossier
        open/switch, import, restore, and after `delete_study` (the O7 cascade deletes drafts through
        the app's own connection, which does not move `data_version`).
  - [x] T6.4 Window focus: Slint 1.17 has no public window-activation callback (`i_slint_core`
        `Window::active` is internal; winit access needs an unstable feature) — do NOT add an unstable
        feature; the 2.5 s timer bounds the latency. Record this as a deviation from A9's "on window
        focus" in the story record.
  - [x] T6.5 Navigation from bands: `show-for-study` sets `ui.set_current_screen(4)`, the study filter
        to the open study, and pushes; `show-draft-studies` sets kind « Études ». Setting
        `current-screen` from Rust does not fire `screen-activated` — call `push_drafts` explicitly.
- [x] **T7 — Import message (8.2a deferral)**
  - [x] T7.1 `MSG_JOURNAL_IMPORTED` names the drafts: « …, {txns} mouvement(s), {drafts}
        proposition(s). (source : …) » filled from `ImportSummary.ai_drafts`; update its test. Count
        unchanged (same const) — state it.
- [x] **T8 — Glossary (posture AC)**
  - [x] T8.1 `settings.slint` glossary: « Propositions » (where AI proposals wait; nothing applied
        without validation, one by one), « Proposition périmée » (the target changed since; can still
        be validated after confirmation), « Cible disparue » (only rejection possible). Neutral
        wording, no banned verb.
- [x] **T9 — Tests (AC 4, 5, 6, 8)**
  - [x] T9.1 AiFrame structural test (`app/src/posture.rs` or a new `app/src/ai_frame_scan.rs` test):
        read every `app/ui/**/*.slint`; fail on any `ai-` property/field access (`.ai-…`, `ai-…:`
        binding reads) that is neither inside `components/ai_frame.slint` nor lexically inside an
        `AiFrame { … }` element (brace-depth scan), nor the `DraftRow` field declarations in
        `state.slint`. Exemptions documented in the test with their §4.1 justification (proposed
        values = app-formatted numbers; ticker/currency = `identifier_invalid`-checked at
        submission; the prefilled edit field = AiFrame `@children`, 8.5b). Include a self-test
        (a synthetic Slint snippet with a stray `row.ai-text` must fail).
  - [x] T9.2 Rust-side guard: the `ai_*` fields of `DraftRow` are set only in `wiring/drafts.rs`
        (grep test) — AI text never enters any other model.
  - [x] T9.3 Concurrency (AC 6): in `persistence/tests/` — create a journal with
        `JournalMode::Delete`, open a raw `rusqlite` connection read-only, `BEGIN; SELECT count(*) FROM
        studies;` (holds SHARED), a thread releases it after ~1 s; meanwhile `Journal` upserts a study
        → Ok, elapsed < 5 s, the study reads back. Plus the control: a read held longer than
        `busy_timeout` yields a named busy error, not a hang.
  - [x] T9.4 Poller unit tests (T6.2), view-model tests (T3.6), `data_version` test (T1.2).
  - [x] T9.5 Wiring test (headless model level if the repo has the pattern): a failed read yields
        `read-failed` + rail « · ⊘ » + no rows; a success clears it.
- [x] **T10 — Posture & verification (AC 8)**
  - [x] T10.1 Re-base the `@tr` floor (count measured, tally comment) and `USER_FACING_MESSAGES`
        (only if a Rust const is added); banned-verb gate green.
  - [x] T10.2 Confusability entry (UX-DR15): a headless 14 px capture of « ★ ◦ ◆ △ » side by side in
        the band/row context, filed in the story record (the gate is "by construction + visual" in
        this repo — `2-5` record; there is no automated perceptual test).
  - [x] T10.3 Headless walk (verify skill) on a temp dossier seeded by `just mcp-seed` (see Dev Notes
        §Verification): screenshots of the rail with count, the Propositions card (every kind, a
        stale and a target-gone row, a draft study), the empty state, the ⊘ state, the study
        reminder band, the Études band + row marker, a draft arriving while the app is open
        (appears within ~3 s), keyboard Tab through chips/dropdown/band buttons.
  - [x] T10.4 Gates: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D
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

Claude Opus 5.5 (claude-opus-5-5), dev-story, 2026-09-28.

### Debug Log References

- Slint: a `VerticalLayout` component cannot declare `row` (a grid layout property) → the row view's
  property is `draft`.
- Posture: `no_error_display_reaches_a_user_string` flagged the log-only `Unshowable.detail`
  (allow-listed, justified) and a first `error.to_string()` in the quiet poll read (removed: the
  poll logs through `read_failure` once, when a failure starts).
- Headless walk: the Études card is wider than the 1600 px window (the known G6 row-width debt), so
  a right-aligned band button fell off-screen → the StatusBand action sits under its text; the rail
  label « Propositions · ⊘ » was clipped at 200 px → the rail is 232 px.

### Completion Notes List

- Ultimate context engine analysis completed - comprehensive developer guide created (2026-09-28).
- **T1** `Journal::data_version()` (`PRAGMA data_version` on the journal's own connection).
  `persistence/tests/inbox_polling.rs`: it moves on another connection's commit only; a DELETE-mode
  read held 1 s delays the app's commit (≥ 0.5 s, < 5 s) and never loses it; a read held past
  `busy_timeout` is a named `Locked` error, nothing half-written, the next write lands (AC 6).
- **T2** `JournalState::read_inbox()` (drafts + today's study of every pending draft, in one read;
  any failure fails the whole read with `InboxReadError { message, cause }`) and
  `try_data_version(log)`; subject `MSG_SUBJECT_DRAFTS` « la liste des propositions »
  (`USER_FACING_MESSAGES` 241 → 242). The module-wide `expect(dead_code)` stays (the decision rail
  is 8.5b's); the `expect(unused_imports)` on `pub use drafts::*` became unfulfilled and was removed.
- **T3** `viewmodel/drafts.rs` (pure): grouping/order, targets and values through the history's
  labels and display (`CELL_FIELDS`, `cell_value_display`, `judgment_value_display` made
  `pub(crate)`), the field→wire map (`cell_wire`) once, the option labels as the §4 chips spell them,
  state words from `draft_freshness`, local-time dates (`date_fr`), counts, the poller's pure
  `PollState::on_tick`. 8 unit tests.
- **T4** `components/ai_frame.slint` (the only reader of `ai-*`), `Tokens.ai-glyph` « ★ »,
  `StatusBand` action slot (`action-label` + `action()`).
- **T5** `DraftRow` + `Drafts` global; `StudyRow.pending-drafts`; `screens/propositions.slint`;
  the sixth destination (Réglages → 5, the rail's own label); the study's ★ / ⊘ reminder in the
  pinned area (8.8 slot marked); the Études band + « ★ {n} ».
- **T6** `wiring/drafts.rs`: `push_drafts` (⊘ on any failure — rows cleared, counts zero, rail
  « · ⊘ »), the 2.5 s poller started in `main.rs` and stopped after the loop, `screen-activated(4)`,
  `refresh_studies` pushes the inbox (every draft-affecting app write ends there: delete cascade,
  imports, restore, dossier switch), `clear_dossier_session` forgets the poll version,
  `push_form` sets the open study's reminder, the bands' navigation.
- **T7** `MSG_JOURNAL_IMPORTED` names the proposals (`{drafts}` from `ImportSummary.ai_drafts`); test
  updated (count unchanged — same const).
- **T8** glossary: « Propositions », « Proposition périmée », « Cible disparue ».
- **T9** posture: `ai_written_text_is_read_only_inside_an_ai_frame` (brace-depth scan, exemptions
  documented) + its self-test (a stray `row.ai-text` fails; the frame and a `state.slint` field
  declaration pass); `draft_rows_are_built_only_in_wiring_drafts`; the inbox label inventory (5).
  State tests: the inbox read sees another connection's commit; an unreadable study makes the inbox
  unavailable (cause named); no dossier = empty, not a failure. **T9.5 — honestly: not done as
  written.** The repo has no UI-level (MainWindow) test harness, so no test asserts the `Drafts`
  global after a failed read; the rules are unit-tested at the state level (`read_inbox`) and the
  view-model level (`inbox_rows`, `PollState`), and the wiring (`show_failure` / `push_drafts`) was
  checked only on screen (headless walks below).
- **T10** posture: `@tr` floor 1035 → 1074 (+39, tally in `posture.rs`), messages 241 → 242,
  inbox labels 0 → 5. Gates: `cargo test --workspace` 1369 passed / 0 failed / 2 ignored (the corpus
  generators); clippy `-D warnings`, `fmt --check`, `cargo deny check` clean.
- **Headless walk** (verify skill, Xvfb :97, temp XDG dirs, provider « none », a copy of
  `corpus/v8.db` migrated to v9 by the app, then `just mcp-seed` WHILE the app was open; the real
  `~/.config/steadyinvest` and `~/.local/share/steadyinvest` checked untouched before and after —
  no default dossier created). Screenshots in `/tmp/claude-1000/-home-gcorbaz-devel-steadyinvest/8577d162-f5d7-4e5d-8ece-8aef6549b517/scratchpad/v85a/`:
  `01-studies.png` (Études band + action, « ★ 1 », rail « Propositions · 2 »),
  `02-props-studies.png` (band action → kind « Études », draft-study row, AiFrame, plain title),
  `04-after-seed.png` (7 drafts arrived within ~3 s of the seed, rail « · 7 »),
  `05-rows-bottom.png` (every kind: cell, note, option judgment, numeric judgment, two draft studies),
  `06-stale-gone.png` / `07-values-gone.png` (« périmée », « cible disparue » with « Actuel — »),
  `08/09-kbd-*.png` (Tab through the chips, Space activates, focus ring),
  `10-study-reminder.png` (★ reminder « 6 proposition(s)… » in the pinned area),
  `11-from-study.png` (reminder action → filter « NESN (CHF) »),
  `12-unreadable.png` / `13-start-broken.png` (⊘ band with its cause, rail « · ⊘ » — before and
  after the rail widening), `14-recovered.png` (recovery on the next poll),
  `15-study-reminder-unavailable.png` (the study's ⊘ reminder), `17-empty.png` (empty state),
  `18-read-only.png` (◦ read-only band on a write-protected copy),
  `19-confusability-14px.png` (★ ◦ ◆ △ at 14 and 19 px in the bundled Inter — all four covered,
  distinct by shape and fill; UX-DR15 entry).
- **Not verified headless:** the dropdown « Étude : » pick by mouse (its keyboard model is the
  existing Dropdown's); Guy's on-display check.

### G3 review (3 layers, no high) — applied 2026-09-28

Commits `ee1e814` (persistence), `17d0725` (posture), `6752489` (app + UI). 1378 tests passed /
0 failed / 2 ignored; clippy `-D warnings`, `fmt --check`, `cargo deny check` clean.

1. **AiFrame scan** — comment/string-aware lexer (`/* */`; `//` not cut inside a string; string
   contents ignored), `_`→`-` normalisation, a relay (`property` / `<=>`) inside an AiFrame is a
   violation; self-test extended (underscore, block comment, string, relay, two-way).
2. **Imported ticker** — the 8.3 identifier rule now holds for a DRAFT STUDY on import (malformed,
   nothing applied) and on every read (corrupt): shared `persistence::{is_ticker,
   is_currency_code}`. Other kinds copy their study's ticker (app data, maybe hand-typed) and are
   not held to it; the inbox shows such a ticker (study gone) only if it reads as one, else « — ».
3. **Stuck ⊘** — `PollState::mark_read_failed`, set by `show_failure`: the next tick re-reads even if
   `data_version` did not move; unit-tested tick sequence.
4. **Rust guard** — cuts only the trailing `#[cfg(test)] mod tests`; AI fields read only in
   viewmodel `ai_fields`, wiring `to_slint` (as `ai_*` assignments) and the 8.2b decision rail
   (writes them into the dossier — decision 13).
5. **Filters** — filter line « Aucune proposition ne correspond à ce filtre. »; « Études » clears the
   study pick; the rail (`screen-activated(4)`) and a dossier change reset kind + study; a failure
   clears the « Étude : » choices.
6. **Études ⊘** — « Les propositions d'étude n'ont pas pu être lues ; état indisponible. » +
   « Voir les propositions »; every row shows « ★ ⊘ » while unreadable (decision 15).
7. **Rail width** — back to 200 px; the count is a 16 px badge after the label (« · ⊘ », « · n »,
   capped « · 99+ » — 999+ does not fit, decision 17); NavItem spacing 12 → 8, rail right padding
   8 → 4. Study action row: at 1600 « Historique » is visible (`g02`); at 1280 it is clipped (`g03`)
   — the pre-existing G6 width debt: 8.5a no longer changes any width there (rail back to 200), so
   main clips it the same way.
8. **Band action** — compact 32 px `ActionButton` variant; TRAILING by default (the study ★ band is
   ~40 px, `g02`), under the text with `action-below` only on the over-wide Études card.
9. **Compact rows** — `one_line` collapses line breaks (LF, CR, U+2028, U+2029), tabs and runs of
   spaces; the proposed note / company name is a separate `ai-lead` elided FIRST, the comment keeps
   up to 360 px (`g05`); the AiFrame date is its own non-eliding Text (`g04`).
10. **Unshowable value** — a proposed value that is no value of its field → the whole inbox
    « indisponible (une proposition est illisible) » (`g15`), even when filtered out (every row is
    built before filtering).
11. **Archived studies** — listed with « étude archivée », left out of the rail / study counts
    (`g12`).
12. **Compact width** — kind chips wrap to two rows below 700 px of screen (`g04`, window 800);
    Dropdown min 200 / preferred 320 px.
13. **Performance** — `Journal::list_pending_drafts` (the inbox reads pending only); an unchanged
    re-read rebuilds no model.
14. **⊘ glyph** — Inter DOES cover « ⊘ » (fontTools; no fallback); it looks large because Inter draws
    it about an em wide — hence the smaller rail badge; kept everywhere for consistency.
15. **Evidence** (headless, Xvfb :97, temp XDG dirs, provider « none », fresh copy of `corpus/v8.db`
    migrated to v9 by the app, `just mcp-seed`; real `~/.config/steadyinvest` and
    `~/.local/share/steadyinvest` untouched before and after) — in `/tmp/claude-1000/-home-gcorbaz-devel-steadyinvest/8577d162-f5d7-4e5d-8ece-8aef6549b517/scratchpad/v85a-g3/`:
    `g01` Études at 1600 (rail « · 7 », band + action under the text, « ★ 5 ») · `g02` study at 1600
    (compact trailing ★ band, « Historique » visible) · `g03` study at 1280 (G6 clip, see 7) · `g04`
    Propositions at 800 px (wrapped chips, origin elided, date kept) · `g05` a multi-line note draft
    on one line, lead elided first · `g06`/`g07` a STALE row produced by a real in-app edit (the §4
    option chip) · `g08` rail activation resets the study filter · `g09`/`g10` Tab to the Dropdown,
    ↓ picks · `g11` the filter line · `g12` archived study · `g13`/`g14` chmod 000 on the dossier
    directory: the pragma does NOT fail (it reads the open connection's counter) — the pragma-failure
    ⊘ path is covered by unit tests only · `g15` read failure with its named cause · `g17` Études ⊘
    band + « ★ ⊘ » rows · `g18` rail « · ⊘ » fits 200 px · `g19` recovery · `g20` active rail with
    count · `g22` an EXISTING ◦ band (Revue) after the StatusBand relayout · `g23`/`g24` Tab to the
    study band's « Voir les propositions », Enter opens the filtered inbox · `g25` ★ / ⊘ / ◦ in band
    and row context. Not reachable on this data: ◆ in context (no watchlist item in the corpus — the
    14 px render `v85a/19-confusability-14px.png` stands); a lock blocking the inbox read (the app's
    WAL connection prevents an exclusive lock from another process).

### Decisions for Guy (owner-pending)

1. (story) No window-focus trigger — Slint 1.17 has no public window-activation API; the 2.5 s timer
   + a re-read when the inbox opens bound the latency (deviation from A9's « on window focus »).
2. (story) The « Étude : » filter (spec: Registre) also serves « À traiter » — « Voir les
   propositions » from a study lands filtered on it.
3. (story) Rows are display-only in 8.5a.
4. (story) A study read failure while building the inbox makes the whole read « indisponible ».
5. The ⊘ band without a nameable cause reads « … la liste est indisponible. » (no empty
   parentheses).
6. (revised by G3) The StatusBand action is a compact button, trailing by default; under the text only
   in the over-wide Études card (`action-below`). Existing bands (no action) render as before.
7. (revised by G3) The rail stays 200 px; the count is a 16 px badge after the label.
8. The submission date is shown once, in the AiFrame header (« … le JJ/MM/AAAA »), not repeated on
   the row.
9. A note draft's compact AiFrame line reads « {note} — {commentaire} » (the proposed text first).
10. The study reminder is not shown on the demo study.
11. The read side uses one state read (`read_inbox`) rather than `try_list_drafts` +
    `try_get_study` (same rules, one failure path).
12. (G3) New wording: « Aucune proposition ne correspond à ce filtre. », « étude archivée », « Les
    propositions d'étude n'ont pas pu être lues ; état indisponible. », the ⊘ cause « une
    proposition est illisible ».
13. (G3) The 8.2b decision rail (`state/drafts.rs`) is allowed to read the AI fields: it writes them
    into the dossier (the validated note, `AiOrigin`), never into a Slint model.
14. (G3) Drafts of an archived study stay listed (« étude archivée ») and are left out of the counts.
15. (G3) While the inbox is unreadable, every Études row shows « ★ ⊘ » (each study's state unknown).
16. (G3) The AiFrame header is two texts: « — Proposée par {client} ({modèle}) » (elides) and
    « le {date} » (never elides) — the §3.3 header split in two.
17. (G3) The rail count caps at « 99+ » (the suggested « 999+ » does not fit 200 px).
18. (G3) The identifier rule is enforced on read / import for draft STUDIES only; other kinds keep
    their study's spelling, and the inbox shows it (study gone) only when it reads as a ticker.

### File List

- `persistence/src/journal.rs` (modified — `data_version`)
- `persistence/src/drafts.rs`, `persistence/src/export.rs`, `persistence/src/mcp_access.rs`,
  `persistence/src/lib.rs`, `persistence/tests/drafts.rs` (modified — G3: identifier rule, pending
  read)
- `app/ui/components/nav_item.slint`, `app/ui/components/action_button.slint` (modified — G3)
- `persistence/tests/inbox_polling.rs` (new)
- `app/src/viewmodel/drafts.rs` (new)
- `app/src/viewmodel/mod.rs`, `app/src/viewmodel/history.rs`, `app/src/viewmodel/studies.rs` (modified)
- `app/src/state/drafts.rs`, `app/src/state/mod.rs`, `app/src/state/messages.rs`,
  `app/src/state/tests.rs` (modified)
- `app/src/wiring/drafts.rs` (new)
- `app/src/wiring/mod.rs`, `app/src/wiring/studies.rs`, `app/src/wiring/journal.rs`,
  `app/src/wiring/push.rs`, `app/src/main.rs`, `app/src/posture.rs` (modified)
- `app/ui/components/ai_frame.slint`, `app/ui/screens/propositions.slint` (new)
- `app/ui/app.slint`, `app/ui/state.slint`, `app/ui/tokens.slint`,
  `app/ui/components/status_band.slint`, `app/ui/screens/study_screen.slint`,
  `app/ui/screens/dashboard.slint`, `app/ui/screens/settings.slint` (modified)
- `_bmad-output/implementation-artifacts/8-5a-ai-frame-and-draft-inbox.md`,
  `_bmad-output/implementation-artifacts/sprint-status.yaml` (modified)

### Change Log

- 2026-09-28 — dev complete (inbox read side, AiFrame, poller, reminders, glossary, posture); status
  review.
- 2026-09-28 — G3 review applied (15 items: scan and guard hardened, identifier rule on read/import,
  never-stuck ⊘, filters, Études ⊘, rail 200 px with badge, compact band action, one-line rows,
  unshowable values, archived studies, narrow chips, pending-only read); headless re-walk `g01`–`g25`.
