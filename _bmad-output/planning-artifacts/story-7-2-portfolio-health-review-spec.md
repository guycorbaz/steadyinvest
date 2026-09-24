# Story 7.2 — Revue de santé du portefeuille (Portfolio Health Review) — spec for review

**Status:** spec for review (Guy) — no code yet. **Epic 7 outline:** « Portfolio Health Review
(diversification/quality roll-up) + faithful export (FR53) ». **Decision (Guy, 2026-09-24):** 7.2
before 7.1; a short spec first, then the code. **Crates:** `app` (state read + wiring + UI) and
`report` (the export); `core`, `contract`, `persistence` untouched — every figure below is an
EXISTING read (Epics 4 and 6) or the engine's snapshot; nothing new is computed.

## 1. What it is, and what it is not

A **read-only roll-up of the whole dossier** — every bank, every currency — answering in one
place the questions the NAIC discipline asks at a portfolio review: *is the portfolio diversified
the way I decided, is every position's study still sound, and which studies are due for their
annual review?* It states facts; it never ranks, scores or recommends (FR13 — no verdict on the
portfolio, no « santé : bonne »).

It is **not** the NAIC « Portfolio Management Guide » form (ST-1130): that form tracks ONE company
over its meetings (P/E zones per year, cumulative quarterly earnings, a price/P/E chart) — it is a
per-study follow-up sheet, and its P/E-at-meeting rows need quarterly data the app does not carry
(v1). Its zone logic already lives in the study's §4. FR53's « Portfolio » export is therefore
the roll-up below, in the study PDF's neutral style — not a reproduction of ST-1130.

## 2. Where it lives

A fifth destination in the nav rail: **« Revue »** (after Portefeuille). One screen, cards (the
UX pass' `PanelCard` / `StatusBand`), no entry — the only actions are « Exporter PDF » and the
links « Ouvrir l'étude » / « Portefeuille ». Re-rendered on activation (issue #94) so it is never
stale on arrival. Read failures state « indisponible », never an empty section (issue #95).

## 3. The cards, top to bottom

1. **En-tête** — dossier name, the date, the reference currency, the number of banks / positions
   / linked studies, and the FR28 footnote of every rate used (« Convertis aux taux : … »).

2. **Répartition** — the four diversification facts side by side against what Réglages holds:
   - **par taille** (6.7 `journal_diversification`): Petite / Moyenne / Grande shares vs targets,
     the « non classé » rows with their reasons;
   - **par secteur** (6.8 `journal_sector_exposure`): one line per sector, « non renseigné » as its
     own bucket, the murmur at/over the concentration threshold;
   - **par devise** (6.8 `journal_currency_exposure`);
   - **par banque** (6.6 consolidation): per-bank invested and share, the global total.
   Each block keeps the 6.x wording; absent facts name their missing pair.

3. **Concentration** — the per-security shares (6.7 rows), largest first, the threshold murmur
   as a band. (Same read as Portefeuille; here it reads across ALL banks at once.)

4. **Positions et leurs études** — one row per held ticker (aggregated across banks), from the
   register rows (4.4/4.5/4.7) and the study's snapshot (`snapshot_for`):
   - the ticker, the banks holding it, the invested amount and share;
   - the study link: the verdict state in words (« critères validés » / « provisoire » / « en
     attente »), « confiance réduite » when set; **no study** stated (with the currency cause
     when a same-ticker study exists in another currency);
   - the present price's zone (words), the U/D ratio, the relative value;
   - the **quality flags** as named facts (the nine `QualityFlagKey`s in the app's neutral
     wording: « marge avant impôt en baisse », « ROE en baisse », « ROE < 10 % », « BPA en
     retard sur les ventes », « PER haut jugé > 20 / > 25 », « H/B < 3 / > 20 »,
     « valeur relative ≥ 100 % ») — count + the list on the row;
   - the data state: « périmé » / « à jour le … » (the refresh freshness), the trailing stop and
     whether it is breached, an open trigger.
   Ordered by invested share, largest first. Sold positions excluded.

5. **Études à revoir** — the annual-review journey (Story 3.6) made visible: every linked study
   whose last effective save (the FR51 history, `try_list_study_history`) is older than **12
   months**, with the date; plus every linked study with « en attente » or « confiance réduite ».
   A calm « aucune » when empty. (12 months is the method's review cadence — not a Réglages
   knob in this story; open question Q2.)

6. **Synthèse en chiffres** (counts only, no judgement): positions · with a study · full verdict ·
   provisional · withheld · with ≥ 1 flag · stale · in the high zone · stop breached · due for
   review. A number is a fact; the section carries no colour.

## 4. The export (FR53)

« Exporter PDF » → `report::render_portfolio_review(&PortfolioReview)`: the `app` builds a plain
`PortfolioReview` value (defined in `report`, contract-free: strings + decimals already formatted
by the same `app` formatters, the rates footnote), `report` lays it out in the study PDF's style
(Helvetica, greyscale, cards as boxed grids, the neutral inventory + its neutrality test). Pages:
(1) en-tête + répartition + concentration; (2) the positions table (repeating header across
breaks, issue #74); (3) études à revoir + synthèse. Native `rfd` save picker (the 5.6 rail),
deterministic bytes.

## 5. Constraints kept

- Neutral voice / banned verbs; absence honesty (« indisponible » vs « aucune »); the colour
  budget (no hue outside a study's §4 — the review is ink only, glyphs and words); the F4
  notice rule (its one notice slot: the export outcome).
- No new persistence, no contract change, no engine change; `app` state gains ONE read,
  `portfolio_review(reference_currency) -> Result<PortfolioReviewFacts, String>`, composed from
  the existing reads (memoized rates, one `snapshot_for` per linked study).
- Personal scale: a dossier of ~50 positions renders in well under a second; no background work.

## 6. Acceptance criteria

**Given** a dossier with two banks, a foreign-currency position, a position without a study and
a study with two quality flags
**When** « Revue » is opened
**Then** the four répartition blocks, the concentration rows, the positions table and the counts
show the SAME figures as Portefeuille and the study screens (one read path, no drift)
**And** the position without a study is stated as such, with the currency cause when applicable
**And** the flagged study's row names both flags.

**Given** a study whose last effective save is older than 12 months
**When** the review renders
**Then** it is listed under « Études à revoir » with that date; a study saved yesterday is not.

**Given** a missing FX pair
**When** the review renders
**Then** the affected figures say « taux manquant EUR → CHF » (never a partial total), the others
stay.

**Given** « Exporter PDF »
**Then** the PDF carries the same facts, greyscale, neutral inventory, deterministic bytes; the
positions header repeats across a page break.

**Given** the posture gates
**Then** every new string is French inside `@tr()` / the report inventory, no banned verb; the
`@tr` floor and the message inventory count are bumped deliberately.

## 7. Tasks (one story, one PR — ~3 days)

1. `app/src/state/review.rs` — `portfolio_review(...)`: compose the reads; tests (state, headless).
2. `report/src/review.rs` — `PortfolioReview` + `render_portfolio_review`; tests (well-formed,
   deterministic, neutral, header repeat).
3. `app/ui/screens/review.slint` + the `Review` global + `wiring/review.rs` (push on activation,
   export via the rfd rail); nav rail entry; posture floors.
4. Headless verify (the walk's lessons: resize + X focus); Guy's on-display check; story record.

## 8. Open questions (defaults proposed)

- **Q1** A fifth nav destination, or a card at the top of Portefeuille? **Default: a
  destination** — the review reads across banks while Portefeuille is per bank, and the screen is
  long already.
- **Q2** The 12-month review cadence: fixed, or a Réglages field? **Default: fixed** (the
  method's annual review; a knob can come with 7.3's screening).
- **Q3** Sector targets do not exist (only the concentration threshold applies to sectors).
  Add a sector table to Réglages now? **Default: no** — facts only; targets are a later decision.
