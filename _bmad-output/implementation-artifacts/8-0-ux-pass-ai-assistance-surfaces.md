# Story 8.0 — UX pass: AI-assistance surfaces

Status: in-progress — spec revision 2 (after G3), ready for Guy's review (spec PR, like PR #206)

## Story

As Guy,
I want every Epic 8 surface designed and worded before it is built,
so that the inbox, the AI marks, the notes and the frozen verdict fit the app as the 7.0 pass
made Epic 7 fit — neutral voice, colour budget, keyboard-first.

Spec: `_bmad-output/planning-artifacts/ux-ai-assistance-surfaces.md` (this story's deliverable;
no code). Origin: G2 (PR #255, merged 2026-09-27), owner decisions O1–O7, D1–D11.

## Acceptance Criteria (epics.md, Story 8.0)

1. **AC1 — Surfaces specified** with layouts and states (empty, loading, « indisponible »,
   refused): the Propositions destination with its « À traiter » and « Registre » views (spec
   §5.1–5.2), the pinned-area order (§5.3), the study reminder and the draft-study signal (§5.4),
   the `AiFrame` and its exemptions (§4.1), chart lines and action chips for judgment drafts (§5.5),
   notes (§5.6), history (§5.7), the ✦ cell mark and its confusability entry (§5.8), « Valider
   l'étude » and the frozen/current comparison (§5.9), keyboard (§6).
2. **AC2 — French wording** fixed as one list (§3.3) incl. MCP refusal reasons and their language
   (French message + English code); open choices listed with defaults (§9 Q1–Q15).
3. **AC3 — Report impact decided** (§7): study PDF (Helvetica/WinAnsi only) — notes not printed,
   AI-origin figures « † » with a legend, frozen verdict block with « • » markers; comparison and
   review unchanged by decision.
4. **AC4 — UX spec updated:** the "margin voice"/"clerk of memory" passages marked superseded; the
   "no suggested line" statements qualified with the [P4] labelled, dotted, inert AI line; UX-DR30
   in `epics.md` points to the addendum.
5. **AC5 — Validated by Guy** in the spec PR (answers to Q1–Q15 folded into §3.3 before merge).

## Tasks / Subtasks

- [x] Task 1 — Read the real UI (nav rail `app.slint`, `study_screen.slint` incl. `RationaleNote`,
  verdict bar, notice slot, history panel; `ModalDialog`, `PanelCard`, `StatusBand`,
  `editable_cell` marker columns, `trust_markers`, `growth_chart` #121 selector, `pe_history_chart`
  #115, `judgment_field` « hist. » chip; `report/src/pdf.rs` WinAnsi encoding) and ground every
  placement in it.
- [x] Task 2 — Write the addendum (`ux-ai-assistance-surfaces.md`, modelled on
  `ux-entry-dialogs-refusals-cards.md`).
- [x] Task 3 — Mark superseded / qualify passages in `ux-design-specification.md` (7 edits, each
  tagged G2 2026-09-27); point UX-DR30 to the addendum.
- [x] Task 3b — G3 review of the draft (e7afcd9) applied as revision 2: glyph « ✦ » (« ◆ » is the
  app's alert glyph), AI-text containment + exemptions, cause-neutral stale wording, frozen/current
  table rows, WinAnsi PDF markers, dotted AI line + per-field action chips, StatusBand action slot,
  missing strings, one Propositions card with « À traiter » / « Registre » views, pinned-area
  order; Q12–Q15 added.
- [ ] Task 4 — Guy's review of the spec PR; fold his answers to Q1–Q15 into §3.3; merge.
- [ ] Task 5 — After merge: status → done; the UI stories (8.1, 8.5a, 8.5b, 8.6, 8.7, 8.8) copy
  §3.3 verbatim. If Q13's default holds, amend Story 8.6 (a forecast P/E becomes a chart line).

## Dev Notes

- Nav re-index: « Propositions » becomes destination 4, « Réglages » 4 → 5: `app.slint` screen
  block + a new arm `4` in `wiring/mod.rs` (5 falls through `_`). The rail count is a separate
  label; the top-bar title stays plain.
- `StatusBand` gains an optional action slot (8.5a; reused by 8.8).
- 8.3 must add the ticker/currency identifier check (`identifier_invalid`) that the AiFrame
  exemption for draft-study identifiers relies on.
- The ✦ cell mark shares the trailing-bottom slot with the stale « ◦ »; exclusive because a
  validated AI value is manual (freshness `Current`) — 8.5b verifies against `contract/src/cell.rs`.
  8.5a checks the bundled fonts' coverage of « ✦ ».
- PDF: Helvetica / WinAnsi only (`winansi_byte`) → « † » (0x86) and « • » (0x95).
- Slint `Path` has no dash attribute → the dotted AI line is built as segments in
  `viewmodel/chart.rs` (dashes are already the est-LOW line and the seeds).

## Dev Agent Record

- 2026-09-27: spec drafted by Claude (fork of the G2 session); no code.
- 2026-09-27: revision 2 after the G3 review of e7afcd9.

### File List
- `_bmad-output/planning-artifacts/ux-ai-assistance-surfaces.md` (new)
- `_bmad-output/planning-artifacts/ux-design-specification.md` (superseded / [P4] marks)
- `_bmad-output/planning-artifacts/epics.md` (UX-DR30 pointer)
- `_bmad-output/implementation-artifacts/sprint-status.yaml` (8-0, epic-8 → in-progress)
