# Story 7.1 — Comparaison de sociétés (Company Comparison) — spec for review

**Status:** spec for review (Guy) — no code yet. **Epic 7 outline:** « Company Comparison
(side-by-side ~30 metrics) + faithful export (FR53) ». **Reference:** the NAIC « Stock Comparison
Guide » (ST-1090) in `docs/NAIC/forms/` — one page, up to five companies as columns, thirty
numbered rows in four groups. **Crates:** `app` (viewmodel + UI + wiring) and `report` (the
export); `core` / `contract` / `persistence` untouched — every row is a figure the study screen
already shows or a min/max over figures the engine already returns.

## 1. What it is

Up to **five studies side by side**, one column each, the form's thirty rows in its four groups,
every figure taken from each study's own engine snapshot (`build_frame` — the one construction
path, so a comparison cannot drift from the study screens). It compares; it never ranks (FR13):
no « best », no highlighting of the « winner », no sort by a metric. A cell is a figure or the
named absence (« — », « à saisir », « indisponible »), never a blank.

## 2. Where it lives

On the **Études** list, a card **« Comparer des études »**: five drop-downs (the new `Dropdown`,
the dossier's study tickers, a picked ticker leaves the other lists) and « Comparer ». The table
opens **in place of the list** (the way an open study does — `Studies.compare-open`), with
« ‹ Retour aux études », « Exporter PDF » and « Ouvrir l'étude » under each column. Re-derived
on open and on return from a study (the #94 rule). No persistence of the selection (a
comparison is a moment, not a record — open question Q3).

*G1 review, 2026-09-25:* the drop-downs list the dossier's STUDIES, not its tickers — « TICKER »,
or « TICKER · CUR » when a ticker has several studies; the study id is carried from the pick to
the column and to « Ouvrir l'étude ». A picked study leaves the other lists; « Comparer » needs
two distinct studies.

## 3. The rows (the form's numbering, neutral wording)

Columns: one per study — ticker, company name, currency, decision date.

**Croissance (§1)**
1. Croissance historique des ventes — `growth.sales_cagr_pct`
2. Croissance estimée des ventes — judgment `projected_sales_growth_pct`
3. Croissance historique du BPA — `growth.eps_cagr_pct`
4. Croissance estimée du BPA — judgment `projected_eps_growth_pct`

**Gestion (§2)**
5. Marge avant impôt, moyenne 5 ans + tendance — `management.avg_ptp_pct`, `ptp_trend`
6. Rendement des capitaux propres, moyenne 5 ans + tendance — `avg_roe_pct`, `roe_trend`
7. Part du capital détenue par la direction — **not carried by the app**: « — » (stated absent)

**Cours (§3–§5)**
8. BPA total estimé sur 5 ans — `returns.avg_annual_eps × 5` (the form's « estimated total
   earnings per share for next 5 years »; the five projected yearly EPS are exact integer powers
   already summed in the engine's average)
9. Fourchette de cours sur 5 ans — max high / min low over the §3 window (the `series` years the
   valuation window covers)
10. Cours actuel — judgment `current_price`
11. PER le plus haut — max of `valuation.per_year[].high_pe`
12. PER haut moyen — `valuation.avg_high_pe`
13. PER moyen — `valuation.avg_pe`
14. PER bas moyen — `valuation.avg_low_pe`
15. PER le plus bas — min of `valuation.per_year[].low_pe`
16. PER actuel — `valuation.current_pe`
17. Zone basse — `zones.forecast_low` à `buy_top`
18. Zone médiane — `buy_top` à `neutral_top`
19. Zone haute — `neutral_top` à `forecast_high`
20. Position du cours actuel — the zone in words, « sous la bande » / « au-dessus » outside
21. Ratio hausse / baisse — `upside_downside`
22. Rendement présent — `returns.present_yield_pct`
23. Rendement annuel total estimé — `returns.projected_total_annualized_return_pct` (the
    form's « combined estimated yield »; « (hors div.) » when only the appreciation is known)

**Autres**
24. Actions en circulation — « — » (not carried)
25. Dilution potentielle — « — » (not carried)
26. Taux de distribution moyen — `valuation.avg_payout_pct`
27. Signaux de qualité — the count + the names (the 7.2 wording) — *the form leaves 27–28
    blank; this is the one addition, an honest one*
28. État de l'étude — critères validés / provisoire / en attente, confiance réduite
29. Date des données — the latest provider `provenance.timestamp` of the study's cells, else the
    decision date — *G1 review, 2026-09-25: of its FILLED cells, else « — » (the decision date
    is in the column header, never passed off as the data's date)*
30. Place de cotation — the ticker's suffix (`.SW`, `.US`) as data, else « — » — *G1 review,
    2026-09-25: a KNOWN venue only (`.US` or the pinned venue table), so a share class such as
    `BRK.B` reads « — », never « B »*

Percentages, P/Es and ratios compare across currencies; **prices (9, 10, 17–19) are native** —
each column carries its currency and, when the selected studies differ in currency, a band says
so (FR28: never a conversion here).

## 4. The export (FR53)

« Exporter PDF » → `report::render_comparison(&Comparison)`: **A4 landscape** (five columns need
the width), one page, the four groups as boxed grids with the row numbers, the column headers
repeated on a second page only if a very long name forces one. Native `rfd` save picker,
deterministic bytes, the report's own neutral inventory + test.

## 5. Constraints kept

- Neutral voice; absence honesty; the colour budget (ink only — a comparison table is where a
  « best » colour would be most tempting, and it is exactly what FR13 forbids).
- No engine change: rows 8, 9, 11, 15 are min/max/× over engine outputs, computed in the
  viewmodel from the snapshot (pure, unit-tested); everything else is a straight read.
- Personal scale: five snapshots, well under a second.

## 6. Acceptance criteria

**Given** three studies picked, one in USD and two in CHF
**When** « Comparer »
**Then** three columns render the thirty rows; every figure equals the study screen's; the
currency band states the mix; prices show their own currency.

**Given** a picked study with no judgments
**Then** rows 2, 4, 10, 16–23 read « à saisir » / « — » as the study screen does, never 0.

**Given** a picked study whose read fails
**Then** its column reads « indisponible » in every row; the others stand.

**Given** « Exporter PDF »
**Then** the landscape PDF carries the same thirty rows, neutral inventory, deterministic bytes.

**Given** the posture gates
**Then** every new string is French inside `@tr()` / the report inventory; floors bumped.

## 7. Tasks (one story, one PR — ~2 days)

1. `app/src/viewmodel/comparison.rs` — `ComparisonColumn` from a `StudyFrame` (the thirty
   rows, formatted; the four derived min/max/×); tests.
2. `report/src/comparison.rs` — `Comparison` + `render_comparison` (landscape); tests.
3. `screens/dashboard.slint` — the picker card; `screens/comparison.slint`; the `Comparison`
   global; `wiring/comparison.rs` (open / return / export); posture floors.
4. Headless verify; Guy's on-display check; story record.

## 8. Open questions (defaults proposed)

- **Q1** Picker: five drop-downs, or a « Comparer » toggle on each list row? **Default:
  drop-downs** (explicit, ordered, no hidden selection state on the rows).
- **Q2** Landscape PDF? **Default: yes** — five columns of figures do not fit portrait legibly.
- **Q3** Persist the last comparison in the dossier? **Default: no** — a comparison is a moment;
  the studies are the records (FR51 keeps their history).
