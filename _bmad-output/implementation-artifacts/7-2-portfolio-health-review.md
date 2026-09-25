# Story 7.2 — Revue de santé du portefeuille (Portfolio Health Review) + export (FR53)

Status: done

Spec: `_bmad-output/planning-artifacts/story-7-2-portfolio-health-review-spec.md` (PR #224,
validated by Guy 2026-09-24 with the three defaults: a fifth nav destination, a fixed 12-month
cadence, no sector targets).

## Story

As Guy,
I want one read-only roll-up of my whole dossier — how it is spread by size, sector, currency and
bank, how concentrated it is, what state each held position's study is in, and which studies are
due for their annual review — with a PDF of it,
so that a portfolio review is one screen of facts instead of a walk through every bank and study.

## Acceptance Criteria (from the spec §6)

1. **AC1 — One read path.** Every figure is an existing Epic 4/6 read (`journal_diversification`,
   `journal_sector_exposure`, `journal_currency_exposure`, `journal_capital_at_risk_consolidation`)
   or the engine snapshot (`snapshot_for`); the screen shows the SAME figures as Portefeuille and
   the study screens. The position without a study is stated, with the currency cause when a
   same-ticker study exists in another currency; a flagged study's row names its flags.
2. **AC2 — Due for review.** A linked study whose last effective save (the latest FR51 snapshot,
   else creation) is older than 12 months is listed with the date; « en attente » and « confiance
   réduite » studies are listed with their reason; a study saved today is not.
3. **AC3 — Absence honesty.** A missing pair names itself on the figure it blocks; a read failure
   of the holdings/portfolios makes the whole view « indisponible »; one study's read failure marks
   its row « indisponible » only.
4. **AC4 — Export.** « Exporter PDF » renders the same facts in the study PDF's style (greyscale,
   neutral inventory, deterministic bytes, the positions header repeated across page breaks),
   through the native save picker.
5. **AC5 — Posture.** New strings French inside `@tr()` / the report inventory; the nine quality
   flags worded once in `messages.rs` (inventory 122 → 132); `@tr` floor re-based.

## Tasks / Subtasks

- [x] **Task 1 — state** `app/src/state/review.rs`: `portfolio_review(reference, small_max,
  medium_max)` → `PortfolioReviewFacts` (diversification, sectors, currencies, consolidation,
  one `ReviewPosition` per held ticker with `ReviewStudy::{None{other_currency}, Unavailable,
  Linked(facts)}`, the due list, the counts, the deduplicated rates). Tests: the threshold
  helper; the composed read (two positions, one linked, the withheld reason, the other-currency
  cause).
- [x] **Task 2 — report** `report/src/review.rs`: `PortfolioReview` (formatted figures + keys)
  and `render_portfolio_review`; the builder of `pdf.rs` opened `pub(crate)`. Tests: well-formed
  + deterministic + ≥ 2 pages, an empty review, the neutrality / no-wordmark inventory.
- [x] **Task 3 — UI + wiring**: the `Review` global + rows (`state.slint`), `screens/review.slint`
  (cards: en-tête, taille, secteur, devise, banque, concentration, positions, à revoir, en
  chiffres), the fifth destination « Revue » (index 3; Réglages → 4), `wiring/review.rs`
  (`push_review` on activation, the export rail, « Ouvrir l'étude » / « Portefeuille »).
- [x] **Task 4 — gates + headless verification** (below).
- [x] **Task 5 — check** (2026-09-24, headless walk on a copy of Guy's dossier, at Guy's request):
  the eight cards render after the Portefeuille writes (positions 1 304 / 780 / 1 540 CHF, NESN
  « provisoire · Zone d'achat · … · Signaux (5) », the NVDA currency cause, « Aucune étude à
  revoir », the counts); adding a USD → CHF rate leaves NVDA « non classé » because the position
  is in CHF and the study in USD (the #81 rule, not a rate matter). PDF export = the tested
  renderer; the native picker is not driveable headless.

## Dev Notes

- The NAIC « Portfolio Management Guide » (ST-1130) is a per-company meeting tracker needing
  quarterly data; the roll-up is FR53's « Portfolio » export instead (spec §1).
- The quality flags were computed since Story 1.8 but never surfaced; `quality_flag_label` words
  them once (neutral facts: « marge avant impôt en baisse », « PER haut jugé au-dessus de 25 »…).
  They ride the review only; surfacing them on the study screen is a separate decision.
- Sector / currency murmurs reuse the 6.8 band (share ≥ threshold − 5 pp); the « non renseigné »
  sector bucket murmurs like any other — a blind spot is a concentration too.
- Freshness (stale / as-of) is the session map, merged in the wiring; the state read stays pure.
- The per-bank share is the bank's converted total over the global total (the 6.6 read carries
  amounts, not shares).

### Verification (2026-09-24, headless on a copy of Guy's dossier — 3 positions, 2 banks)
- « Revue » renders: en-tête « 2 banque(s) · 3 position(s) · 2 avec une étude »; taille (Grande
  56,6 %, NVDA.US « non classé (taux manquant USD → CHF) » as one band); secteur (« non renseigné »
  56,6 % murmurs); devise; positions with NESN.SW « provisoire · Zone d'achat · 77,1 CHF · H/B
  2,2:1 · valeur relative 124,9 % » and « Signaux (5) : … », NVDA.US « Aucune étude liée :
  l'étude NVDA.US est en USD et la position en CHF »; « Aucune étude à revoir »; the counts.
- Gates: fmt clean, clippy `-D warnings` clean, `cargo test --all` green (see the PR).

### Review Findings — G1 catch-up review (2026-09-25, #237)

3-layer adversarial review of PR #225, checked against main 73a7b19. Decisions are Guy's
(2026-09-25).

- [x] [Review][Decision] The review's own « seuil − 5 » murmur, applied to concentration, sectors and currencies (an all-CHF dossier is always warned), fallback literal 50 — **apply the spec: concentration = `core::risk::concentration_flagged` (as Portefeuille), sectors at/over the threshold, currencies no murmur, fallback through the config default** [app/src/wiring/review.rs:178-182]
- [x] [Review][Decision] PDF: « Études à revoir » does not start page 3; signals printed without their count — **page flow ratified; « Signaux (n) » added** [report/src/review.rs]
- [x] [Review][Patch] PDF prints stale rows or « Aucune donnée. » for a block that is « indisponible » on screen; no unavailable flags in `PortfolioReview` [app/src/wiring/review.rs:206,230,382-480, report/src/review.rs:60,310]
- [x] [Review][Patch] PDF loses the named absences: `global_missing`, `concentration_missing`, per-position missing pairs, the other-currency cause of « aucune étude » [app/src/wiring/review.rs:428-455, report/src/review.rs:26,357]
- [x] [Review][Patch] An unconsolidated bank reads « non classé (étude indisponible) » [app/src/wiring/review.rs:247-257]
- [x] [Review][Patch] A history read failure falls back to the creation date (false « plus de 12 mois ») [app/src/state/review.rs:246-256]
- [x] [Review][Patch] A normalize failure is worded as a read failure; a global total absent without a named pair shows nothing; shares blocked by a missing global pair don't name it; only the first missing pair is named [app/src/state/review.rs:234, app/src/wiring/review.rs:132-158,247-285]
- [x] [Review][Patch] Study line dangles: « prix actuel :  CHF · H/B  · valeur relative : » [app/ui/screens/review.slint:205-207]
- [x] [Review][Patch] Trailing stop and breach taken from `held[0]` only (other banks ignored) [app/src/state/review.rs:202,275-288]
- [x] [Review][Patch] A legacy holding with no currency is matched differently from the register [app/src/state/review.rs:203,219]
- [x] [Review][Patch] Threshold and size targets shown as raw config strings beside formatted shares (locale rule) [app/src/wiring/review.rs:107,130-156,425]
- [x] [Review][Patch] One due reason per study (« age » hides « withheld » / « low confidence ») [app/src/state/review.rs:305]
- [x] [Review][Patch] Other-currency study lookup is absence-blind (#95) [app/src/state/review.rs:223]
- [x] [Review][Patch] Share-row labels keyed by string value (a bank or sector named « small » reads « Petite »); PDF unclassified `MissingRate` rows lose « non classé » [app/ui/screens/review.slint:65-71, report/src/review.rs:270-297]
- [x] [Review][Patch] Export notice never cleared (survives a dossier switch) [app/src/wiring/review.rs:514]
- [x] [Review][Patch] « Ouvrir l'étude » skips `invoke_screen_activated` (#94) [app/src/wiring/review.rs:1158-1163]
- [ ] [Review][Patch] PDF: a position's note line can be split from its row by a page break [report/src/pdf.rs:1166-1184, report/src/review.rs:477]
- [x] [Review][Patch] Zone / verdict logic duplicated from `viewmodel::engine`; dead-code silencers (`let _ = …`) [app/src/state/review.rs:135, app/src/wiring/review.rs:542, report/src/review.rs:519-520]
- [x] [Review][Patch] Tests: the 12-month path, a missing pair, two banks, a row with signals, parity with Portefeuille, header repeat [app/src/state/tests.rs, report/src/review.rs]

Fixed in PR B (G1, branch `fix/g1-b-review`): every item checked above, and decisions 6
(concentration = `concentration_murmur` shared with Portefeuille — with the « present and positive »
guard on both sides; sectors at/over the threshold; currencies never) and 9d (« Signaux (n) »).
Also: stops and study links read per lot as the register does; « non calculable » studies counted
and listed as due (« données non calculables »); an unknown last-save date listed as due
(« ancienneté inconnue »). The PDF note-row page break is in the PDF PR.

### Review Findings — G1 final review, area 2 (2026-09-25, #237, on integ/g1-final e686141)

- [x] [Review][Patch] M1 — A legacy lot (no declared currency): its stop was labelled in the reference currency, and the study price in the position's currency — **the stop carries no currency and is never compared; the row (and the PDF) say why (« non comparé au prix : le lot n'a pas de devise renseignée », worded per lot after G3); the price is labelled with the STUDY's currency**. The register (`wiring/holdings.rs`) computes its own comparison (not shared) and still compares unconditionally — left to the portfolio branch [app/src/state/review.rs, app/src/wiring/review.rs]
- [x] [Review][Patch] M2 — The row's study was `links[0]` (lot position); counts / due read the first lot's study only — **chosen by identity (the newest linked study — the ticker's own; a failed read before « aucune étude »); flag / zone counts and the due list read every lot's study (« NESN (USD) » when the lots link to different studies); the verdict partition follows the row's study** [app/src/state/review.rs]
- [x] [Review][Patch] M3 — The review was not re-pushed when a price / FX / study fetch landed while shown; the PDF exported the pushed rows as they were — **re-pushed on every async arrival while on display; the export re-pushes first** [app/src/wiring/fetch.rs, app/src/wiring/review.rs]
- [x] [Review][Patch] L4 — Amounts at 0 dp (`LargeMonetary`) where Portefeuille shows `Price` (at most 2 dp) — **`Price` (at most two decimals, trailing zeros not padded), as Portefeuille; the PDF columns widened for « 12 345 678,99 CHF »** [app/src/wiring/review.rs, report/src/review.rs]
- [x] [Review][Patch] L5 — An empty dossier read three « indisponible » size classes — **one statement « Aucune position classée : le dossier ne contient aucune position. », screen and PDF**
- [x] [Review][Patch] L6 — PDF share / target columns without « % » (the test data carried it) — **the layout writes the unit; the sample hands the bare figure, as the app does**
- [x] [Review][Patch] L7 — FX footnote rate as the stored « 0.8 » — **through the user's number format** (Portefeuille's footnote is the same and is left to the portfolio branch)
- [x] [Review][Patch] L8 — A dismissed trigger was still shown — **per-lot triggers keyed by holding id; the register's dismissed set honoured**
- [x] [Review][Patch] L9 — A global total absent for pairs AND a non-pair cause named only the pairs — **both named**
- [x] [Review][Patch] L10 — Trigger words differed screen / PDF; the due subtitle named three of five reasons — **« Le prix a atteint le seuil suiveur. » / « Le prix est dans la zone haute. » on both; the subtitle lists all five**

Fixed on branch `fix/g1-l-review`.

#### G3 review of `fix/g1-l-review` (2026-09-25)

- [x] [Review][Patch] M-a — Counts « avec au moins un signal » / « zone haute » read every lot's study while the row shows one — **read the row's shown study only; the due count is worded « {} études à revoir » on its own line (screen and PDF); the mixed-links band names the OTHER studies' signals / high zone**
- [x] [Review][Patch] M-c — One full re-composition per async result (recompute storm) — **coalesced: a latch + one zero-delay single-shot timer per burst (`RepushLatch`, tested)**
- [x] [Review][Patch] L-a — An async re-push cleared « Revue exportée : chemin » — **the notice is cleared on arrival (screen-activated arm) and at the start of an export only**
- [x] [Review][Patch] L-b — Row currency taken from the shown lot's effective currency — **the shown study's own currency; without a study every lot's declared currency, « — » kept for a lot declaring none**
- [x] [Review][Patch] L-c — « aucune étude » and « indisponible » merged into one « — »; the band claimed « la plus récente » over an unreadable lot; an unreadable lot's stop was silently « not breached » — **two facts on the band; « une étude lue, sans pouvoir dire si c'est la plus récente »; the stop reads « non comparé au prix : l'étude du lot n'a pas pu être lue »**
- [x] [Review][Patch] L-d — « la position » → « le lot n'a pas de devise renseignée » (screen + PDF)
- [x] [Review][Patch] L-e — `size_empty` keyed by the review's own positions
- [x] [Review][Patch] L-g — Tests: three lots CHF / USD / CHF with studies created on different days (the newer with the smaller id), an identity tie; the « at most two decimals » comments
- M-b (register side) and L-f (Portefeuille's FX footnote) belong to the portfolio branch — not touched here.
- G1 P (`fix/g1-p-followups`): D5 — a currency-less lot is presumed in the reference currency; its stop is compared against a reference-currency study and, against a study in another currency, « non comparé au prix : le lot n'a pas de devise renseignée et l'étude est en USD » (screen + PDF), through the register's own rule (`state::stop_basis`). The unreadable-study cause is kept.
