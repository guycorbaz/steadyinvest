# Story 7.3 — Examen rapide (Quick Screen / Stock Check List) + criblage de la liste de suivi — spec for review

**Status:** spec for review (Guy) — no code yet. **Epic 7 outline:** « Discovery/screening +
Quick Screen / Starter Checklist ». **Reference:** the NAIC « Stock Check List for Beginning
Investors » (1996) in `docs/NAIC/forms/stock checklist.pdf` — two pages, four sections: past
sales record, past earnings-per-share record, price record of the stock, conclusion. **Crates:**
`core` (one pure module, the form's arithmetic), `app` (state + viewmodel + UI + wiring),
`report` (the export); `contract` / `persistence` / `ingestion` untouched — a quick screen reads
the same fetched or saved financials a study does, and writes nothing.

## 1. What it is, and what it is not

The form the NAIC hands a beginner **before** the Stock Selection Guide: six years of sales and
EPS reduced to two compound growth rates, five years of prices reduced to an average P/E, and
four conclusions the reader writes in against **their own objective**. In steadyinvest it is the
**« Examen rapide »**: one screen, one ticker, the form's figures computed from the data the
app already fetches for a study, the four conclusions stated as facts against an objective the
user types on the screen (never a default the app chooses). It is a **look before a study**:
its last action is « Créer l'étude » from the same fetched data, so nothing is fetched twice.

It **states, never recommends** (FR13): « le taux de croissance des ventes (8,2 %) atteint
l'objectif (7 %) » is a comparison the user set up; « acceptable / trop élevé » for the price is
worded as the form's fact — the present P/E above / around / below the five-year average.

The **criblage** (screening) half is the same examination run over the **liste de suivi**: one
row per watched ticker, the four figures side by side, in the watchlist's order, no ranking, no
sort. It answers « which of the titles I watch deserve a study next? » with facts, and leaves the
choice to the reader. Market-wide screening (a provider's screener endpoint, thousands of
tickers) is **out of scope**: it is provider-locked, quota-hungry, and the PRD keeps it roadmap.

## 2. Where it lives

- **Études**, a card **« Examiner un titre »**: ticker (text) + currency (`Dropdown`, the
  configured currencies) + « Examiner » → fetch through the configured provider chain (the 6.9
  path, `fetch_canonical`) → the examination opens in place of the list (`Studies.screen-open`,
  the 7.1 pattern), with « ‹ Retour aux études », « Exporter PDF », « Créer l'étude ».
- **On a study** (the study screen's action row): « Examen rapide » — the same screen from the
  study's own saved years, no fetch, « Créer l'étude » absent (it exists).
- **Liste de suivi**, a button **« Examiner la liste »**: one fetch per watched ticker without a
  study (rate-limited by the 6.9 batching), the saved years for those with one, then a table
  card « Criblage » under the list. Quota / failure per row → « indisponible » on that row only.

Nothing is persisted: an examination is a moment (the 7.1 Q3 rule); the study is the record.

## 3. The figures (the form's numbering)

Let `Y` be the ascending usable years (the engine's canonical series; a year lacking sales or
EPS is skipped for the section that needs it, and the screen says which).

**1 · Ventes passées** — (1) sales of the most recent year, (2) the year before, (3) their sum,
(4) ÷ 2; (5) sales five years ago, (6) six years ago, (7) their sum, (8) ÷ 2; (9) = (4) − (8);
(10) = (9) ÷ (8) in %; then **the compound annual rate**: the form's conversion table is
`(1 + r)^5 = 1 + (10)` (27 % ↔ 5 %, 271 % ↔ 30 %), so the rate is
`endpoints_cagr_pct((8), (4), 5)` — the engine's existing exact-root helper, the form's five
years kept as-is (faithful, and stated on the screen: « base : deux moyennes de deux ans, à
cinq ans d'écart, comme le formulaire »).

**2 · BPA passés** — the same ten lines on EPS, the compound rate, then the form's sentence:
« le BPA a augmenté **plus / moins** vite que les ventes » (a comparison of the two rates; equal
→ « au même rythme »). The form's free lines (« reasons for the difference », « will the factors
continue ») are **one free-text field**, kept on the screen and the PDF, never stored.

**3 · Cours** — present price (the latest fetched close or the study's judged price) and present
EPS (TTM if fetched, else the latest year); a five-row table: year, high (A), low (B), EPS (C),
P/E at high (A ÷ C), P/E at low (B ÷ C); totals and averages of the two P/E columns; « average
of the high and low P/E averages ». Then the facts the form asks for:
- present price **higher / lower** than the high of five years ago, and by what %;
- « this stock has sold as high as the current price in N of the last 5 years »;
- present P/E vs the average of averages: « supérieur / voisin (± 10 %) / inférieur ».
The « unusually high / low P/E » line and the « adjust the average to … » lines are the reader's
judgment: two free fields, not computed.

**4 · Conclusion** — the reader's objective (one field, « objectif de croissance annuelle, % »;
empty → the four lines read « objectif non renseigné »), then:
1. the past sales growth rate **atteint / n'atteint pas** the objective;
2. the past EPS growth rate atteint / n'atteint pas;
3. « possible EPS growth in the coming five years » — the form asks for the reader's view; the
   screen carries the reader's yes / no as a field, unfilled by default;
4. the price: the §3 P/E fact repeated in the form's words (« dans la norme des cinq ans » /
   « au-dessus de la norme »), never « acceptable » or « trop élevé » decided by the app.

The form's footer sentence is kept in French on the screen and the PDF: « ce formulaire n'est
pas une analyse suffisante ; il aide à poser les questions avant une étude complète ».

## 4. The criblage table (liste de suivi)

One row per watched ticker: ticker · years used (« 6 / 6 », « 4 / 6 ») · sales rate · EPS rate ·
EPS vs sales (plus / moins / même) · present P/E vs five-year average · price vs five-year high ·
a study exists (oui / non). « Ouvrir l'examen » per row opens the full screen. The objective is
NOT applied here (a table of pass / fail is a ranking in disguise); the rates are the facts.

## 5. The export (FR53)

« Exporter PDF » → `report::render_quick_screen(&QuickScreen)`: **A4 portrait**, the form's two
pages in the study PDF's style — the four numbered sections, the ten-line ladders, the five-row
price table with totals and averages, the conclusion lines with the reader's fields, the footer
sentence. Deterministic bytes, greyscale, neutral inventory + test, no wordmark. The criblage
table has no PDF in this story (its rows are the examinations; each exports on its own).

## 6. Constraints kept

- **Neutral voice**: no « acheter », no « bon / mauvais », no colour outside the §4 zone words
  (the screen has none); the objective and the yes / no are the reader's, echoed as typed.
- **Absence honesty**: fewer than six usable years → the ladder names the years it used and the
  rate is computed on the span it has (`endpoints_cagr_pct` with the real span) with the caption
  « sur N ans (formulaire : 5) »; fewer than three → « indisponible ». A missing price → the §3
  facts read « — ».
- **One arithmetic**: `core::checklist::quick_screen(&[CanonicalYear], present_price, present_eps)
  -> QuickScreenOutputs` — pure, `Decimal`, exact roots through the existing growth helper, unit
  tested against the form's conversion table (27 % → 5,0 %, 271 % → 30,0 %).
- **Data licensing**: nothing new stored; a fetched examination lives in the session only.
- **Quota**: the watchlist run reuses the 6.9 batching and stops at the first quota reply,
  marking the remaining rows « non examiné (limite d'usage) ».

## 7. Acceptance criteria

**Given** a ticker with six usable years fetched
**When** « Examiner »
**Then** §1 and §2 show the ten lines and the two rates; the rates equal `endpoints_cagr_pct` of
the two-year averages over five years; §3 shows five rows, the averages, the three facts.

**Given** an objective of 7 % typed, sales rate 8,2 %, EPS rate 5,1 %
**Then** conclusion 1 reads « atteint », 2 « n'atteint pas », 3 stays the reader's field, 4 the
P/E fact; no other word.

**Given** a study opened → « Examen rapide »
**Then** the same figures come from the study's saved years without a fetch; « Créer l'étude » is
absent; « Retour » lands on the study.

**Given** « Examiner la liste » with three watched tickers, one already studied, one whose fetch
hits the quota
**Then** the table has three rows: the studied one from its years, the fetched one, the third
« non examiné (limite d'usage) »; no sort control; no pass / fail column.

**Given** « Exporter PDF »
**Then** a two-page portrait PDF with the four sections, deterministic, neutral, no wordmark.

**Given** the posture gates
**Then** every new string French inside `@tr()` / the report inventory; floors bumped.

## 8. Tasks (one story, two PRs — ~3 days)

**PR 1 — the examination**
1. `core/src/checklist.rs` — `quick_screen(...)` + `QuickScreenOutputs` (the ladders, the two
   rates, the P/E table, the three §3 facts); tests incl. the conversion table.
2. `app/src/viewmodel/quick_screen.rs` — formatting boundary; `app/src/state/quick_screen.rs` —
   from a study's years or a `FetchedFinancials`; `screens/quick_screen.slint` + the `QuickScreen`
   global (fields: objective, free texts, yes / no); « Examiner un titre » card; the study
   screen's « Examen rapide »; `wiring/quick_screen.rs` (fetch through the chain, « Créer
   l'étude » handing the fetched data to the existing study-create path).
3. `report/src/quick_screen.rs` — `render_quick_screen`; tests.
4. Posture floors; headless verify; story record.

**PR 2 — the criblage**
5. « Examiner la liste » on Liste de suivi: the batched run, the « Criblage » card, per-row
   « Ouvrir l'examen »; quota stop; tests on the row states.

## 9. Open questions (defaults proposed)

- **Q1** The objective: typed on the screen each time, or a setting (« objectif de croissance »)
  in Réglages? **Default: typed on the screen, remembered for the session** — an objective is a
  judgment of the moment, and a stored one would look like the app's.
- **Q2** The five-year exponent when the ladder's averages are 1,5 and 5,5 years apart (four
  years, strictly): the form's table (five years) or the exact span? **Default: the form's five
  years**, stated on the screen — fidelity to the form a beginner learns on (NFR-U3).
- **Q3** The criblage in this story (PR 2) or its own story 7.3b? **Default: PR 2 of this story**
  — same engine, same screen per row; only the batch and the table are new.
- **Q4** Story 7.5 (« PDF/print of the other forms », FR53): the comparison and the review ship
  with their PDFs (7.1, 7.2) and the checklist ships with its own here. **Proposal: close 7.5 as
  covered** once this story lands, unless another form is wanted (the Portfolio Management Guide
  was set aside in the 7.2 spec for lack of quarterly data).
