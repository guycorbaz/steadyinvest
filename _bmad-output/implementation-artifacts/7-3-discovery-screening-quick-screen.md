# Story 7.3 — Examen rapide (Quick Screen / Stock Check List) + criblage de la liste de suivi

Status: review (PR 1 — the examination; PR 2 — the criblage — to follow)

Spec: `_bmad-output/planning-artifacts/story-7-3-quick-screen-spec.md` (PR #229, merged by Guy
2026-09-24 without comment → the four defaults stand: the objective typed per session, the form's
five-year exponent, the criblage as PR 2, 7.5 to close as covered).

## Story

As Guy,
I want the beginner's check list on a ticker — six years of sales and EPS reduced to two compound
rates, five years of prices reduced to an average P/E, four conclusions against my own objective —
before a full study, and the same on my studies,
so that a first look costs one screen and the study starts from the data the look already fetched.

## Acceptance Criteria (from the spec §7)

1. **AC1 — The ladders.** Six usable years → the ten lines and the compound rate over the form's
   five years (`endpoints_cagr_pct` of the two-year averages); fewer → the real span, stated;
   fewer than three → « indisponible ».
2. **AC2 — The conclusions.** With an objective typed, lines 1 and 2 read « atteint » /
   « n'atteint pas » against it; without, « objectif non renseigné »; line 3 stays the reader's
   choice; line 4 words the P/E fact.
3. **AC3 — From a study.** « Examen rapide » on the open study uses its saved years without a
   fetch, hides « Créer l'étude », and « Retour » lands on the study.
4. **AC4 — From a fetch.** « Examiner un titre » on Études fetches through the configured chain,
   keeps the financials in the session, and « Créer l'étude » writes the study from them (one
   fetch); the in-progress fiscal year (no sales) is dropped as the study apply path does (#109).
5. **AC5 — Export.** A two-page portrait PDF in the study PDF's style, deterministic, neutral.
6. **AC6 — Posture.** New strings French inside `@tr()` / the report inventory; messages
   inventory 134 → 139; `@tr` floor re-based (768 → 850).

## Tasks / Subtasks

- [x] **Task 1 — core** `core/src/checklist.rs`: `quick_screen(years, present_price,
  present_eps) -> QuickScreenOutputs` (two `Ladder`s, `PriceRecord` with the five rows, totals,
  averages, the three facts, `PePosition`, `RateComparison`). Tests: the conversion table (27 % →
  5,0 %, 271 % → 30,0 %), a seven-year ladder, a short series' real span, the price record.
- [x] **Task 2 — report** `report/src/quick_screen.rs`: `QuickScreen` (formatted + keys) and
  `render_quick_screen` (A4 portrait). Tests: determinism / media box, empty + unavailable,
  templates, neutrality. `report/examples/render_quick_screen_demo.rs` for eyeballing.
- [x] **Task 3 — app**: `viewmodel/quick_screen.rs` (`quick_screen_view`, `meets_key`),
  the `QuickScreen` global + `QuickPriceRow`, `Studies.screen-open`,
  `screens/quick_screen.slint` (four cards, the reader's fields as `TextField` / `ChoiceChip`),
  the « Examiner un titre » card on Études, « Examen rapide » on the study screen,
  `wiring/quick_screen.rs` (examine / examine-study / close / objective-edited / create-study /
  export), `WorkerJob::QuickScreen` + `WorkerOutcome::QuickScreen` in the fetch worker, the
  `Session.quick_screen` slot.
- [x] **Task 4 — gates + headless verification** (below).
- [ ] **Task 5 — PR 2, the criblage**: « Examiner la liste » on Liste de suivi (the batched run,
  the « Criblage » card, per-row « Ouvrir l'examen », the quota stop).
- [ ] **Task 6 — Guy's on-display check**: an examination from a study, one from a fetch, the PDF.

## Dev Notes

- The form's conversion table is the fifth root even though the two-year averages sit four years
  apart; the screen states the base (« deux moyennes de deux ans, à 5 ans d'écart »).
- The present EPS is the TTM figure when the fetch or the study carries one, else the latest
  annual EPS; a non-positive EPS yields no P/E (never a negative multiple).
- Sales are the study's unit (full units when fetched — « 89 490 000 000 »), the same figure the
  size classification reads.
- The reader's fields (reasons, factors, P/E notes, objective, EPS outlook) live in the Slint
  global for the session; the PDF echoes them as typed.

### Verification (2026-09-24, headless on a copy of Guy's dossier)
- From the NESN.SW study: « Examen rapide » → the two ladders (2025/2024 vs 2020/2019, sales
  rate 0,4 %, EPS −10,2 %, « le BPA a augmenté moins vite »), the five-row price record
  (totals 120,1 / 93,5, averages 24,0 / 18,7, average of averages 21,4), the three facts
  (−40,2 % vs the 2021 high, sold as high in 5 of 5 years, P/E 26,7 above 21,4); objective « 1 »
  → both rates « n'atteint pas »; the « oui » chip; « Retour à l'étude » lands on the study.
- From a fetch: « Examiner un titre » ROG.SW / CHF → « Examen en cours… » → the examination
  « fournisseur : EODHD » (one real EODHD call ran through Guy's stored key — noted in the PR);
  « Créer l'étude » wrote ROG.SW with the fetched years (« L'étude a été créée avec les données
  récupérées ») and opened it. Finding fixed on the spot: the in-progress 2026 row (no sales,
  EPS 0) fed the EPS ladder and the price record — now dropped like the study apply path.
- The demo PDF: two portrait pages, the four sections, the reader's fields, the footer.
- Gates: fmt clean, clippy `-D warnings` clean, `cargo test --workspace` green.
