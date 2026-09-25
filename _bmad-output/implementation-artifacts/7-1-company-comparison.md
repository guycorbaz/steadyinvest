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
   trend word and the number of years they really average (« moyenne n ans », G1 final review),
   8/9/11/15 derived (× 5, min / max over the §3 window), 10–19 and 21–23 the §3–§5 outputs, 26
   the payout average, 27 the quality flags (count + words), 29 the latest provider date (« — »
   without one — G1, spec amended), 30 the ticker's listing venue when it is a known venue
   (« — » otherwise; a share class such as BRK.B is not an exchange). Rows 7, 24, 25 are not carried by
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

### Review Findings — G1 catch-up review (2026-09-25, #237)

3-layer adversarial review of PR #227, checked against main 73a7b19. Decisions are Guy's
(2026-09-25).

- [x] [Review][Decision] Columns keyed by ticker, not by study; « Ouvrir l'étude » runs its own ticker lookup — **the pickers list studies (id carried end to end), « TICKER · CUR » when ambiguous** [app/src/wiring/comparison.rs:45,89-92, app/src/state/watchlist.rs:39-46]
- [x] [Review][Decision] PDF runs to two landscape pages (spec §4: one) and its header omits the decision date (spec §3) — **two pages ratified; the decision date is added to the column header** [report/src/comparison.rs:206]
- [x] [Review][Patch] A picked ticker stays in the other lists (spec §2); duplicate picks enable « Comparer » and give a one-column table [app/ui/screens/dashboard.slint:428-439, app/src/wiring/comparison.rs:27-29]
- [x] [Review][Patch] Picks, table and notice survive a dossier switch / restore; the export notice is never cleared [app/src/wiring/journal.rs:130-145,446-449, app/src/wiring/comparison.rs:195]
- [x] [Review][Patch] A missing study (`Ok(None)`) is shown « indisponible » like a read failure; the picker hides a read failure behind an empty list [app/src/wiring/comparison.rs:45-54, app/src/wiring/studies.rs:70-78]
- [x] [Review][Patch] Row 27 prints « 0 » quality signals when nothing could be evaluated [app/src/viewmodel/comparison.rs:129-131]
- [x] [Review][Patch] Rows 9 / 11 / 15: min / max over the known years only, shown as the 5-year figure [app/src/viewmodel/comparison.rs:92-105]
- [x] [Review][Patch] Row 29 labels the creation date as the data date; row 30 derives « B » from BRK.B [app/src/viewmodel/comparison.rs:30-36,173-174]
- [x] [Review][Patch] Unavailable column header dangles: « · » on screen, « ROG.SW () » in the PDF [app/ui/screens/comparison.slint:123, report/src/comparison.rs:206]
- [x] [Review][Patch] Row 20 zone wording differs: screen follows `Labels.zone-*`, PDF prints « zone basse / médiane / haute » (AC4) [app/ui/screens/comparison.slint:43-46, report/src/comparison.rs]
- [x] [Review][Patch] PDF: the second header row overwrites the repeated grid header — a continuation page loses tickers and currencies; header rows can be orphaned at a page foot [report/src/pdf.rs:1040-1049, report/src/comparison.rs:223-226]
- [x] [Review][Patch] The nav rail closes the comparison without `Comparison.on_close` [app/ui/app.slint:131-132]
- [x] [Review][Patch] Tests: the derived rows 8 / 9 / 11 / 15 and parity with the study screen (spec §5, AC1) [app/src/viewmodel/comparison.rs]

Fixed in PR C (G1, branch `fix/g1-c-comparison`): the items checked above, and decisions 3
(pickers list studies) and 9b (decision date in the PDF header). The export notice is cleared on
every « Comparer »; clearing on a dossier switch, the PDF header repeat and the nav-rail close are
in later PRs. Open for Guy: row 27 reads « 0 » only when every quality rule could be evaluated
(else « — »); row 29 reads « — » without a provider date (the spec's decision-date fallback was
dropped — spec amended).

Fixed in PR G (G1, branch `fix/g1-g-dossier-switch`): every dossier change (open, create, recent,
reclaim, restore) clears the comparison through one helper; the nav rail and the programmatic
routes to Études close it through its Rust close path.

Fixed in PR F (G1, branch `fix/g1-f-pdf-layout`): every header row repeats after a page break and
none is orphaned at a page foot (the item checked above; `report/src/pdf.rs`,
`every_header_row_repeats_after_a_break_and_none_is_orphaned`).

### Review Findings — G1 final review (2026-09-25, #237)

- [x] [Review][Patch] PDF quality-signal list keyed by bare ticker: two columns of one ticker are indistinguishable — **each line is named by its column's header identity « TICKER (CUR) · date » (the ticker carries « · n »)** [report/src/comparison.rs:293-305]
- [x] [Review][Patch] The picker resolves a pick by re-matching its label, not by index (the dropdown contract) [app/ui/screens/dashboard.slint:435-439, app/src/wiring/comparison.rs:352-372]
- [x] [Review][Patch] A gone pick counts toward the ≥ 2 distinct studies « Comparer » needs — **only listed studies count; a failed listing marks none gone** [app/src/wiring/comparison.rs:126-134,151]
- [x] [Review][Decision] Rows 5 / 6 say « moyenne 5 ans » over fewer years — **Guy: say the real number. `core` states the years each §2 average runs over; the label reads « moyenne n ans » when the columns agree, else « moyenne » with each cell naming its span (« sur 3 ans »), screen and PDF; the study screen's §2 column title says the same (« Moy. n ans », « Moy. a / b ans » when PTP and ROE differ — label only)** [app/src/viewmodel/comparison.rs:213-214]
- [x] [Review][Patch] Story record stale: the PDF header item fixed but unchecked; AC1 rows 29 / 30 wording [this file]

Fixed in the G1 final-review branch `fix/g1-m-comparison-screen`.

G3 review of the final-review branch: when PTP and ROE spans differ, the study screen's §2 title
stays « Moyenne » and each average cell names its span (« Moy. a / b ans » read as « a of b »);
the study PDF reads core's span counts. The « sur n ans » cells are tested through
`name_average_spans` with a three-year study.
