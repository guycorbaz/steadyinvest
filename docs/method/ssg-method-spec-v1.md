# steadyinvest — SSG Method Specification (v1)

**`method_version`: `ssg-1.0.0`**
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

---

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

### §2 — Management
For each year and as a **5-year average** + **trend** (up / even / down):
- **% pre-tax profit on sales (PTP)** = `pre_tax_profit / sales × 100`. If only after-tax net profit
  and tax rate are available: `pre_tax_profit = net_profit / (1 − tax_rate)`. [Tutorial p11–12]
- **% earned on equity (ROE)** = `eps / book_value_per_share × 100`. [Tutorial p12]
- 5-year average = arithmetic mean of the last 5 usable years. Trend = comparison of recent years to
  the 5-year average (see quality flags). [Tutorial p12]

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

### §5 — 5-year potential
- **Present yield** = `present_full_year_dividend / current_price × 100`. [Tutorial p21]
- **Average annual EPS (next 5 yrs)** = mean of projected yearly EPS (or the middle-year projected EPS).
- **Average annual dividend** = `avg_annual_eps × avg_payout_pct/100`. [Tutorial p21]
- **Average yield** = `average_annual_dividend / current_price × 100`. [Tutorial p21]
- **Projected price appreciation %** = `(forecast_high − current_price) / current_price × 100`.
- **Projected total annualised return %** = annualised appreciation (current_price → forecast_high over
  the horizon) **plus** average yield. (Study return projection uses the **gross** dividend.)

### Verdict (derived, neutral — see FR13)
A **fact-only** verdict states the present-price zone and the supporting figures. A study is a
"quality-and-value" candidate when ALL hold (these are *facts surfaced*, never a recommendation):
- U/D ratio ≥ **3.0** [Tutorial p20]; **and** relative value < **100%**; **and** present price in the
  **Buy** zone; **and** projected appreciation implies roughly doubling over 5 years (≈ 15%/yr).
  [Tutorial p20]
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
| `high_debt` | total debt is sizeable vs the firm's own history (flag for review; no hard ratio in v1) | info |
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

## 7. Golden tolerance (FR9 / NFR-C2)

- **Zoning and the categorical verdict must match EXACTLY** (Buy/Neutral/Sell; quality-candidate
  yes/no).
- Derived **numeric** values must match within **±0.5%** relative (`|a − b| ≤ 0.005 × |expected|`).
  This is the **fixed method default** (`core::method::golden_relative_tolerance`); a test may compare
  with a tighter local epsilon, but changing this constant is a method change (it is fingerprinted).
  [PRD NFR-C2]

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
scale **must** bump `METHOD_VERSION` (next: `ssg-1.1.0` for additive, `ssg-2.0.0` for breaking). The
`core` change-detection test will fail until the version is bumped and the snapshot regenerated.
