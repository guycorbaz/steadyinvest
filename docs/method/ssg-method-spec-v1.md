# steadyinvest — SSG Method Specification (v1)

**`method_version`: `ssg-1.2.0`**
**Status:** authoritative oracle for the calculation engine (`steadyinvest-core`) and its golden tests.
**Independent project — not affiliated with NAIC / BetterInvesting.** This document specifies the
*method* (formulas, ratios, thresholds — which are not protectable). It uses **neutral labels** and
does **not** reproduce NAIC marks, logos, verbatim instructional prose, or the copyrighted form
layout. Source grounding: `docs/NAIC/Stock Selection Guide Tutorial.pdf` (cited as *Tutorial pNN*)
and the SSG Handbook.

> **How this spec is binding.** Every numeric threshold and formula here is mirrored by a typed
> constant in `steadyinvest-core` (`core::method`, `core::rounding`, `core::quality_flags`,
> `core::method_version`). Changing any of those constants **requires** bumping `METHOD_VERSION`;
> a change-detection test enforces it. By the Foundational Invariant, a `method_version` change
> re-addresses (invalidates) every derived verdict — never a silent change.

All calculations run in the **security's native currency** (FX only at the portfolio layer, out of
scope here). All money/ratio math is **exact decimal** (`rust_decimal`), never `f32`/`f64`.

**Version history**

| `method_version` | Change |
|---|---|
| `ssg-1.0.0` | Initial normative spec. |
| `ssg-1.1.0` | **Additive, zero behavioral change.** Absorbed as normative text the interpretations recorded while implementing Stories 1.7–1.9 (issues #12, #13, #15) — every rule marked *(absorbed at ssg-1.1.0)* below was already the engine's behavior under `ssg-1.0.0`. |
| `ssg-1.2.0` | **Additive: the historical inputs are defined (§0).** A year is the company's fiscal year; its high/low prices are the fiscal year's (not the calendar year's); its EPS is the reported diluted EPS (never an adjusted, non-GAAP one). The engine's formulas, thresholds and scales are unchanged; the provider mapping follows §0 from this version (found on a real NVDA.US fetch, 2026-09-26: calendar-year prices beside January-fiscal-year EPS, and EODHD's non-GAAP `epsActual`), so a study fetched under `ssg-1.2.0` can show other figures than the same study fetched before. A stored study keeps its figures until the user fetches again. |

---

## 0. Historical inputs — what a year's figures are *(added at ssg-1.2.0)*

The engine takes the yearly figures as given; this section fixes what they must BE, so that a
provider mapping (or a manual entry) feeds the method the figures the method is defined on.
[Tutorial p6, p13–14; SSG Handbook]

- **A year is the company's fiscal year**, labelled by the calendar year in which it ENDS (a fiscal
  year ended 28 January 2024 is « 2024 »), with one exception: a 52/53-week year that ends in the
  **first seven days of January** is labelled by the PREVIOUS year (a « Saturday nearest
  31 December » year ended 2 January 2021 is « 2020 », as the company names it). Without the
  exception such a company would show two ends in one calendar year and lose a fiscal year, and
  the gap between two labels would no longer be the gap between the years (§1 growth). *(Owner's
  decision, G3 review of ssg-1.2.0, 2026-09-26.)* Every yearly figure of that label — `sales`, `eps`,
  `high_price`, `low_price`, dividend, pre-tax profit, book value — covers that same fiscal year.
- **`high_price` / `low_price`** are the highest and lowest daily prices of that fiscal year: from
  the day after the previous fiscal-year end through the fiscal-year end, in today's shares (every
  price before a split restated by that split). Not the calendar year's: for a company whose year
  does not end in December, a calendar-year high/low beside the fiscal-year EPS skews the high
  and low P/E of §3.
- **`eps`** is the **reported diluted EPS** of the fiscal year (as published under GAAP / IFRS: net
  income attributable to the common shares ÷ the diluted weighted-average share count), restated
  into today's shares — never an « adjusted », « operating » or other non-GAAP figure. When the
  reported figure is not available the year's EPS is **absent**, never replaced by an adjusted one.
- The **fiscal year in progress** (not yet reported) is not a history year: it has no annual
  statements, and a study's history holds complete fiscal years only.
- A fiscal year that is **not 12 months long** (a stub or transition year after a change of year
  end) keeps its real period: its prices cover that period, never a period cut or padded to
  12 months, and its length is reported so that the §3 `fiscal_period_misalignment` flag names it.

Provider mappings *(ssg-1.2.0)*:

- **EODHD — prices.** The daily bars are reduced into the fiscal years of the **income statement's**
  yearly dates (the statement of the year's sales and EPS; a date served only by the balance sheet
  or the cash flow is no fiscal-year end). Between two reported ends more than 18 months apart
  (a missing statement), one-year periods are filled in; a shorter gap is one reported period.
  A response without any yearly income statement falls back to calendar years: there is no fiscal
  calendar to follow, and such a response has no sales, so its rows never enter a study.
- **EODHD — EPS.** The reported diluted EPS is computed from one fiscal year's statements of the
  SAME date: `netIncomeApplicableToCommonShares` ÷ the balance sheet's
  `commonStockSharesOutstanding`, because EODHD's `Earnings.Annual.epsActual` is its non-GAAP EPS.
  When the applicable-to-common figure is not served, `netIncome` stands for it unless a non-zero
  `preferredStockAndOtherAdjustments` is reported (then the EPS is absent). An ABSENT adjustment is
  read as none: an exception to « absent, never zero » accepted by the owner (G3, 2026-09-26),
  to be reviewed once a real fetch shows how often EODHD serves neither figure.
- **EODHD — open points** *(owner's decisions, G3, 2026-09-26; to be settled on a real fetch)*:
  - `commonStockSharesOutstanding` is the diluted weighted-average count according to EODHD's
    glossary only; that it is not the period-end count is not yet verified.
  - The trailing-twelve-months EPS of the current P/E (§1 relative value) is EODHD's
    `Highlights.EarningsShare`; whether it is the reported or an adjusted figure is not yet
    verified. If adjusted, the current P/E would be set against a history of reported EPS.
- **Twelve Data** serves no statement and no fiscal calendar: its price-only years are calendar
  years and never enter a study's history.

## 1. SSG output set (FR4)

The engine consumes a study's historical inputs (≥ up-to-10 years) + judgment inputs and produces,
deterministically, the following output set, organised by the five method sections.

### §1 — Growth (Visual Analysis)
Inputs: yearly `sales`, `eps`, `high_price`, `low_price` (historical); recent quarter + year-ago
quarter sales/EPS (optional). Outputs:
- **Historical sales CAGR** and **historical EPS CAGR** over the available history (compound annual
  growth rate). *Trend-line estimation is a user judgment in the UI; the engine computes the CAGR of
  a given line / of the endpoints per the chosen method.* [Tutorial p9–10]
- **Recent quarterly % change** (sales, EPS): `(latest − year_ago) / year_ago × 100`. [Tutorial p6]
- **Projected (judgment) future sales/EPS growth %** and the resulting **estimated high EPS** and
  **estimated low EPS** for the forecast horizon (default 5 years). EPS may be projected directly or
  via the revenue projection ("preferred procedure"). [Tutorial p10–11, p17–18]

Normative details *(absorbed at ssg-1.1.0)*:
- **Endpoints CAGR:** `n` = the calendar-year span between the **first and last usable years**
  (`last.year − first.year`); gaps in the reported series compound across. `n = 0`, `start ≤ 0`,
  `end ≤ 0` (zero is neither sign) or a sign-crossing ⇒ the CAGR is **unknown** — the §9 guard runs
  **before** any fractional power is taken.
- **Estimated high EPS:** a direct `estimated_high_eps` judgment wins; otherwise derived as
  `latest_usable_eps × (1 + g_eps/100)^horizon` (exact integer power), the base being the EPS of the
  **most recent usable year**.
- **Estimated low EPS is direct-only:** a low-EPS judgment is not a growth projection; absent ⇒
  **unknown**.
  *Recorded guidance (2026-09-24, from use — non-normative, the engine is unchanged):* the
  estimated low EPS is the **downside case** over the horizon — the latest reported EPS, or lower
  (a recession year), **never a point on the growth trend**. It multiplies the judged average low
  P/E to give forecast-low option (a), so an over-optimistic low EPS lifts the forecast low above
  the present price and the study reads « below the band » although the data are right (seen on a
  fast-growing security: trend-low 18 vs latest EPS 4.09 → forecast low 619 vs price 225). The UI
  proposes the latest EPS for this field and says so under it.
- **Projection with growth < −100 %/yr ⇒ unknown** (the growth factor turns negative; no power is
  taken on a negative base).
- **Quarterly % change with a year-ago value ≤ 0 or absent ⇒ unknown** (a non-positive base has no
  meaningful percent change).
- **Quarterly observations bypass normalization:** they are caller-supplied in post-split,
  native-currency terms.

### §2 — Management
For each year and as a **5-year average** + **trend** (up / even / down):
- **% pre-tax profit on sales (PTP)** = `pre_tax_profit / sales × 100`. If only after-tax net profit
  and tax rate are available: `pre_tax_profit = net_profit / (1 − tax_rate)`. [Tutorial p11–12]
- **% earned on equity (ROE)** = `eps / book_value_per_share × 100`. [Tutorial p12]
- 5-year average = arithmetic mean of the last 5 usable years. Trend = comparison of recent years to
  the 5-year average (see quality flags). [Tutorial p12]

Normative details *(absorbed at ssg-1.1.0)*:
- **Trend reading:** "recent years vs the average" = the **latest usable year's** ratio compared to
  the 5-year average, with the ±0.5 pp even-band **inclusive** (`≤` ⇒ even).
- **Averaging window:** the last 5 **usable** years; within the window, a year whose ratio is
  `unknown` is excluded from the mean (mean over the remaining years).
- **PTP with `sales ≤ 0`:** raises `negative_or_zero_denominator` — the §3 key extends to a
  present-but-non-positive **sales** denominator.

### §3 — Price–Earnings history (last 5 years)
Per year, then averaged over 5 years:
- **High P/E** = `high_price / eps`; **Low P/E** = `low_price / eps`. [Tutorial p13–14]
- **% payout** = `dividend_per_share / eps × 100`. [Tutorial p14]
- **% high yield** = `dividend_per_share / low_price × 100`. [Tutorial p14]
- **Average high P/E** = mean of 5 yearly high P/Es; **Average low P/E** = mean of 5 yearly low P/Es;
  **Average P/E** = `(avg_high_pe + avg_low_pe) / 2`. [Tutorial p15]
- **Average % payout** = mean of 5 yearly payouts. **Average low price** = mean of 5 yearly low prices.
- **Current P/E** = `current_price / (Σ last 4 quarterly EPS)`. [Tutorial p15–16]
- **Relative value** = `current_pe / average_pe × 100` (ideal < 100%). [Tutorial p16]

Normative details *(absorbed at ssg-1.1.0)*:
- **Current P/E with TTM EPS ≤ 0** also raises `negative_or_zero_denominator` (context `ttm_eps`,
  study-level), mirroring the per-year EPS-denominator rule (see also §9).
- **Average low price carries no exclusion rule:** mean of the window's low prices **as reported**.

### §4 — Risk & reward (zoning)
- **Forecast high price** = `avg_high_pe(judged) × estimated_high_eps`. [Tutorial p17]
- **Forecast low price** = the user-selected option among:
  (a) `avg_low_pe(judged) × estimated_low_eps`; (b) average low price of last 5 years;
  (c) a recent severe market low; (d) **price the dividend will support** =
  `present_dividend / (high_yield/100)`. **Constraint: forecast low ≤ current price.** [Tutorial p18–19]
- **Range** = `forecast_high − forecast_low`; **third** = `range / 3`. Zones (default thirds):
  - **Buy** = `[forecast_low, forecast_low + third]`
  - **Neutral/Hold** = `(forecast_low + third, forecast_low + 2·third]`
  - **Sell** = `(forecast_low + 2·third, forecast_high]`
  [Tutorial p19]
- **Present-price zone** = which of Buy/Neutral/Sell the `current_price` falls in.
- **Upside/downside ratio (U/D)** = `(forecast_high − current_price) / (current_price − forecast_low)`.
  [Tutorial p20]

Normative details *(absorbed at ssg-1.1.0)*:
- **Option (d) divisor:** the §3 **5-year average high yield** (`avg_high_yield_pct`); option (d) is
  **not selectable** when that average is ≤ 0 **or unknown**.
- **Present price outside `[forecast_low, forecast_high]` ⇒ zone unknown** (zones are defined only
  over the range). A price above the high side raises **no** finding — the §4 constraint binds only
  the low side.
- **Degenerate range (`forecast_high ≤ forecast_low`) ⇒ zones and U/D unknown**, never inverted
  bands. A range so small the thirds collapse at `Decimal`'s 28-digit precision limit is likewise
  treated as degenerate (strict zone ordering by construction, NFR-C3).

### §5 — 5-year potential
- **Present yield** = `present_full_year_dividend / current_price × 100`. [Tutorial p21]
- **Average annual EPS (next 5 yrs)** = **mean of the projected yearly EPS** — the normative
  deterministic pick *(absorbed at ssg-1.1.0; the tutorial's middle-year alternative is NOT used)*.
- **Average annual dividend** = `avg_annual_eps × avg_payout_pct/100`. [Tutorial p21]
- **Average yield** = `average_annual_dividend / current_price × 100`. [Tutorial p21]
- **Projected price appreciation %** = `(forecast_high − current_price) / current_price × 100`.
- **Projected total annualised return %** = annualised appreciation (current_price → forecast_high over
  the horizon) **plus** average yield. (Study return projection uses the **gross** dividend.)

Normative details *(absorbed at ssg-1.1.0)*:
- **`current_price` absent or ≤ 0 ⇒ every §5 output is unknown** (each §5 formula divides by, or is
  relative to, the current price).
- **The total requires both terms:** the annualised appreciation AND the average yield must both be
  known; either `unknown` ⇒ the total is `unknown` — never a silent 0-yield assumption. (Display
  layers may separately surface the appreciation-only figure, honestly marked; that is presentation,
  not method.)

### Verdict (derived, neutral — see FR13)
A **fact-only** verdict states the present-price zone and the supporting figures. A study is a
"quality-and-value" candidate when ALL hold (these are *facts surfaced*, never a recommendation):
- U/D ratio ≥ **3.0** [Tutorial p20]; **and** relative value < **100%**; **and** present price in the
  **Buy** zone; **and** projected appreciation implies roughly doubling over 5 years (≈ 15%/yr) —
  normative comparator: projected appreciation **≥ 100 %**, inclusive, consistent with the U/D
  `≥ 3.0` criterion *(absorbed at ssg-1.1.0)*. [Tutorial p20]
The verdict is **degraded/withheld** when a load-bearing input is unvalidated or the study is
low-confidence (FR12 — see §5/§6 below).

---

## 2. Quality-flag thresholds (FR7)

Quality flags are **methodology** signals (distinct from plausibility warnings). Each:
`(metric, comparator, threshold, severity)`. v1 set:

| Key | Rule | Severity |
|-----|------|----------|
| `ptp_trend_declining` | 5-yr PTP trend is **down** (recent < earlier beyond the even-band) | warn |
| `roe_trend_declining` | 5-yr ROE trend is **down** | warn |
| `roe_low` | latest ROE < **10%** | info |
| `eps_lags_sales` | EPS CAGR < sales CAGR (margin compression) | info |
| `high_debt` | total debt is sizeable vs the firm's own history (flag for review; no hard ratio in v1). **Not raisable in v1** *(absorbed at ssg-1.1.0)*: the canonical financials carry no debt field, so the engine has no input to compare — the key stays pinned in the catalog (a typed-key test asserts it is the only unraisable entry); adding a debt input is a future contract + normalize + engine change. | info |
| `projected_high_pe_aggressive` | judged future **high P/E > 20** | warn |
| `projected_high_pe_implausible` | judged future **high P/E > 25** → re-evaluate | warn |
| `ud_below_target` | U/D ratio < **3.0** | info |
| `ud_extreme` | U/D ratio > **15.0** (reconsider high/low) | warn |
| `relative_value_high` | relative value ≥ **100%** (current P/E ≥ average) | info |

**Trend "even" band:** a 5-yr trend is *even* when recent years vary by ≤ **0.5 percentage points**
from the 5-yr average; beyond that it is up/down. [Tutorial p12 "vary by just a few tenths"]

---

## 3. Plausibility rules (FR10)

Plausibility issues are **input-data** warnings (distinct from quality flags and from the review tag):

| Key | Detection rule |
|-----|----------------|
| `split_series_break` | year-over-year EPS or price jumps by a factor ≥ **1.5** or ≤ **0.67** inconsistent with sales (candidate unadjusted split / series break) |
| `currency_mismatch` | a cell's currency ≠ the study's native currency |
| `fiscal_period_misalignment` | reported period length ≠ ~12 months, or fiscal-year-end shift between consecutive years |
| `out_of_bounds_ratio` | computed PTP or ROE outside **[−100%, +100%]**, or P/E outside **[0, 200]** (the chart axis bound) |
| `negative_or_zero_denominator` | EPS ≤ 0 used as a P/E denominator, or book value ≤ 0 for ROE → mark `unknown/insufficient`, never coerce to 0 |
| `low_price_above_current` | a selected forecast low price > current price (violates the §4 constraint) |

Plausibility warnings never block computation; they surface at the cell.

**`split_series_break` — normative detection rule** *(absorbed at ssg-1.1.0; quantifies
"inconsistent with sales")*:

- Detection runs on the **post-adjustment** series (declared splits already applied): a correctly
  declared split never trips the detector.
- Per field (`eps`, `high_price`, `low_price`), year `t` is flagged when its y/y factor is
  **≥ 1.5 or ≤ 0.67 — boundary values included** (normative comparators, §7), **and** the sales y/y
  factor does **not** move beyond the same band **in the same direction**: an up-jump is "explained"
  only if the sales factor is also ≥ 1.5; a down-jump only if sales is also ≤ 0.67. **Absent** sales
  (either year missing, or prior-year sales = 0 ⇒ no factor) or an **opposite/in-band** sales move ⇒
  still flagged.
- A `None`/zero **prior-year** value yields **no factor and no flag** (absence of evidence is not
  evidence — and no division panic).
- Only **calendar-consecutive** years compare (`t = t−1 + 1`); a gap in the reported series is not
  "year-over-year".
- **Flag only** — values pass through unchanged; the system never auto-corrects.
- The low bound is **pinned at `0.67`**, deliberately not the non-terminating exact reciprocal `2/3`
  (marginally more sensitive on the down side; kept as the normative constant).
- A **sign-crossing** factor (e.g. EPS −1.00 → +1.10, factor −1.1 ≤ 0.67) satisfies the rule and is
  flagged: a sign-crossing is a notable series event worth surfacing even when no split is suspected
  (flag-only, never blocking).

---

## 4. "Usable year" & low-confidence rule (FR8)

- A year is **usable** iff all **load-bearing fields** are present and valid for that year:
  `sales`, `eps`, `high_price`, `low_price`. (Appendix A.)
- A study is **low-confidence** when **usable years < 5**. The engine still computes on available data
  and carries a queryable `low_confidence` state into the verdict (never a hard block). [PRD FR8]

## 5. "Load-bearing input" definition (FR12)

The verdict is **degraded or withheld** when any load-bearing input is missing, not validated (review
≠ ✓), or stale. Load-bearing inputs for the verdict:
- the per-year `sales`, `eps`, `high_price`, `low_price` of the usable years;
- the judgment inputs that determine the zones: `estimated_high_eps`, `estimated_low_eps`,
  `judged_avg_high_pe`, `judged_avg_low_pe` (or the selected forecast-low option), and `current_price`.

The `FullVerdict` type (Story 1.11) is constructible **only** when every load-bearing input is `✓`
and not stale.

## 6. Banned-verb list (FR13) — posture gate

Scope: **system-generated** signals/labels/alerts/microcopy (NOT user free-text notes/rationale).
No system signal may contain an imperative action/recommendation verb. v1 banned set (case-insensitive,
whole-word, plus the French equivalents used in the UI):

`buy, sell, hold, purchase, acquire, dump, exit, enter, trade, invest, divest, recommend, suggest,
should, must, "ought to"` · French: `acheter, vendre, conserver, garder, acquérir, investir,
recommander, suggérer, devrait, "il faut"`

Allowed neutral framing states facts: e.g. "the present price is in the **Buy zone** you defined"
(zone is a **label**, not a command). Note: zone *labels* Buy/Neutral/Sell are nouns naming the
defined price bands and are permitted; the ban targets **imperative verbs** directed at the user.
*(This is a posture gate verified by a targeted test over system strings — not a blanket grep over all
text.)*

The zone-label exemption **extends to zone-derived field-path nouns** in golden deviation reports
(`risk_reward.zones.buy_top`, `verdict_facts.present_price_in_buy_zone`) — nouns naming the
user-defined bands, gated by a `core::golden`-local posture test *(absorbed at ssg-1.1.0)*.

## 7. Golden tolerance (FR9 / NFR-C2)

- **Zoning and the categorical verdict must match EXACTLY** (Buy/Neutral/Sell; quality-candidate
  yes/no).
- Derived **numeric** values must match within **±0.5%** relative (`|a − b| ≤ 0.005 × |expected|`).
  This is the **fixed method default** (`core::method::golden_relative_tolerance`); a test may compare
  with a tighter local epsilon, but changing this constant is a method change (it is fingerprinted).
  [PRD NFR-C2]

Golden-fixture policy *(absorbed at ssg-1.1.0)*:
- **Stale `method_version` ⇒ check failure.** A fixture whose `meta.method_version ≠ METHOD_VERSION`
  fails its check with a single `meta.method_version` deviation, **before** any comparison runs. A
  golden derived under an older method is re-validated by hand at each method bump — never silently
  replayed against the new method.
- **`fixture_format_version`** is a fourth version axis scoped to fixtures (distinct from
  `schema_version`, `METHOD_VERSION` and the app version), pinned to **1**; any other value is a
  parse error.
- In a fixture's `expected` block, the per-year tables (`management.per_year`, `valuation.per_year`)
  and `normalize_findings` are **optional** (omitted = not asserted). Every other expected field is
  presence-enforced: an **omitted** required field is a parse error; an explicit **`null`** means
  "expected unknown".
- App-bundled goldens (`app/assets/golden/`) are a **byte-identical full copy** of the CI fixtures,
  enforced by a drift test (distinct *role*, not distinct content).

> **Normative comparators.** The operators in the threshold tables below and in §2 are **normative** —
> the engine must use exactly the stated comparator (`>`, `≥`, `<`, `≤`). The `core` constants pin the
> magnitudes; the comparators here pin the boundary behavior. (E.g. `> 20` excludes exactly 20.0;
> `relative value < 100%` for a quality candidate vs `≥ 100%` for the `relative_value_high` flag.)

## 8. Rounding mode & per-field display scale

- **Named rounding mode: half-up — `RoundingStrategy::MidpointAwayFromZero`** (e.g. 2.5 → 3). Chosen
  for fidelity to the paper-form convention; differs from `rust_decimal`'s default banker's rounding.
  **Rounding is applied ONLY at display**, never mid-calculation (calculations keep full decimal
  precision). [Architecture: "named rounding mode + per-field display scale ... only at display"]
- **Per-field display scale** (decimal places):

| Field group | Scale |
|-------------|-------|
| Prices (high/low/current/forecast/zone bounds) | 2 |
| EPS, dividend per share | 2 |
| P/E ratios (high/low/avg/current) | 1 |
| Percentages (PTP, ROE, payout, yield, growth, relative value) | 1 |
| Upside/downside ratio | 1 |
| Sales / large monetary aggregates | 0 |

---

## 9. Degenerate inputs & undefined cases (binding for the engine, Story 1.8)

The engine must never panic or emit a plausible-but-wrong number on a degenerate input. Each case
below resolves to either a typed `unknown/insufficient` result (carried into the verdict, which then
degrades/withholds) or a named plausibility/quality state — never a silent 0 and never a `Decimal`
division-by-zero panic.

| Case | Rule |
|------|------|
| **U/D denominator ≤ 0** (`current_price ≤ forecast_low`; the §4 constraint allows equality) | U/D is **undefined** → verdict withheld for the U/D criterion; surface as a state, not a number. If `current_price < forecast_low`, also raise `low_price_above_current`. |
| **CAGR base ≤ 0 or sign-crossing** (start EPS ≤ 0, or start/end opposite signs) | CAGR is **unknown/insufficient** (do not compute `(end/start)^(1/n)`); the affected growth output is `unknown`, never 0. |
| **Current P/E with TTM EPS ≤ 0** (`Σ last 4 quarterly EPS ≤ 0`) | Current P/E **unknown** → relative value and `relative_value_high` are **unknown** (not computed); verdict's relative-value criterion is unmet-by-insufficiency. |
| **Per-year P/E with EPS ≤ 0** | that year's P/E is `unknown` (`negative_or_zero_denominator`), excluded from the 5-yr P/E averages. |
| **ROE with book value ≤ 0** | ROE `unknown` (`negative_or_zero_denominator`), excluded from the 5-yr ROE average/trend. |
| **PTP gross-up with `tax_rate ≥ 1`** | `pre_tax_profit = net_profit / (1 − tax_rate)` is **unknown** (non-positive denominator); prefer a directly-reported pre-tax profit. |
| **Forecast-low option (d) with `high_yield ≤ 0`** (non-dividend payer) | option (d) is **not selectable** (division by zero); the user must pick (a)/(b)/(c). |

## Change control
Any edit to a formula, threshold, the banned-verb list, the tolerance, the rounding mode, or a display
scale **must** bump `METHOD_VERSION` (next: `ssg-1.3.0` for additive, `ssg-2.0.0` for breaking). The
`core` change-detection test will fail until the version is bumped and the snapshot regenerated, and
every golden fixture's `meta.method_version` must be re-validated by hand (§7). A change to what a
historical input IS (§0) — which period a figure covers, which EPS is used — bumps it too, although
no `core` constant moves (the precedent is `ssg-1.2.0`): the figures a study shows change, and that
must never happen silently.
