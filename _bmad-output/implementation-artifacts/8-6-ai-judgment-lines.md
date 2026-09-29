# Story 8.6: AI judgment lines

Status: review

<!-- Created 2026-09-28 by create-story on branch feat/8-6-ai-judgment-lines (main with 8.1–8.5b).
     Autonomous Epic 8 run (Guy, 2026-09-28): questions resolved with the most conservative option,
     listed under « Decisions taken » for the final report. -->

## Story

As Guy,
I want an AI-proposed judgment drawn beside mine but inert until I accept it,
so that the AI can challenge my numbers without ever moving my verdict.

## Acceptance Criteria

Source: `epics.md` Story 8.6 (amended after 8.0 Q13) + the Epic 8 posture AC; UX spec
`ux-ai-assistance-surfaces.md` (validated 2026-09-27, Q1–Q16 defaults) §3.3 « Cell and judgment
marks », §5.5, §6, §7, §8, Q13, Q15; arch §Phase 4 A6/A7/A8; FR29, FR32, FR33, FR51, FR64, FR72,
FR74, FR77, NFR-U1, NFR-U2.

1. **AI lines on the charts (FR33, FR72, Q13).** For the open study, every **pending** judgment
   draft on a chart-draggable field is drawn on that chart **beside** the owner's line, never in its
   place:
   - §1 growth chart: `estimated_high_eps` and `estimated_low_eps` — a line from the last historical
     EPS (the owner lines' fixed origin) to the proposed value at the forecast edge, on the **same
     EPS log scale** as the owner lines;
   - §1: a `projected_eps_growth_pct` draft is drawn as the est-high line it **implies** — only
     when the owner has no direct `estimated_high_eps` (see Decision 2);
   - §3 P/E chart (#115): `judged_avg_high_pe` and `judged_avg_low_pe` — a full-width horizontal
     level at the proposed P/E on the same linear scale.
   The AI line is **dotted** (short strokes, distinct from the owner's solid lines and the 9/6 dash
   of est-low / judged-low), `chart-stroke-thin`, ink `text-mid`; it ends in a **hollow circle**
   marker at the forecast edge (§1) or the right edge (§3); it carries the label « IA {valeur} »
   (app-formatted value, the owner's number format and the field's display rule); it has **no drag
   handle** and is never selectable in « Ajuster : ». Three non-colour cues: dots + hollow marker +
   label (NFR-U1). A proposed value outside the chart's scale is clamped to the edge like a drag,
   the label keeps the exact value.
2. **Action chips (spec §5.5).** Under each chart's legend, when that chart has pending AI lines, a
   row « Propositions de l'IA : » with **one chip per pending field** — « ★ IA · {champ} : {valeur} »
   — rendered as `ActionButton`s (focusable, neutral ink), outside the « Ajuster : » selector and
   never a series-hued `ContextChip`. Every **other** pending judgment draft (projected sales
   growth, projected EPS growth when not drawn, recent severe low, present full-year dividend) gets
   the same chip under its `JudgmentField`, after any « hist. » chip; `forecast_low_option` gets
   « ★ IA · Option du bas prévisionnel : {libellé} » after the `ChoiceChip` group, which is untouched
   while pending. A stale draft's chip ends with « · périmée » (the 8.5a state word).
3. **One line per field (D4).** A target holds at most one pending draft (refused at submission,
   8.3), so a chart shows at most one AI line per field; the view model asserts it (a second pending
   draft on the same field is a data error → the overlay for that field is dropped and logged, the
   inbox still lists both).
4. **Inert while pending (FR72).** A pending AI line or chip changes **no** zone, U/D, 5-year
   potential, verdict, verdict state, alert or saved value: the owner chart geometry
   (`GrowthChartState` / `PeChartState` owner fields incl. the EPS/P/E scale bounds), the zone bar,
   the verdict bar, `risk-computed` and every alert fact are identical with and without pending
   judgment drafts. The AI value is **excluded** from the scale computation (the scale stays the
   owner's stable scale).
5. **Deciding from the chart (FR74, NFR-U2).** Click or Enter/Space on a chip opens the **8.5b
   decision dialog** for that draft (same `open_decision`), with every 8.5b rule unchanged (≤ 2
   actions: chip = 1, « Valider » = 2; Q5 focus; stale confirmation; edit before validating; target
   gone; refusals inline; undo). The AI line itself is not a click target (no second hit area on the
   chart, so a drag is never confused with a decision).
6. **After validation (FR29, FR33, A6).** A validated judgment draft becomes the study's judgment
   through 8.2b's `decide_draft` (zones recompute like any owner judgment change) and keeps
   `Judgment.ai_placed` for that field. While the slot is set, the caption « placée par l'IA ·
   validée le {JJ/MM/AAAA} » (`text-low`, caption size, local date) shows: at the line end for the
   four chart fields (§1 est-high / est-low, §3 high / low P/E), under the `JudgmentField` for the
   others, after the `ChoiceChip` group for `forecast_low_option`. It is app text (no AI-written
   string, Q15).
7. **Clearing (A6, FR51).** Any write that **changes** that field's value (drag commit, keyboard
   step, numeric entry, clear, « hist. » suggestion, option pick) clears the slot (8.2b rule, value-
   changing writes only) and the caption goes; the study history keeps the snapshot with the mark.
8. **Undo (FR32, FR77).** Ctrl+Z after a validation from a chip restores the prior judgment, records
   the draft `validated_undone`, redraws the pending AI line and chip; Ctrl+Y re-validates (8.2b/8.5b
   rails, unchanged). *Corrected at dev (Decision 9): `validated_undone` is a decided state (epics
   8.2b/8.5b, arch A8) — the undo restores the value and removes the caption; no pending line or
   chip comes back.*
9. **Live refresh.** A judgment draft arriving (poll, 8.5a), decided, undone or redone updates the
   open study's AI lines, chips and captions without reopening the study; a dossier switch or study
   close clears them.
10. **Never self-placed (FR33).** The system still never places or suggests a line: an AI line
    exists only for a pending AI draft; a caption only for an owner-validated one. No seed, no
    default, no derived line is ever labelled « IA ».
11. **Metamorphic (FR72, 8.3 AC 12 extended).** A test drives the app's **view-model path**: for
    every golden fixture and the v1/v8 corpus studies, with and without pending judgment drafts of
    every judgment field (including a projected-growth draft and a `forecast_low_option` draft),
    every computed output (`engine` view models), the owner chart geometry, the zone bar, the
    verdict state and every alert fact are equal.
12. **PDF (spec §7, deferred from 8.5b).** An AI-placed judgment (its `ai_placed` slot set) prints
    « † » after the figure (« * † » when also judged-marked), with the existing one-line note
    « † valeur proposée par une IA et validée par l'utilisateur » under the section when a « † » was
    printed; pending drafts print nothing. A study without AI marks stays **byte-identical** (the
    8.5b pinned hash stays green).
13. **Posture (Epic 8 posture AC).** Every new string from §3.3 verbatim; `@tr` floor and
    `MSG_*`/label inventories re-based with the delta stated; banned-verb gate; the AiFrame
    structural scan and the Rust AI-field guard stay green (chips, labels and captions carry **no**
    AI-written text — only app-formatted values, Q15); glossary entry « Ligne IA »; headless visual
    verification (DoD) on a seeded temp copy.

## Tasks / Subtasks

- [x] **T1 — Seed coverage (AC 13, walk).** Extend `mcp/examples/seed.rs` so `just mcp-seed <copy>`
  also submits, on a study with EPS and P/E history: `estimated_high_eps`, `judged_avg_high_pe`,
  `judged_avg_low_pe`, `recent_severe_low`, and — on a **second** study without a direct est-high —
  `projected_eps_growth_pct` (plus the existing `estimated_low_eps` and `forecast_low_option`). Keep
  the seed guard and idempotence untouched; one draft per field (D4).
- [x] **T2 — Pure overlay model (AC 1–4, 10).** New `app/src/viewmodel/ai_lines.rs`:
  - [x] `JudgmentOverlay { field, draft_id, value_label, stale, chip_label }` and
    `pending_judgment_overlays(study: &Study, drafts: &[DraftRecord], format) -> Vec<...>` —
    reads pending judgment drafts of this study only (8.5a `DraftRecord`), parses with the 8.2b
    registry (`DraftField`, `parse_value`), freshness with `draft_freshness`, labels with the 8.5a
    display rules (`format_scaled`, `option_label`); an unparsable value → overlay dropped + logged
    (never a guessed value).
  - [x] `ai_growth_lines(frame, overlays) -> AiGrowthLines { high: AiLine, low: AiLine }` and
    `ai_pe_levels(frame, judgment, overlays) -> AiPeLevels` where `AiLine { visible, commands,
    marker_commands, y, label }` — computed with the **same** scale bounds as `growth_chart` /
    `pe_chart` (factor out the bound computation into a shared helper so both use one source; the
    AI value is not added to the bounds), clamped to the edge.
  - [x] `dotted_line(p0, p1)` next to `dashed_line` (short strokes, e.g. 2 / 5 viewbox px — distinct
    from 9 / 6) and `hollow_circle(cx, cy, r)` (arc commands, stroked, no fill).
  - [x] Implied est-high for a `projected_eps_growth_pct` draft: compute the engine's derived value
    on a **clone** of the study with the proposed growth (and no direct est-high) through
    `report::form::build_snapshot` → `outputs().growth.estimated_high_eps` — never persisted, never
    shown as a verdict; drawn only when the owner has no direct `estimated_high_eps` (Decision 2).
  - [x] Validated captions: `ai_placed_captions(judgment) -> per-field Option<String>` from the
    `AiPlaced` slots (« placée par l'IA · validée le {JJ/MM/AAAA} », local date).
- [x] **T3 — Slint (AC 1, 2, 5, 6).**
  - [x] `state.slint`: `AiLine` struct; `Studies.ai-growth` / `Studies.ai-pe` / `Studies.ai-chips`
    (`[AiChip { draft-id, label, field-key }]` per surface) / `Studies.ai-captions` (per field key).
  - [x] `growth_chart.slint` / `pe_history_chart.slint`: draw the AI `Path`s (dotted, thin,
    `text-mid`) and the hollow marker **after** the owner lines, no `TouchArea`; label « IA
    {valeur} »; caption at the line end when validated; « Propositions de l'IA : » chip row under the
    legend, outside « Ajuster : ».
  - [x] `judgment_field.slint`: optional `ai-chip-label` / `ai-draft-id` (chip after « hist. ») and
    `ai-caption`; study_screen passes them for `sales_growth`, `eps_growth` (when not drawn),
    `recent_severe_low`, `dividend`; the `forecast_low_option` chip + caption after the ChoiceChips.
  - [x] Chip activation → `Drafts.open-draft(id)` (8.5b callback), Enter/Space, key-repeat ignored
    (8.5b G3 rule), focus returns to the chip on close.
- [x] **T4 — Wiring (AC 4, 8, 9).** `wiring/drafts.rs` keeps the open study's pending judgment
  drafts from the last `push_drafts` read (no extra query per edit); a new `push_ai_judgments(ui,
  state)` called from `push_form` (after the owner chart push, never before) and from `push_drafts`
  (when the open study's pending set changed) and after decide / undo / redo; cleared in
  `forget_dossier` and on study close. The owner chart push functions are **not** modified except
  to share the bound helper.
- [x] **T5 — PDF (AC 12).** `report/src/pdf.rs`: a `judged_ai(figure, ai)` (or an `ai` flag on the
  judgment print path) using `AI_SIGIL`; wire the seven printed judgment figures to their
  `ai_placed` slot; the note iff a « † » was printed on that page/section; tests: an AI-placed
  est-high prints « * † » and the note; the pinned no-AI hash unchanged; widths still fit.
- [x] **T6 — Tests (AC 3, 4, 10, 11).**
  - [x] View-model geometry: dotted line = many short segments, distinct from `dashed_line` output;
    hollow marker present; AI y on the owner's scale; clamping; no AI line without a pending draft;
    no line for a seed/derived owner line; implied est-high only without a direct est-high; stale
    chip suffix; duplicate pending on one field → dropped.
  - [x] Owner geometry invariance: `growth_chart` / `pe_chart` outputs byte-equal with and without
    overlays computed (the overlay functions take `&` only).
  - [x] Metamorphic (AC 11) in `app/src/viewmodel/verify.rs`, next to the 8.3 suite: engine view
    models, zone bar, verdict state, risk computed and alert facts equal with and without pending
    judgment drafts of every field.
  - [x] State: validate from a chip → `ai_placed` set, caption shown; a value-changing write clears
    it; a same-value write keeps it (8.2b rule); undo → `validated_undone` + overlay back; redo.
  - [x] Posture: new strings inventoried; AiFrame scan + AI-field guard unaffected (extend the
    guard's allow-list only if a new function reads AI fields — it should not: overlays read
    `proposed_value`, not `comment`).
- [x] **T7 — Glossary (AC 13).** « Ligne IA » — « Proposition de jugement d'une IA, tracée en
  pointillé avec un rond creux et l'étiquette « IA » ; elle ne change rien tant qu'elle n'est pas
  validée. » (`@tr`).
- [x] **T8 — Headless walk (AC 13, DoD).** On a temp copy of `corpus/v8.db` migrated to v9, temp
  HOME/XDG, provider « none », seeded with `just mcp-seed` (T1): §1 with est-high + est-low AI lines
  beside owner lines (and a seed-dimmed owner line); §1 implied-growth line on the second study;
  §3 high/low P/E AI levels; chip rows; judgment-field chips incl. `forecast_low_option`; stale chip
  after an owner edit; validate from a chip (2 actions) → caption at the line end; value-changing
  edit clears it; undo/redo; zone bar and verdict bar unchanged with pending drafts (side-by-side
  capture); keyboard: Tab to chips, Enter opens the dialog, focus returns; 1280 and 1600 px;
  confusability capture of the dotted AI line against est-low dash and a seed-dimmed line. Screens
  in the session scratchpad `v86/`. Before/after listing of `~/.config/steadyinvest` and
  `~/.local/share/steadyinvest` unchanged.
- [x] **T9 — Record.** Story record, posture deltas, decisions, sprint-status 8-6 → review.

## Dev Notes

### What exists (read before changing)

- **Owner charts** — `app/src/viewmodel/chart.rs`:
  - `growth_chart(frame, format)` (≈ l.218): EPS log scale `(e_lmin, e_lmax)` from historical EPS +
    the 30 % fan headroom + the least-squares seed band — **excluding** the live judged values so
    the scale does not move during a drag. `endpoint(judged, shown, dashed)` draws est-high solid,
    est-LOW **dashed** (`dashed_line(..., 9.0, 6.0)`, issue #121); a line shown at the seed is
    `derived` → dimmed (opacity 0.55 in `growth_chart.slint`). The est-high value is the engine's
    `outputs().growth.estimated_high_eps` (direct value, else derived from `projected_eps_growth_pct`
    — `core/src/ssg/growth.rs` `compute`, direct wins; est-low is direct-only).
  - `pe_chart(frame, judgment, format)` (≈ l.514): linear P/E scale from per-year highs/lows + the
    5-yr average seeds (judged values excluded); judged-high solid, judged-low **dashed**; seeds dimmed.
  - **Correction to the 8.0 spec rationale (§5.5):** the spec says « on §1 the owner's lines and
    seeds are solid »; in the code the §1 est-LOW line **is dashed** (issue #121, in the view model,
    not in the `.slint`). Dots remain the one free pattern on both charts, so the decision stands —
    only the rationale sentence is wrong (Decision 7).
  - `dashed_line` bakes dashes into `M…L` segments (Slint `Path` has no dasharray) — `dotted_line`
    follows the same pattern with shorter strokes; Slint 1.17's `Path` has `stroke-line-cap`
    (checked in the i-slint-compiler 1.17 builtins) — set `round` on the AI path so the short
    strokes read as dots.
- **Chart state** — `app/ui/state.slint` `GrowthChartState` (l.328) / `PeChartState` (l.366):
  add **separate** AI properties; do not add AI fields into the owner structs' semantics.
- **Growth chart component** — `app/ui/components/growth_chart.slint`: « Ajuster : » row with
  `ContextChip`s (series-hued, the drag-context selector — the AI chips must **not** be these);
  `LegendItem`; the drag strip `TouchArea` (only owner lines). `pe_history_chart.slint` mirrors it.
- **Judgment fields** — `app/ui/screens/study_screen.slint`: `JudgmentField` keys `sales_growth`,
  `eps_growth` (l.996/1006), `est_high_eps`, `est_low_eps` (l.1035/1043), `high_pe`, `low_pe`
  (l.1187/1193), `recent_severe_low` (l.1256), `current_price` (l.1262, **not draftable**, D6),
  `dividend` (l.1267); the forecast-low `ChoiceChip` group (l.1230–1248, wire names
  `avg_low_price_last_5y` in the app vs `avg_low_price_last5y` in the contract — use the existing
  mapping, never hand-roll). Wire ↔ `DraftField`: `state/cells.rs` `JUDGMENT_WIRE_KEYS` /
  `judgment_draft_field` (l.673–689). `judgment_field.slint` already has the « hist. » chip slot.
- **Drafts in the app** — `app/src/state/drafts.rs` `read_inbox()` (pending only, + studies +
  archived) and `draft_freshness`; `app/src/wiring/drafts.rs` `push_drafts` (the poller path,
  `INBOX` cache), `open_decision(ui, state, id)` (8.5b), `wire_decisions` (`on_open_draft`);
  `app/src/viewmodel/drafts.rs` `option_label`, `DRAFTS_USER_FACING_LABELS`,
  `DECISION_USER_FACING_LABELS`, state words (« périmée »).
- **AI marks** — `contract/src/study.rs` `AiPlaced` (9 slots) and `contract/src/draftable.rs`
  `DraftField::{ai_slot, judgment_value, set_judgment}` (8.2b). The 8.2b judgment writers clear a
  slot only on a value change (8.2b G3 item 8).
- **PDF** — `report/src/pdf.rs`: `AI_SIGIL` / `AI_NOTE` (l.153–155) used for cells (8.5b, l.550–
  616, 649–740); `judged()` (l.184) marks judged figures; judgment figures printed at ≈ l.275–415
  from `study.judgment`; pinned no-AI hash test `a_study_without_ai_origin_renders_byte_identical_to_main`.
- **Metamorphic suite** — `app/src/viewmodel/verify.rs` (8.3, l.332–482) runs persistence-level
  pending drafts through `report::form::build_snapshot`; 8.6 adds the view-model/alert layer.
- **Seed** — `mcp/examples/seed.rs` submits a study draft, a note, a cell (`eps`), `estimated_low_eps`
  and `forecast_low_option` today (l.140–184).

### Guardrails

- The owner chart functions and `core` are **not** changed in behaviour. The only allowed refactor
  is extracting the scale-bound computation so the AI overlay uses the identical bounds.
- The AI overlay is read-only over `&Study` + drafts; nothing it computes is persisted; the implied
  est-high clone is a local value dropped after drawing.
- No AI-written text anywhere in 8.6 surfaces: chips/labels use `proposed_value` parsed and
  formatted by the app, never `comment`/`origin_*` (the comment is in the dialog's AiFrame, 8.5b).
- Pending drafts are read from the inbox read already done by `push_drafts` — no new query on every
  `push_form`.
- Q15: compact marks carry no AI-written text; the footer disclaimer + the dialog's AiFrame satisfy
  FR64.
- Safety (Epic 8 preamble): headless on temp copies with temp HOME/XDG, provider « none », no
  provider fetch, no real rfd portal, never register the MCP server, never touch the real config or
  data dirs. `CARGO_BUILD_JOBS=4`; never `git add -A`.

### Posture baseline (main after 8.5b)

`@tr` floor **1106**; `USER_FACING_MESSAGES` **247**; `DRAFTS_USER_FACING_LABELS` /
`DECISION_USER_FACING_LABELS` as on main. Expected: new `@tr` strings « Propositions de l'IA : »,
« IA {} », « ★ IA · {} : {} », « placée par l'IA · validée le {} », « · périmée » (if not already
inventoried), glossary term + definition — state the measured delta.

### Testing standards

Pure view-model functions unit-tested in their module; state tests in `app/src/state/tests.rs`;
metamorphic in `verify.rs`; PDF in `report/src/pdf.rs` tests; all gates: `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `cargo deny check`.

### Decisions taken (owner-pending, for the final report)

1. **The AI line is not a click target** — only the chips open the dialog (no hit area competing
   with the drag strip).
2. **Implied est-high from a growth draft is drawn only when the owner has no direct est-high** —
   with a direct value the growth draft would not move the est-high on validation (direct wins in
   `core`), so a line would mislead; the chip is shown instead.
3. **Stale pending judgment drafts are still drawn**, with the chip suffix « · périmée »; target-gone
   does not arise for judgments (the study exists while its drafts do).
4. **A proposed value off-scale is clamped to the chart edge**, label exact (the owner-drag rule).
5. **Line label value** for a growth draft: the implied est-high EPS (the axis unit); the chip
   carries the proposed growth %.
6. **`forecast_low_option`**: chip + caption only (no chart line).
7. **Spec §5.5 rationale corrected** (est-low on §1 is dashed in code); dots stay the free cue.
8. **Captions are app text** (date only, no client/model) — the full origin is in the history
   Détail's AiFrame.
9. **AC 8 corrected**: an undone validation stays `validated_undone` (decided); the undo restores
   the value and drops the caption, the AI line does not come back (the epics and 8.2b/8.5b say
   so; the AC text contradicted them).
10. **§1 AI labels right of the drag strip**, beside their marker (left of the strip, the fan's
    sloped lines cross them); §3 keeps them left of the strip under the level. Two labels clamped
    to one edge: the low one steps a line aside.
11. **Captions above a high line, below a low one** (a sloped §1 line otherwise strikes them).
12. **A direct proposal's line label is its chip's value** (same digits on chart and chip, 8.5a
    `same_decimals`); the implied est-high stays formatted as the axis unit (Decision 5).
13. **Focus after a decision from a chip that is then gone** goes to the study (Ctrl+Z / Ctrl+Y);
    a cancelled dialog still returns it to the chip (8.5b F7).

### Project Structure Notes

New: `app/src/viewmodel/ai_lines.rs`. Touched: `viewmodel/chart.rs` (bound helper + dotted/hollow
helpers), `viewmodel/mod.rs`, `wiring/drafts.rs`, `wiring/push.rs`, `app/ui/state.slint`,
`components/growth_chart.slint`, `components/pe_history_chart.slint`,
`components/judgment_field.slint`, `screens/study_screen.slint`, `screens/settings.slint`
(glossary), `app/src/posture.rs`, `app/src/viewmodel/verify.rs`, `app/src/state/tests.rs`,
`report/src/pdf.rs`, `mcp/examples/seed.rs`.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 8.6]
- [Source: _bmad-output/planning-artifacts/ux-ai-assistance-surfaces.md#5.5, #3.3, #6, #7, #8, #9 Q13/Q15]
- [Source: _bmad-output/planning-artifacts/architecture.md#Phase 4 A6, A7, A8]
- [Source: app/src/viewmodel/chart.rs — growth_chart, pe_chart, dashed_line]
- [Source: core/src/ssg/growth.rs — compute (direct est-high wins over derived)]
- [Source: _bmad-output/implementation-artifacts/8-5b-deciding-drafts.md — decision dialog, undo]
- [Source: _bmad-output/implementation-artifacts/8-2b-ai-origin-staleness-and-decisions.md — AiPlaced, clearing rule]
- [Source: docs/review-checklist.md]

## Dev Agent Record

### Agent Model Used

Claude Opus 5.5 (claude-opus-5-5)

### Debug Log References

- Posture scan (`ai_written_text_is_read_only_inside_an_ai_frame`) flagged a layout property named
  `ai-row-h` (any `ai-*` name read outside an AiFrame) → renamed `chip-row-h`.

### Completion Notes List

- T1–T7 as specified (commits b027008, 9f27c12, 51949e6, 95c1769). Posture: `@tr` floor
  1106 → 1113 (+7, measured: chip, stale chip, row label, line label, caption, glossary term and
  definition); `AI_LINES_USER_FACING_LABELS` 0 → 1 (« Option du bas prévisionnel »);
  `USER_FACING_MESSAGES` unchanged (247). Gates: fmt, clippy `-D warnings`, `cargo test
  --workspace` 1414 passed, `cargo deny check` ok.
- **T8 headless walk** (verify skill, Xvfb :94, temp HOME / XDG, provider « none »; real
  `~/.config/steadyinvest` and `~/.local/share/steadyinvest` listed + md5 before/after —
  unchanged). A fresh copy of `persistence/tests/corpus/v8.db`, migrated to v9 by one app launch,
  plus two studies built from golden g01 (DEMO1 with a direct est-high, DEMO2 without) inserted
  with Python — the corpus' only study (NESN, two years, degenerate figures) cannot show a
  realistic chart nor the implied-growth line; `just mcp-seed` then seeded DEMO1 (every chart
  judgment + severe low + option) and DEMO2 (projected EPS growth). The seed must run in the real
  environment: its guard reads the REAL app config to refuse the real dossier (given the temp
  XDG it saw the copy as « real » and refused — the guard works). Screens in the session
  scratchpad `v86/`: 03 (NESN, both P/E proposals off-scale → labels overlapped, fixed), 12 §1
  est-high/est-low AI lines + chip row, 13 §3 levels, 15 zone bar unchanged with 8 pending drafts
  (g01's 44,79 / 34,06 / 23,33 / 12,60) + field and option chips, 17c chip → dialog (focus
  « Valider »), 18 validated → caption (overlapping the line → fixed), 19/24 fixed layout,
  20 Enter-validation of est-high, 26–28 §3 validate → Ctrl+Z (« Validation annulée… ») →
  Ctrl+Y, 32–33 value-changing edit clears the est-high caption and keeps est-low's, 35 stale
  chip « · périmée », 36–38 Shift+Tab to the chip, Enter → stale dialog, Esc → focus back on the
  chip, 40–41 severe-low and option validated → captions under the field / after the chips,
  44 DEMO2 implied est-high « IA 2,25 » beside the owner's derived 2,36, 45–46 zoomed
  confusability (dotted grey + hollow ring vs est-low's pink 9/6 dash vs the thin seed fan).
  1280 (00–30) and 1600 (31–46).
- **Found and fixed in the walk** (194319a): §1 AI labels crossed by their own rising line and two
  clamped labels printed over each other; captions struck through by a falling line; « IA 11 »
  vs chip « 11,0 »; the chart min-height never counted the legend row (pre-existing since #121,
  hidden by the chip row) so the next row covered the legend; Ctrl+Z dead after a chip decision
  (the focused chip vanished with the dialog).
- **Seen, not changed**: under Xvfb's software renderer, the partial repaint when the dialog opens
  left bits of chart paths over the modal (17); a full repaint (resize) shows it clean (17c) —
  a headless-renderer artifact, not reproduced after a full frame. At 1280 the study form is
  wider than the window (right-hand fields clipped) — pre-existing, noted in 8.5b.
- No UI-level harness: the wiring (focus return, redraw on read) is verified headless only.

### File List

New: `app/src/viewmodel/ai_lines.rs`, `app/ui/components/ai_judgment.slint`.
Modified: `app/src/viewmodel/{chart,drafts,mod,verify}.rs`, `app/src/wiring/{dialog,drafts,push,studies}.rs`,
`app/src/state/tests.rs`, `app/src/posture.rs`, `app/ui/state.slint`,
`app/ui/components/{growth_chart,pe_history_chart,judgment_field}.slint`,
`app/ui/screens/{study_screen,settings}.slint`, `report/src/pdf.rs`, `mcp/examples/seed.rs`,
`_bmad-output/implementation-artifacts/sprint-status.yaml`.

## Change Log

- 2026-09-28 — create-story (37588a0).
- 2026-09-29 — dev-story: T1–T7 (b027008, 9f27c12, 51949e6, 95c1769); headless walk v86/ and its
  fixes (194319a); AC 8 corrected (Decision 9); status → review.
