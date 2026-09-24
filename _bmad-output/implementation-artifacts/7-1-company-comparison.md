# Story 7.1 — Comparaison de sociétés (Company Comparison) + export (FR53)

Status: done

Spec: `_bmad-output/planning-artifacts/story-7-1-company-comparison-spec.md` (PR #226,
validated by Guy 2026-09-24: the picker on the Études list, up to five studies, the thirty rows
of the comparison form in four groups, a landscape PDF, no ranking).

## Story

As Guy,
I want up to five of my studies side by side on the thirty rows of the comparison form — growth,
management, price, and the study's own state — with a PDF of it,
so that choosing between candidates is one table of the same figures the study screens show,
never a walk through five studies with a notepad.

## Acceptance Criteria (from the spec §6)

1. **AC1 — One read path.** Every cell comes from the SAME `build_frame` the study screen uses:
   rows 1–4 the §1 growth outputs and the judgment's projections, 5–6 the §2 averages with their
   trend word, 8/9/11/15 derived (× 5, min / max over the §3 window), 10–19 and 21–23 the §3–§5
   outputs, 26 the payout average, 27 the quality flags (count + words), 29 the latest provider
   timestamp (else creation), 30 the ticker's exchange suffix. Rows 7, 24, 25 are not carried by
   the study model and read « — ».
2. **AC2 — Worded rows as keys.** Row 20 (zone) and row 28 (state, with « confiance réduite »)
   cross as keys; the screen and the report word them in their own inventories.
3. **AC3 — Absence honesty.** A missing figure is « — »; a study that cannot be read is a column
   of « indisponible »; a currency mix is stated in a band and prices stay native (FR28).
4. **AC4 — Export.** « Exporter PDF » renders the same cells A4 landscape (greyscale, neutral
   inventory, deterministic bytes, headers repeated per group), through the native save picker.
5. **AC5 — Posture.** New strings French inside `@tr()` / the report inventory; one new message
   (`MSG_COMPARISON_EXPORTED`, inventory 132 → 133); `@tr` floor re-based (706 → 768).

## Tasks / Subtasks

- [x] **Task 1 — report** `report/src/comparison.rs`: `Comparison` / `ComparisonColumn` (formatted
  rows + keys) and `render_comparison`; `pdf.rs` gains page dimensions (`Doc::landscape()`,
  `right()`, the landscape content translate). Row 27 keeps the count in the grid and lists the
  flags' words under it (wrapped); the trend arrows are dropped (no WinAnsi glyph). Tests:
  landscape media box + determinism, the key rows, row 27, an empty comparison, neutrality.
- [x] **Task 2 — view-model** `app/src/viewmodel/comparison.rs`: `comparison_column(study, frame,
  format)` (the one float → string boundary), `unavailable_column`; the engine's `fmt*` helpers
  opened `pub(crate)`.
- [x] **Task 3 — UI + wiring**: the `Comparison` global (row-major `cells`, headers, five picks),
  `Studies.compare-open`, the « Comparer des études » card on Études (five `Dropdown`s on the
  dossier's study tickers, pushed from `refresh_studies`), `screens/comparison.slint` (four
  cards, « Ouvrir l'étude » per column, « Retour »), `wiring/comparison.rs` (`push_comparison`,
  the export rail, open-study on top of the table).
- [x] **Task 4 — gates + headless verification** (below).
- [x] **Task 5 — Guy's on-display check** (2026-09-24): « ok pour une première version ; des
  améliorations seront à apporter à l'usage ». Improvements will be filed as they surface in use.

## Dev Notes

- The comparison is across studies, not positions: a ticker's study in any currency qualifies;
  the currency-mix band states the consequence instead of converting (FR28).
- `report/examples/render_comparison_demo.rs` renders a five-column fabricated comparison (one
  unavailable) for eyeballing the landscape layout.
- `Studies.compare-open` stays true under an open study, so « ‹ Retour aux études » from the
  study lands back on the table, which re-derives on mount (#94 rule).

### Verification (2026-09-24, headless on a copy of Guy's dossier — 3 studies)
- The picker card lists NESN.SW / NVDA.US / SCHN.SW in each drop-down; « Comparer » enables at
  two picks; the table shows the thirty rows (NVDA.US 46,6 % / 58,7 % growth, « 47,6 % · ↑
  hausse », zones « 140,70 – 1 046,25 »…, « Zone d'achat », 31,0:1, « 3 : PER haut jugé … »,
  « provisoire », 2026-09-24, US) with the currency-mix band (USD + CHF); « Ouvrir l'étude »
  opens NVDA.US and its « Retour » lands on the table; the table's « Retour » lands on the list.
- The demo PDF: two landscape pages, clipped headers, row 27 words listed under « Autres ».
- Gates: fmt clean, clippy `-D warnings` clean, `cargo test --workspace` green.
