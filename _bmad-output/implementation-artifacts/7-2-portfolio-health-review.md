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
