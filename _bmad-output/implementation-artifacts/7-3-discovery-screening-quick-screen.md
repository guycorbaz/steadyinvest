# Story 7.3 — Examen rapide (Quick Screen / Stock Check List) + criblage de la liste de suivi

Status: done (PR 1 — the examination, #231; PR 2 — the criblage, #232; final test on main 2026-09-24)

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
- [x] **Task 5 — PR 2, the criblage**: « Examiner la liste » on Liste de suivi (the batched run,
  the « Criblage » card, per-row « Ouvrir l'examen », the quota stop). `WorkerJob::Screening`
  (batch + row + the run's `stop` latch) / `WorkerOutcome::Screening` + `ScreeningSkipped`;
  `viewmodel/screening.rs` (row states, `years_used`, the formatted row — tested);
  `wiring/screening.rs` (plan, launch, outcome, open, close); the `QuickScreen.from-watchlist`
  origin; `@tr` floor 850 → 892; messages inventory unchanged (139 — every new word is `@tr`).
- [x] **Task 6 — the final check** (2026-09-24, on main, Guy's go to use his key): an examination
  from a study, the criblage with one real EODHD fetch (ROG.SW), « Examiner un titre » →
  « Créer l'étude » (2016–2025 CHF, no in-progress row), the PDFs.

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
  rate 0,4 %, EPS −4,4 % [corrected 2026-09-24: « −10,2 % » is the report crate's demo study (EPS 2025 3,56 · 2024 4,13), not Guy's dossier, whose EPS (2025 3,51 · 2024 6,95 vs 2020 4,29 · 2019 8,84) give −4,45 %, as the screen and the criblage show], « le BPA a augmenté moins vite »), the five-row price record
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

### PR 2 — the criblage (2026-09-24)

Decisions taken in code (for Guy's review):
- **The quota stop is latched by the worker**, not the UI: the row whose fetch returns the usage
  limit and every row still queued behind it read « non examiné (limite d'usage) »; the card
  carries a band saying the limit interrupted the run. Any other failure → « indisponible » on
  that row only. The latch is per run (its own flag), independent of the holdings / FX cancel.
- **A studied ticker** = the linked study, else the newest same-ticker study (the watch link's own
  auto-match) — examined at once from its saved years, no fetch. « Étude : oui / non » reads that.
- **A fetched row has no currency** (a watch item carries none): its examination names none and
  offers no « Créer l'étude » (Études' « Examiner un titre » asks for the currency). The PDF head
  then names the ticker alone.
- **Re-launching** supersedes the run in flight (its queued rows drain unfetched, its late results
  are ignored by batch number); « Fermer le criblage » does the same. Nothing is persisted.
- **No provider** (« aucun ») → the unstudied rows read « indisponible » and the provider refusal
  is shown once; the studied rows stand.

Verification (headless, a copy of Guy's dossier with NESN.SW / ROG.SW / SCHN.SW watched, the
isolated config's provider set to « aucun » — no real fetch, see the key warning):
- « Examiner la liste » → the refusal « Aucun fournisseur de données n'est sélectionné… », then the
  card: NESN.SW 6 / 6 · 0,4 % · −4,4 % · moins vite · au-dessus · −40,2 % · oui; ROG.SW
  « indisponible » (no study, no provider), its « Ouvrir l'examen » disabled; SCHN.SW 6 / 6 ·
  0,2 % · 4,6 % · plus vite · voisin · −13,0 % · oui. Watchlist order kept, no sort control.
- « Ouvrir l'examen » on NESN.SW → the full examination « depuis l'étude », « ‹ Retour à la liste
  de suivi », no « Créer l'étude »; Retour → Liste de suivi with the card still shown; « Fermer le
  criblage » hides it; the Études nav lands on the list.
- Walk finding fixed: the column heads wrapped and did not line up with the cells → shared fixed
  column widths.
- Not driven: a real fetched row and a real quota stop (they need Guy's key) — covered by the row
  state tests (`failed_state`, the four row views); the worker's latch itself is inline in the
  worker loop.
- Gates: fmt clean, clippy `-D warnings` clean, `cargo test --workspace` green.
