# Story 8.8: Frozen decision-time verdict (FR68)

Status: done

<!-- Created 2026-09-30 by create-story on branch feat/8-8-frozen-verdict (main with 8.1–8.7).
     Autonomous Epic 8 run (Guy, 2026-09-28): questions resolved with the most conservative option,
     listed under « Decisions taken » for the final report. The Epic 8 retrospective follows, WITH
     Guy (2026-09-30). -->

## Story

As Guy,
I want to validate a study and have its verdict frozen at that moment, then see plainly when today's
verdict differs,
so that my decision stays on record and a later refresh never silently rewrites what I decided on.

## Acceptance Criteria

Source: `epics.md` Story 8.8 (l.1342–1364) + the Epic 8 posture AC; UX spec
`ux-ai-assistance-surfaces.md` (validated 2026-09-27, Q1–Q16 defaults) §3.1, §3.2 (l.78), §3.3
(l.150–164), §4.5, §4.6, §5.3, §5.9 (l.367–384), §6, §7 (l.396–407), §8 (l.437–443), Q8, Q12; arch
D11, A13 (l.1152–1170), A3, A8, A12; PRD FR68 (l.902–913), FR12, FR13, FR29, FR32, FR51, FR52,
FR59, FR60, FR69, NFR-U1.

### A. Freezing

1. **« Valider l'étude » (spec §5.9, Q12).** In the study action row after « Historique »
   (`study_screen.slint` l.575–586), an `ActionButton` « Valider l'étude », Tab-reachable, disabled
   on the demo study and on a read-only dossier like its neighbours. **Disabled while the verdict is
   not `Full`**, with the reason beside it (caption, `text-mid`): « Verdict incomplet — entrées
   ouvertes : {liste} » — {liste} the open gates' labels (`engine::open_gate_label`, « BPA 2023 — non
   validé »…) joined by « , »; a low-confidence study with no open gate names its low-confidence
   label (Decision 5).
2. **The freeze (A13, FR68, D11).** On a `Full` verdict, « Valider l'étude » stores
   `frozen_verdict: Option<FrozenVerdict>` (`#[serde(default, skip_serializing_if =
   "Option::is_none")]`) in the `Study` blob — through the ordinary owner-edit rail
   (`mutate_study` → `put_study_with_history`: one upsert + its FR51 snapshot) — built ONLY from a
   `core::verdict::FullVerdict` of the study's coherent snapshot (the compile-time "Full only"
   gate) and that snapshot's outputs:
   - `frozen_at` (the app clock), `method_version` (String), `inputs_hash`;
   - the facts: `quality_value_candidate`, `present_zone` (neutral `low` / `middle` / `high`,
     Decision 2), the four criteria (`met` / `unmet` / `unmet_by_insufficiency`);
   - the figures: U/D ratio (or `undefined` / `unknown`), relative value %, projected
     appreciation %, the « Potentiel à 5 ans » figure the study header shows, the zone bounds
     (forecast low, buy top, neutral top, forecast high), forecast high / low;
   - the inputs it was computed from: EVERY input the `inputs_hash` covers (all year cells of the
     usable years, every judgment input, the quarterly observations) as `key → value` (Decision 3).
   Outcome notice (F4 slot): « Étude validée ; verdict figé le {JJ/MM}. »
3. **Replace (spec §3.3).** With a frozen verdict already stored, « Valider l'étude » first asks the
   modal confirm: title « Remplacer le verdict figé ? », body « Le verdict figé le {JJ/MM} sera
   remplacé ; il reste lisible dans l'historique. », verb « Valider l'étude ». Confirmed → the new
   freeze replaces it (the previous stays in the FR51 history); cancelled → nothing.
4. **Undo / redo (FR32, A8).** The freeze is on the study's undo stack like any owner edit: Ctrl+Z
   restores the previous `frozen_verdict` (none, or the replaced one) with the notice « Validation
   de l'étude annulée. »; Ctrl+Y re-applies it with « Étude validée ; verdict figé le {JJ/MM}. ».
5. **History (FR51, spec §3.3 l.150).** A snapshot whose only change is a new / replaced frozen
   verdict reads « Étude validée ; verdict figé »; one that removes it (an undo) reads « Validation de
   l'étude annulée » (Decision 7); a frozen change beside other changes adds the clause. Never
   « autres champs modifiés » for a freeze; never a note-only entry.
6. **Refusals.** Read-only dossier → the existing read-only refusal; a write failure → the existing
   named save failure; a verdict that stopped being `Full` between render and click → « Action
   refusée » « L'étude ne peut pas être validée : le verdict n'est pas complet (entrées ouvertes :
   {liste}) ; rien n'a été enregistré. » (spec §3.3, the Q12 alternative's words — Decision 6).

### B. Frozen vs current

7. **The strip (spec §4.6, §5.3).** `FrozenVerdictStrip` in the reserved slot under the verdict bar
   (`study_screen.slint` l.460), above the ★ band and the notice. Not rendered without a frozen
   verdict. **Same** (facts, figures, `inputs_hash` and `method_version` all equal): the one-line
   caption « Étude validée le {JJ/MM} ; le verdict actuel est identique au verdict figé. »
   **Differs**: a `StatusBand` glyph « • », « Le verdict actuel diffère du verdict figé le {JJ/MM}. »,
   action « Voir la comparaison » / « Masquer la comparaison », starting collapsed on every open.
8. **The comparison (spec §5.9).** Columns « figé ({méthode}, {JJ/MM}) » · « actuel ({méthode},
   aujourd'hui) » (« actuel ({méthode}, provisoire) » / « actuel ({méthode}, retenu) » when the
   current verdict is not full); rows « Verdict » (quality-value candidate) · « Zone du prix » (the
   runtime `Labels.zone-*` words, in ink — never a zone hue) · « Ratio hausse/baisse » (value + the
   ≥ 3 criterion) · « Valeur relative » (< 100 %) · « Appréciation projetée » (≥ doubling) ·
   « Potentiel à 5 ans » · « Entrées » (« identiques » or « {n} modifiée(s) : {champ} {figé} →
   {actuel}, … ») · « Méthode ». A withheld current cell reads « retenu — entrées ouvertes : {liste} »;
   provisional figures in `text-mid`. **Changed rows**: a leading « • » and semibold figures (two
   non-colour channels, NFR-U1); unchanged rows `text-mid`. Then the cause line « Cause : {causes} ».
   Capped at 40 % of the study area's height, scrolling internally.
9. **The cause (FR29, A13).** Derived from the stored study at display time (Decision 4): any
   `method_version` difference → « changement de méthode ({vA} → {vB}) »; year cells whose
   provenance is newer than `frozen_at` → provider → « rafraîchissement du {JJ/MM} » (the latest),
   manual with an AI origin → « proposition de l'IA validée », manual → « modification de votre
   part »; changed judgment fields → AI-placed → « proposition de l'IA validée », else
   « modification de votre part » — except `current_price` / `ttm_eps` (refreshed by the provider and
   the holdings price too): « rafraîchissement du {JJ/MM} » when a provider refresh is seen after
   `frozen_at`, else « cause inconnue »; nothing attributable → « cause inconnue ». Several causes
   are joined by « , », each once.
10. **Always live (A13).** The current verdict is computed live, never persisted; the frozen one is
    never recomputed. A test: freeze → refresh with a changed EPS series → the stored
    `frozen_verdict` is byte-identical, the current differs, the comparison shows it; the same with
    a method change (the stored `method_version` set to an older version — the constant cannot move
    in a test, Decision 8).

### C. Elsewhere

11. **PDF (FR52, spec §7).** After the verdict in « Synthèse »: « Verdict figé le {JJ/MM/AAAA}
    ({méthode}) » with its figures and, when it differs, the comparison table in greyscale, changed
    rows marked « • » (0x95), « -> » for « → », « >= » for « ≥ », and the cause line. A study without
    a frozen verdict prints byte-identical (the pinned hash stays green).
12. **Export / import (FR59, FR60).** A single-study and a whole-journal round-trip preserve the
    frozen verdict byte-identically; a legacy blob without the key reads `None` and re-serializes
    byte-identical (the pinned canonical study JSON unchanged — `skip_serializing_if`).
13. **MCP (FR68 [P4], FR69, A3).** `get_study` returns the frozen verdict (the blob's
    `frozen_verdict`, neutral zone codes) beside `computed`; the tool description says so. No verdict
    is frozen or changed through MCP: `frozen_verdict` is no `DraftField` (a submission naming it is
    refused `field_not_draftable` — case added), and the 8.3 rejected-writes suite gains an attempt to
    write it (`UPDATE studies SET payload = json_set(payload, '$.frozen_verdict', …)` → denied,
    dossier unchanged); the 8.4 non-exposure suite stays green with a frozen study in its fixture
    (no « buy / hold / sell » word leaks).
14. **Posture (Epic 8 posture AC).** Every string from §3.3 verbatim; `@tr` floor, `MSG_*`,
    `HISTORY_USER_FACING_LABELS`, engine labels, `REPORT_USER_FACING` re-based with measured deltas;
    banned-verb gate (core's verdict vocabulary list too if a core string is added — none planned);
    glossary « Verdict figé » (« Le verdict enregistré quand vous validez l'étude ; il ne change
    plus. Le verdict actuel, recalculé à chaque ouverture, est comparé au verdict figé et toute
    différence est signalée. »); headless visual verification (DoD).

## Tasks / Subtasks

- [x] **T1 — Contract (AC 2, 12).** `contract/src/frozen.rs`: `FrozenVerdict`, `FrozenZone`
  (serde `low` / `middle` / `high`), `FrozenCriterion`, `FrozenUpsideDownside`, `FrozenZoneBounds`,
  `inputs: BTreeMap<String, String>`; `Study.frozen_verdict` with
  `#[serde(default, skip_serializing_if = "Option::is_none")]`; every `Study { … }` literal gains
  `frozen_verdict: None` (list in Dev Notes); tests like `contract/tests/ai_marks.rs` (absent key,
  legacy byte-identical, round-trip); `roundtrip.rs` strategy.
- [x] **T2 — Report builder (AC 2, 10).** `report::form`: `freeze(study, now) -> Result<FrozenVerdict,
  NotFull>` from ONE `build_frame` (`verdict()` must be `Verdict::Full`); `frozen_inputs(study)` —
  the hashed input set as keys (`y{year}.{field}`, `j.{field}`, `q.{field}`) → canonical decimal
  strings / « absent »; `current_facts(snapshot)` in the same shape for the comparison. Tests: Full
  only; byte-stable; inputs keys cover the digest's.
- [x] **T3 — State (AC 2–6).** `JournalState::freeze_verdict(study_id, replace_confirmed)` via
  `mutate_study`; `FreezeOutcome`; the not-full refusal; undo discriminator (`Stepped` sees a
  `frozen_verdict` change → the two notices). `MSG_*` constants.
- [x] **T4 — Comparison view model (AC 7–9).** `app/src/viewmodel/frozen.rs`: `strip(study,
  snapshot, format) -> FrozenStripView { None | Same{date} | Differs{date, columns, rows, cause} }`;
  row values app-formatted like the verdict bar; « Entrées » diff with field labels; cause derivation
  (pure, tested with provenance fixtures).
- [x] **T5 — Slint (AC 1, 7, 8).** `state.slint`: `FrozenStrip` struct + `Studies.frozen-strip`,
  `freeze-reason`, `can-freeze`, callback `freeze-verdict()`; `components/frozen_verdict_strip.slint`;
  the button + reason in the action row; confirm via `Dialog.confirm` (the note-delete precedent).
  Wiring in `wiring/push.rs` (push_form + live drag path) and `wiring/judgment.rs` (callback, notices).
- [x] **T6 — History (AC 5).** `viewmodel/history.rs` facet `frozen: Option<FrozenChange>`; labels
  « Étude validée ; verdict figé », « Validation de l'étude annulée »; excluded from `notes_only`.
- [x] **T7 — PDF (AC 11).** `report/src/pdf.rs`: the block after « Position : … », strings in
  `REPORT_USER_FACING`, WinAnsi-safe; tests: frozen same / differs; no-frozen pin unchanged.
- [x] **T8 — MCP + export (AC 12, 13).** Tool description; refusal case `frozen_verdict`; the 8.3
  engine-denial statement; non-exposure fixture with a frozen study; persistence export round-trip
  test (single study + journal, byte-identical).
- [x] **T9 — Tests of AC 10** (freeze → changed EPS refresh → frozen byte-identical, differs, cause
  « rafraîchissement du … »; method change → « changement de méthode (… → …) »); undo / redo; replace.
- [x] **T10 — Posture + glossary (AC 14).**
- [x] **T11 — Headless walk (DoD).** Temp HOME / XDG, provider « none », a copy of the corpus + the
  g01 DEMO studies (DEMO1 is Full once every input is validated — use « Toute l'étude » validation):
  disabled button + reason on a provisional study; freeze; « identique » caption; an owner edit of a
  load-bearing input → the • band, comparison expanded (every row marked / not, « Entrées », cause
  « modification de votre part »); replace confirm; undo / redo notices; history entries; 1280 and
  1600. Screens in the session scratchpad `v88/`. No provider fetch (the refresh cause is tested).
- [x] **T12 — Record.** Story record, posture deltas, decisions, sprint-status 8-8 → review.

## Dev Notes

### What exists (read before changing)

- **core::verdict** (`core/src/verdict/mod.rs`): `FullVerdict { facts, inputs_hash, method_version:
  &'static str }` private fields, accessors, no serde, only from `StudySnapshot::verdict()`;
  `Verdict { Full, Provisional(Degraded), Withheld(Degraded) }`; `DegradedVerdict.open_gates`,
  `low_confidence`. `VerdictFacts` (`core/src/ssg/types.rs:262–278`) = zone + 4 `CriterionFact` +
  `quality_value_candidate` — the figures (U/D `UpsideDownside`, `ZoneBounds`, relative value,
  appreciation, potential) are in `snapshot.outputs()`. `inputs_digest` (`verdict/digest.rs:38–163`):
  all 7 year fields of every canonical year + usable_years, 10 judgment inputs, 5 quarterly
  observations; gates not hashed. `METHOD_VERSION = "ssg-1.2.0"` (`core/src/method_version.rs`).
  **Do not add serde to core** (8.4 rule); `contract` does not depend on `core` → the builder lives in
  `report::form` (shared by app, PDF, MCP).
- **#252 method stamp** is per provider cell (`"{digest}@ssg-X.Y.Z"` in `hash_of_dependencies`,
  `app/src/state/refresh.rs:36–55, 359–385`) — the INPUTS' definition, not the verdict's; the verdict
  method change is `frozen.method_version != METHOD_VERSION` (new).
- **contract::Study** (`contract/src/study.rs:156–207`); `AiPlaced` / `ai_origin` use
  `skip_serializing_if` (the pattern that keeps pins byte-identical); pin
  `persistence/tests/corpus_gate.rs:148`; `contract/tests/ai_marks.rs:83–112` template.
  `Study { … }` literals: `contract/src/study.rs:194`, `contract/tests/roundtrip.rs:178`,
  `app/src/viewmodel/drafts.rs:~1200`, persistence tests `draft_study_validation.rs`, `watchlist.rs`,
  `drafts.rs`, `readonly_newer.rs`, `readonly_protected.rs`, `corpus_gate.rs`, `inbox_polling.rs`,
  `draft_decisions.rs`, `e2e_lifecycle.rs`, `journal_roundtrip.rs` (+ any others the compiler finds).
- **App verdict**: `engine::verdict_badge` (l.678–723; `fmt_ud`, `fmt_total_return`, `zone_key`),
  `open_gate_label` (l.907), `verdict_state()`; `VerdictBar` (`components/verdict_badge.slint:179`),
  `Labels.zone-*`; `push_form` sets the verdict (`wiring/push.rs:122`), the live drag path (l.305).
- **Owner-edit rail**: `mutate_study` (`app/src/state/cells.rs:554–587`); `add_note` rail
  (`state/notes.rs:31–92`); undo `state/undo.rs` (`Stepped`, `step()` l.254–340); undo outcomes
  `wiring/judgment.rs:597–635`; confirm `wiring/dialog.rs::confirm_for`; callback pattern
  `wiring/judgment.rs:121–138`; notices `study_notice::outcome(ui, Source::Edit, …)`.
- **History** `viewmodel/history.rs` (`diff_states` l.247–296, `summary_of`, `detail_of`,
  `notes_only`; freeze today reads « autres champs modifiés »).
- **PDF** `report/src/pdf.rs`: « Position : … » l.602–610; `REPORT_USER_FACING` l.1301; WinAnsi
  (`winansi_byte` l.3337: † 0x86, • 0x95); pin `a_study_without_ai_origin_renders_byte_identical_to_main`
  (l.3997); marks emitted only when present (the 8.5b/8.6 way to keep the pin).
- **MCP**: `dto::study_read` (`mcp/src/dto.rs:136–151`: raw `study` + `computed`), neutral
  `zone_code` (l.35–41, tests forbid buy/sell/hold); `tools.rs:186–192` description; 8.3 suites
  `persistence/src/mcp_access.rs:1726–1796, 1871–1996`, `persistence/tests/mcp_access.rs:568–696`;
  8.4 `mcp/tests/non_exposure.rs`, fixtures `mcp/tests/support/mod.rs:52`; `DraftField::ALL`
  (`contract/src/draftable.rs:148`).
- **Export**: the payload is the serde Study (carried automatically); model test
  `persistence/tests/export.rs:816–870`.
- **UI slot**: `study_screen.slint:460–461` comment; `StatusBand` action slot
  (`components/status_band.slint:10–23`). Name collision: `validate_draft_study` / `validate-study`
  are 8.7's — use `freeze_verdict` / `freeze-verdict`.
- **Posture baseline (main after 8.7)**: `@tr` ≥ 1147; `USER_FACING_MESSAGES` 248; history labels 26;
  engine labels 25; drafts 6; decision 4; AI lines 1.

### Guardrails

- The frozen verdict is built only from a `FullVerdict` + its own snapshot's outputs (one
  `build_frame`); never re-derived, never recomputed; the current verdict is never persisted.
- No colour for the frozen verdict or its differences (glyph, placement, weight, ink); the only hued
  verdict stays the current one.
- No MCP write path; the frozen verdict is readable, never draftable.
- Additive field only (A5, D9 withdrawn): no `SCHEMA_VERSION` bump; `skip_serializing_if` keeps every
  pin and legacy blob byte-identical.
- Safety: headless on temp copies, provider « none », no provider fetch driven headless, never touch
  the real config / data dirs, never register the MCP server. `CARGO_BUILD_JOBS=4`; never `git add
  -A`. Read gates from the `test result` summary, never through a pipeline that hides the exit code.

### Decisions taken (owner-pending, for the final report)

1. **FR68's first half is superseded by D11 / A13** (the current verdict is live and coloured; the
   comparison shows on any difference, not only on a method change) — the stories and the UX follow
   D11; recorded, not reconciled in the PRD.
2. **Zones are stored as neutral codes** (`low` / `middle` / `high`, the MCP convention) — the raw
   blob is returned by MCP (FR13: no buy / sell word through MCP); the app writes them with the
   runtime `Labels.zone-*`.
3. **« The load-bearing input values » = every input the `inputs_hash` covers** — so an inputs-only
   difference always names its fields in « Entrées » (a narrower set could show « identiques » under a
   different hash).
4. **The cause is derived at display time** from the stored study (cell provenance, AI marks, the
   stored `method_version`); `current_price` / `ttm_eps` changes are « rafraîchissement du … » only
   when a provider refresh is seen after the freeze, else « cause inconnue » (the holdings price also
   writes them). No cause column is added to the history (A12).
5. **A low-confidence Full-blocked study** names its low-confidence label in the reason ({liste}),
   having no open gate to name.
6. **The not-full refusal** (a verdict that stopped being Full between render and click) uses the
   §3.3 Q12-alternative words; the button itself stays disabled with its reason (Q12 default).
7. **The history label of an undone freeze** is « Validation de l'étude annulée » (the outcome's
   words, without the period); the spec names only the freeze's.
8. **The method-change test** rewrites the stored `method_version` to an older one (the constant
   cannot move in a test) — the comparison and the cause are what is checked.
9. **« {méthode} » is the stored method string verbatim** (« ssg-1.2.0 »), the one the verdict trace
   already shows; dates on screen JJ/MM (spec), in the PDF JJ/MM/AAAA (spec §7).
10. **« Potentiel à 5 ans »** is the figure the study header shows under that name (the same
    formatter), frozen as its decimal.
11. **Replace when nothing differs**: « Valider l'étude » still asks the replace confirm (the spec
    confirms whenever a frozen verdict exists); the new freeze carries the new date.
12. **The « Verdict » row reads « critères réunis » / « critères non réunis »** (the app had no word
    for the quality-value candidate; neutral, fact-stating) and each criterion « réuni » / « non
    réuni » / « inconnu » after its figure (« 0,8:1 · ≥ 3 : non réuni »).
13. **« Entrées » on screen names each changed input** (« PER haut moyen 18 → 20 »); **the PDF gives
    their count** (« 2 modifiée(s) ») — the screen is the place to read them.
14. **The PDF block is lines « libellé : figé -> actuel »** with « • » on the changed ones (not a
    ruled table), after « Position : … » in « Synthèse ».
15. **The cause / difference logic lives in `report::frozen`** (one logic for the screen and the
    PDF); the app formats the figures with its own number format.
16. **« Valider l'étude » sits on its own line under the action row** (the spec says « after
    Historique »; the row is already wider than the window at 1280 / 1600 — the button would be
    off-screen); its reason elides on the same line.
17. **Undoing a REPLACE** restores the previous frozen verdict, so that history entry reads
    « Étude validée ; verdict figé » (a verdict is set), not « annulée ».

### Testing standards

Contract tests in `contract/tests`; builder tests in `report`; state tests in
`app/src/state/tests.rs`; view model tests in `viewmodel/frozen.rs`; PDF tests in `report/src/pdf.rs`;
MCP / persistence suites as listed; gates `cargo test --workspace`, clippy `-D warnings`, fmt, deny.

### Project Structure Notes

New: `contract/src/frozen.rs`, `app/src/viewmodel/frozen.rs`,
`app/ui/components/frozen_verdict_strip.slint`. Touched: `contract/src/{lib,study}.rs`,
`report/src/{form,pdf}.rs`, `app/src/state/{cells or studies,undo,messages,tests}.rs`,
`app/src/viewmodel/{mod,history,engine}.rs`, `app/src/wiring/{push,judgment}.rs`,
`app/ui/{state.slint, screens/study_screen.slint, screens/settings.slint}`, `app/src/posture.rs`,
`mcp/src/tools.rs`, MCP / persistence / contract tests.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 8.8]
- [Source: _bmad-output/planning-artifacts/ux-ai-assistance-surfaces.md#3.3 (l.150–164), #4.6, #5.3, #5.9, #7, #9 Q8/Q12]
- [Source: _bmad-output/planning-artifacts/architecture.md#Phase 4 D11, A13, A3, A8, A12]
- [Source: _bmad-output/planning-artifacts/prd.md#FR68]
- [Source: core/src/verdict/mod.rs, core/src/verdict/digest.rs, core/src/ssg/types.rs]

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5)

### Debug Log References

- A divergent refresh of a VALIDATED cell never changes it (#110: parked as pending) — the AC 10
  test therefore accepts the provider values after the refresh (the owner's gesture that lets the
  new EPS in); the cause reads « rafraîchissement du … » (the accepted cells are provider-sourced).
- The fixed test clock stamps the freeze and the refresh in the same second; the cause needs a
  provider write strictly after the freeze → the tests back-date `frozen_at`.
- `row` and `col` are reserved names inside Slint layouts (renamed `entry` / `half`).
- A word-wrapped `Text` inside the action `HorizontalLayout` inflated the page height (the whole form
  below vanished) — the reason now elides on its own line.

### Completion Notes List

- T1 (contract): `FrozenVerdict` — additive, `skip_serializing_if`, NEUTRAL
  names (`low` / `middle` / `high`, `low_zone_top`, `present_price_in_low_zone`, `unknown` — the MCP
  `computed` spelling); pins and legacy blobs byte-identical.
- T2 (`report::form::freeze` / `verdict_record` / `frozen_inputs`): only a `Verdict::Full`; the
  inputs are every value the digest covers (5 years × 7 fields + 10 judgment + 5 observations in the
  test).
- T3 (`JournalState::freeze_verdict`, `Stepped::Freeze`, `MSG_FREEZE_*`), T4 (`viewmodel::frozen`
  strip + `report::frozen` shared `same` / `changed_inputs` / `causes`), T5 (the strip component, the
  button, the confirm, the wiring), T6 (history facet), T7 (PDF block), T8 (MCP + export), T9
  (tests), T10 (posture + glossary).
- **Posture deltas (measured)**: `@tr` floor 1147 → 1172 (+25, itemised in `posture.rs`);
  `USER_FACING_MESSAGES` 248 → 251; `HISTORY_USER_FACING_LABELS` 26 → 28;
  `FROZEN_USER_FACING_LABELS` 18 (new, scanned); `REPORT_USER_FACING` + the 27 PDF strings (neutral
  test green). Gates: fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast` 1445
  passed, deny ok.
- **T11 headless walk** (Xvfb :94, temp HOME / XDG, provider « none », a fresh copy of
  `persistence/tests/corpus/v8.db` + DEMO1 / DEMO2 from golden g01, `just mcp-seed`; real
  `~/.config/steadyinvest` + `~/.local/share/steadyinvest` listed + md5 before/after — unchanged).
  Screens in the session scratchpad `v88/`: 01 (the button off-screen after « Historique » and the
  form gone → fixed), 02 NESN provisional: « Valider l'étude » disabled, « Verdict incomplet —
  entrées ouvertes : Prix haut 2021 — non validé, … »; 05 DEMO1 frozen: « Étude validée ; verdict
  figé le 30/09. » + the caption « … identique au verdict figé »; 07 an owner edit (PER haut 18 →
  20) → the « • » band; 08 / 10 the comparison (• + semibold on U/D, appreciation, potential,
  entries; « Entrées : 1 modifiée(s) : PER haut moyen 18 → 20 »; zones « Zone de maintien » in ink;
  « Cause : modification de votre part »; columns aligned after the fix); 09b collapsed on reopen;
  11 the replace confirm; 12 replaced (identique again); 14 undo → « Validation de l'étude
  annulée. » and the previous frozen verdict back; 17 history: « Étude validée ; verdict figé » ×2;
  18 at 1280.
- **Found and fixed in the walk**: the button off-screen at the end of an overflowing action row
  and the page inflated by a word-wrapped reason (→ its own line, elided); the comparison's columns
  not under their headers and the label column too narrow.
- **Seen, not changed (pre-existing)**: the study screen wider than the window at 1280 and with the
  history open (the G6 width debt); after a modal closes no study element holds the keyboard focus,
  so Ctrl+Z needs a click on a focusable control first (as after any dialog since 2.9).
- No UI-level harness: the wiring is verified headless only.

### G3 review (3 layers: Blind Hunter, Edge Case Hunter, Acceptance Auditor)

Applied (86ccf96), 1449 tests, clippy / fmt / deny green; re-walk `v88g3/01–03` (DEMO1 made
provisional after its freeze: band « diffère », « Valider l'étude » disabled with its reason, the
Verdict row marked, « Cause : modification de votre part, entrées ouvertes : BPA 2025 — non
validé »; real `~/.config` / `~/.local/share` unchanged):
1. **The screen and the PDF disagreed on « identique »** (all three layers, high): one rule,
   `report::frozen::differs(frozen, current, current_full)` — a verdict no longer Full differs
   even with the same figures; its Verdict row is marked and the cause names the open inputs
   (`CAUSE_OPEN` « entrées ouvertes : {list} »). The PDF prints the current state (« provisoire »
   / « retenu ») and masks a withheld verdict's figures as the screen does.
2. **The withheld mask missed the zone row** (blind + auditor): every current cell after the
   Verdict row reads « retenu — … ».
3. **Undoing a replacement said « Validation de l'étude annulée »** (blind + edge): the notice
   follows the restored verdict — the previous one's « verdict figé le {date} », or « annulée »
   when none is left; it now agrees with the history.
4. **Dates local on screen, UTC in the PDF** (all three): the UTC date of the stamp on both
   surfaces (`report::frozen::day_month`, `jj_mm_aaaa`).
5. **« • » over identical texts** (blind + edge): the mark follows the DISPLAYED text on both
   surfaces (a change below the display precision, `Undefined` ↔ `Unknown`, or the unshown
   in-low-zone criterion alone no longer mark a row).
6. **Causes** (edge + blind): a refresh stamped in the freeze's own second counts (`>=`); a year
   removed after the freeze is the owner's edit, not « cause inconnue ».
7. **TTM shown as « 5 · 0 · 0 · 0 »** (edge): only the first quarter is shown.
8. **A missing study read « échec de l'enregistrement »** (blind + auditor): `MSG_STUDY_GONE`.
9. **The button was hidden on the demo** (auditor, AC 1): disabled, like its neighbours.
10. **The comparison could reopen expanded** (blind): it closes whenever the strip leaves the
    « diffère » state; the table's half columns never go negative in a narrow window.
11. Tests (auditor 5–7): `report::frozen` unit tests (each cause, AI validated, unknown, several
    causes once each in order, current price with and without a refresh, a removed year, the
    open inputs, `differs`, the UTC day); the provisional strip (state); the PDF « • » on the
    method and the entries rows, « Entrées : identiques / 1 modifiée(s) »; the export fixture
    with `Money` values carrying trailing zeros (their scale survives both exports).
    `@tr` unchanged (1172: the G3 edits add no literal).

Dismissed after checking: the « Full only » gate at run time rather than compile time —
`verdict_record` is also the CURRENT side of every comparison, and `freeze` is the only store
path in the app (the MCP fixture writes the blob as MCP would read any stored one); string-ordered
stamps (every stamp is the app clock's RFC 3339 UTC seconds, as in 8.7); the ratio written
« 5,2:1 » on screen and « 5,2 : 1 » in the PDF (each surface's existing convention, verdict bar
and PDF « Position »); `area-height: root.height` (the study screen's root IS the study area);
no `#[serde(other)]` on the frozen enums and an older build dropping the field (not in
production; the other contract enums follow the same rule).

Deferred (low): `current_price` / `ttm_eps` carry no provenance, so their cause is « rafraîchissement
du … » when a provider write happened since the freeze, else « cause inconnue » — an owner-typed
price after a refresh reads as the refresh (owner-pending, final report); re-validating an
identical verdict is allowed (AC 3's confirm) and adds a step and a history entry; a study that
does not normalize shows « entrées ouvertes : — » and its refusal reads « échec de
l'enregistrement »; the four quarterly inputs share the label « Trimestre »; `frozen_inputs`
mirrors the digest by construction, tested by count only; the comparison is rebuilt on each drag
tick and the history diff clones two studies per pair; the contract proptest never generates a
frozen verdict.

### File List

New: `contract/src/frozen.rs`, `contract/tests/frozen.rs`, `report/src/frozen.rs`,
`app/src/state/frozen.rs`, `app/src/viewmodel/frozen.rs`,
`app/ui/components/frozen_verdict_strip.slint`. Modified: `contract/src/{lib,study}.rs`,
`contract/tests/roundtrip.rs`, `report/src/{lib,form,pdf}.rs`, `app/src/state/{mod,undo,messages,
tests}.rs`, `app/src/viewmodel/{mod,engine,history,drafts}.rs`, `app/src/wiring/{push,judgment,
studies}.rs`, `app/src/posture.rs`, `app/ui/{state.slint, components/modal_dialog.slint,
screens/study_screen.slint, screens/settings.slint}`, `mcp/src/tools.rs`,
`mcp/tests/{support/mod,stdio_e2e}.rs`, `persistence/src/mcp_access.rs`,
`persistence/tests/{mcp_access,export,…}.rs` (the `Study` literals),
`_bmad-output/implementation-artifacts/sprint-status.yaml`.

## Change Log

- 2026-09-30 — create-story (5fd8c0b); dev-story T1–T12; headless walk v88/; status → review.
- 2026-09-30 — G3 review (3 layers) applied (86ccf96); re-walk v88g3/01–03; status → done.
