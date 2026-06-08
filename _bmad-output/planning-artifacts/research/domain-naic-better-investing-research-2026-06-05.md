---
stepsCompleted: [1, 2, 3, 4, 5, 6]
lastStep: 6
inputDocuments:
  - docs/NAIC/SSGHandbook.pdf
  - docs/NAIC/SSGPlus_QuickStart.pdf
  - docs/NAIC/Stock Selection Guide Tutorial.pdf
  - docs/NAIC/A-Beginners-Tour-of-the-SSG-Jan-2015.pdf
  - docs/NAIC/forms/Stock Selection Guide and Report.pdf
  - docs/NAIC/forms/Stock Comparison Guide.pdf
  - docs/NAIC/forms/Portfolio Management Guide.pdf
  - docs/NAIC/forms/stock checklist.pdf
workflowType: 'research'
lastStep: 1
research_type: 'domain'
research_topic: 'NAIC / Better Investing investment methodology (for the steadyinvest Rust desktop app)'
research_goals: 'Produce a structured domain document covering (1) Better Investing philosophy, (2) the 5 SSG sections with exact calculation formulas, (3) business judgment rules/thresholds, (4) mapping of each NAIC form to app features, (5) the auto-fetched "magic numbers" and their data sources.'
user_name: 'Guy'
date: '2026-06-05'
web_research_enabled: true
source_verification: true
---

# Research Report: NAIC / Better Investing Investment Methodology

**Date:** 2026-06-05
**Author:** Guy
**Research Type:** domain
**Project:** steadyinvest (Rust desktop investment-management app)

---

## Research Overview

This document is the domain-research foundation for **steadyinvest**, a fully independent, open-source, cross-platform **Rust desktop** application that implements the classic NAIC / BetterInvesting fundamental-analysis methodology (the "Stock Selection Guide" family of forms) while using **neutral terminology** and an **original visual design** to stay clear of trademark/copyright exposure. It was produced primarily from the authoritative NAIC PDFs in `docs/NAIC/`, cross-verified with current (June 2026) web sources on the BetterInvesting ecosystem, financial-data providers, IP/licensing, and the Rust desktop stack.

It covers: the Better Investing philosophy; the full Stock Study (SSG) methodology with **exact calculation formulas** for all five sections; the consolidated **business-judgment rules and thresholds**; the **mapping of each NAIC form to app features**; the **auto-fetched "magic numbers" and candidate data providers**; the competitive/feature benchmark; the **legal/trademark/data-licensing constraints**; and the **Rust technical foundations** (GUI = Slint, charts via plotters/custom canvas, local SQLite). The executive summary, strategic recommendations, and a phased roadmap are in the Research Synthesis section below; together they are intended to feed directly into the BMad Product Brief, PRD, UX and Architecture phases.

---

<!-- Content will be appended sequentially through research workflow steps -->

## Domain Research Scope Confirmation

**Research Topic:** NAIC / Better Investing investment methodology (for the steadyinvest Rust desktop app)

**Research Goals:** Produce a structured domain document that the BMad PRD, UX and Architecture phases can build on.

**Domain Research Scope (tailored to a methodology-extraction project):**

- Better Investing philosophy & principles — quality growth, regular investing, dividend reinvestment, diversification, "Up-Straight-Parallel"
- The SSG in depth — the 5 sections with all exact calculation formulas (average P/E, zoning, upside/downside ratio, 5-year compound return…)
- Business judgment rules & thresholds — growth ≤15-20%, debt ≤30%, 25/50/25 zoning, U/D ratio ≥3:1, declining-margins = reject, etc.
- Form → feature mapping — SSG, Stock Comparison Guide, Portfolio Management Guide, Stock Check List → app features
- "Magic numbers" & data sources — auto-fetched data (sales, EPS, pre-tax profit, high/low prices, dividends, ACE estimates) and current financial-data API providers (availability, coverage, cost, US/EU/CH markets)
- Ecosystem state — existing tools (SSGPlus / BetterInvesting Toolkit, alternatives)
- **Form visual fidelity & UX (design constraint)** — capture the exact layout/structure of each original NAIC form so familiar users are not disrupted, while identifying graphical-augmentation opportunities (interactive charts, colored buy/hold/sell zoning, real-time recalculation, signal color-coding). Guiding principle: *same skeleton as the paper form, analytical layer added on top.*

**Research Methodology:**

- Local NAIC PDFs as primary reference + multi-source web verification
- Confidence levels for uncertain information
- Proper citations

**Scope Confirmed:** 2026-06-05

---

## Domain & Ecosystem Analysis

### The NAIC / BetterInvesting Organization & Method

The National Association of Investors™ (NAIC®), parent of BetterInvesting®, is a U.S. 501(c)(3) non-profit investment-education organization founded in 1951; it reports having helped 5M+ people. Its method rests on four principles: **invest regularly** (dollar-cost averaging), **reinvest all dividends/earnings**, **diversify**, and **buy high-quality growth stocks at a reasonable price**. The Stock Selection Guide (SSG) is the central instrument: find candidate companies, verify company quality, and determine a fair price to pay.
_Confidence: High (primary org sources)._
_Sources: betterinvesting.org/about-us/mission-method-of-stock-investing ; betterinvesting.org_

### Tooling Ecosystem (competitive landscape for steadyinvest)

- **CoreSSG™ / SSGPlus™** — BetterInvesting's official online SSG tools, **powered by Morningstar** data covering 8,000+ stocks. CoreSSG = beginner, conservative, step-by-step; SSGPlus = advanced (more reports, quarterly data, screening, peer comparison, Member Sentiment estimates, mobile). Subscription-based, web/mobile.
- **Toolkit 6 (TK6)** — ICLUBcentral (Doug Gerlach) desktop app; **sales to new users discontinued 2021-09-30**, still supported for existing users; SSGPlus is the recommended successor. Uses `.ITK` study files (a relevant import/interop format to consider).
- **ManifestInvesting** — independent platform (since 2005) built around **PAR (Projected Annual Return)**; complements SSG with Value Line / Morningstar inputs.
- **Positioning insight for steadyinvest:** the market is dominated by *subscription web tools tied to a data vendor (Morningstar)*. A **Rust desktop app** that the user owns, with **pluggable data providers** and **faithful SSG-form fidelity plus an analytical GUI layer**, is a credible differentiator — especially for users who want local data ownership and are not locked to BetterInvesting membership. Note `.ITK`/`.ssg` interop and Morningstar parity as expectations.
_Confidence: High._
_Sources: betterinvesting.org/store/tools/individual-investors ; iclub.com/products/tk6.asp ; manifestinvesting.com_

### Financial Data Providers (the "magic numbers" supply)

The SSG requires ~10 years of annual history (sales, EPS, pre-tax profit, high/low price, dividend, shares outstanding, book value) plus forward analyst estimates (ACE). Candidate providers verified (June 2026):

| Provider | Fundamentals depth | Estimates (ACE) | Free tier | Markets | Indicative paid |
|----------|-------------------|-----------------|-----------|---------|-----------------|
| **EODHD** | 30+ yrs US large, 10 yrs minor | yes | 20 calls/day | 60–70+ exchanges (US+EU) | ~$60/mo fundamentals |
| **Financial Modeling Prep** | yes | yes (revenue/EPS) | limited | US + intl | tiered |
| **Nasdaq Data Link / Sharadar (SF1)** | 24 yrs, 16,000 US cos | — | preview | US only | low single-user |
| **Finnhub** | yes | EPS estimates | yes (generous) | US + intl | tiered |
| **Alpha Vantage / Twelve Data** | yes | partial | yes | US + intl | tiered |
| **Refinitiv I/B/E/S / Zacks / FactSet** | institutional | gold-standard | no | global | enterprise |

**Key findings:** (1) **US coverage is deep and inexpensive**; (2) **European / Swiss coverage requires a premium multi-exchange provider** (EODHD, Tradefeeds) — decisive given Guy may want CH/EU stocks; (3) a **provider-abstraction layer** in the Rust architecture is strongly indicated so the data source can be swapped/configured; (4) Morningstar (the official SSG source) is not openly API-accessible at hobbyist tiers — parity must come from the above alternatives.
_Confidence: High (multiple vendor sources, June 2026)._
_Sources: eodhd.com ; site.financialmodelingprep.com/developer/docs ; data.nasdaq.com/databases/SF1 ; finnhub.io ; alphavantage.co ; twelvedata.com ; developer.factset.com ; data.nasdaq.com/databases/ZEEH_

### Market Maturity & Implication

The SSG method is a mature, 70+-year-old, well-documented retail fundamental-analysis discipline with a small but loyal user base and aging tooling (TK6 sunset, web tools behind membership). This favors a **modern, owned, extensible desktop implementation** that stays faithful to the familiar forms while adding interactive analytics — exactly the steadyinvest thesis.
_Confidence: Medium-High._

---

## Competitive Landscape — Feature Benchmark

Verified against BetterInvesting's official CoreSSG vs SSGPlus vs Toolkit 6 feature-comparison sheet. This doubles as a **feature backlog reference** for steadyinvest.

### Feature matrix (key rows)

| Capability | CoreSSG | SSGPlus | Toolkit 6 | steadyinvest target |
|---|---|---|---|---|
| Platform | Web (Win/Mac/iOS/Android) | Web | **Local install, Windows only, no Mac** | **Rust desktop, cross-platform (Win/Mac/Linux)** |
| Data source | Morningstar | Morningstar | Separate purchase | **Pluggable providers (EODHD/FMP/Finnhub…)** |
| 10-yr integrated data | Yes | Yes | No (separate) | Yes (auto-fetch) |
| Quarterly data (PERT-A) | No | Yes (data+graph) | Yes | Yes |
| Visual Analysis graph items | 12 | 21 | 16 | ≥21 (interactive) |
| Movable graph lines (judgment) | No | **Yes** | Yes | **Yes** (core differentiator) |
| Preferred Procedure Calculator | No | Yes | Yes | Yes |
| Growth-trend data EPS/Sales | No | Yes | Yes | Yes |
| Low-price options | 3 | 5 | 7 | configurable |
| Judgment Audit | No | Yes | Yes | Yes |
| Outlier selection | by year | **by cell** | by year | by cell |
| Max saved studies | 50 | 1000 | Unlimited | **Unlimited (local)** |
| **Offline usage** | No | No | **Yes** | **Yes (key advantage)** |
| Import OLT/TK studies (.ITK) | Yes | Yes | Yes | **Yes (interop)** |
| Peer comparison items | 8 | 9 | No | yes |
| Stock Comparison Guide items | 25 | **46** | 30 | ≥30 |
| Compare to industry averages | Yes | Yes | No | yes |
| Portfolio reports | 1 | 4 | 6 | several |
| PERT report | No | Yes | Yes | Yes |
| Diversification reports | No | 2 | 2 | Yes |
| Stock screening criteria | 5 | 13 | No | yes |
| Ticker heat map | No | Yes | No | optional |
| Member Sentiment (community) | Yes | Yes | No | out of scope (personal app) |

### Strategic reading

- **The empty quadrant steadyinvest fills:** Toolkit 6 was the only **offline, desktop, unlimited-studies** option — and it is sunset (Win-only, no new sales since 2021). The web tools (SSGPlus) are richest analytically but are **online-only, membership-gated, Morningstar-locked**. steadyinvest = **TK6's ownership/offline model + SSGPlus's analytical depth + provider independence**, cross-platform via Rust.
- **Table-stakes features** (must-have for credibility with NAIC users): full 2-page SSG, PERT-A quarterly, Visual Analysis graphs with **movable/judgment lines**, Preferred Procedure Calculator, Judgment Audit, Stock Comparison Guide, Portfolio Management with PERT & diversification reports.
- **Interop expectation:** ability to import legacy `.ITK` / OLT study files lowers switching cost for existing TK6/SSGPlus users.
- **Out of scope** (community/network features): Member Sentiment estimates, online study sharing — not meaningful for a single-user owned app (could be a later export/share feature).
_Confidence: High (official BetterInvesting comparison sheet)._
_Source: betterinvesting.org/getmedia/492bb3c4-aa27-4672-8b27-58b06b93be13/corevplusvtoolkit-keyfeatures-differences.pdf_

---

## Legal, Trademark & Licensing Considerations (open-source constraint)

> **Project decision (Guy, 2026-06-05):** steadyinvest is a **fully independent, open-source (GitHub) desktop app**. It must **not lean on NAIC/BetterInvesting branding** to avoid legal exposure, while keeping the **methodology and calculations exactly identical**. _This section is informational, not legal advice — a brief IP-lawyer review is recommended before public release._

### Trademark landscape (verified)

The following are **registered trademarks** of the National Association of Investors Corporation, and the corresponding paper forms are **copyrighted**: NAIC®, BetterInvesting®, the BetterInvesting logo, **Stock Selection Guide®**, **Stock Comparison Guide®**, **Portfolio Management Guide®**, **PERT / Portfolio Evaluation Review Technique**, and *National Association of Investors Corporation*.
_Source: iclub.com/downloads/NAIC Stock Analyst Manual.pdf ; betterinvesting.org/about-us/mission-method-of-stock-investing_

### What is safe vs. what to avoid

- ✅ **Safe — the method itself:** mathematical formulas, financial ratios, the 5-step analytical logic, thresholds, and the *idea* of zoning/upside-downside. Facts and methods are not copyrightable; you may implement them freely.
- ⚠️ **Avoid — protected expression & marks:** the registered names above as product/feature labels; verbatim copies of the copyrighted form layouts, their exact wording, and the BetterInvesting/NAIC logos. Re-implement the **structure** (so familiar users feel at home) with **original, neutral labels and original visual design**.
- ✅ **Recommended posture:** an optional one-line factual note ("methodology inspired by classic fundamental growth-investing techniques") without claiming affiliation or endorsement. Do **not** state or imply NAIC/BetterInvesting endorsement.

### Neutral terminology mapping (NAIC term → steadyinvest neutral label)

Concept identical; name neutralized. Proposed defaults (final naming to be confirmed in PRD/UX):

| NAIC / trademarked term | Neutral steadyinvest label (proposed) |
|---|---|
| Stock Selection Guide (SSG) | **Stock Study** (a.k.a. "Company Study") |
| §1 Visual Analysis of Sales, Earnings & Price | **Growth Trend Analysis** |
| §2 Evaluating Management | **Management Quality** |
| §3 Price-Earnings History | **Valuation History** |
| §4 Evaluating Risk & Reward | **Risk / Reward Zones** |
| §5 Five-Year Potential | **5-Year Return Projection** |
| Stock Comparison Guide (SCG) | **Company Comparison** |
| Portfolio Management Guide (PMG) | **Portfolio Tracker** |
| PERT report | **Portfolio Health Review** |
| Stock Check List | **Quick Screen / Starter Checklist** |
| Buy / Maybe / Sell zones | (generic — keep as is) |
| Upside-Downside Ratio | (generic — keep as is) |
| "Up, Straight & Parallel" slogan | **"Steady, parallel growth"** (paraphrase) |

### Data licensing & redistribution (critical for open source)

- **Access ≠ redistribution rights.** Market-data vendor ToS generally permit *use/display* under your own key but **forbid redistributing or bundling raw data inside a product**. Therefore: **ship no vendor data in the repo**; the user supplies their **own API key**; the app fetches data at runtime into a **local** store.
- Alpha Vantage explicitly welcomes open-source *wrappers* but requires preserving response content; Finnhub/EODHD redistribution terms are stricter and plan-dependent → verify each provider's ToS before declaring official support.
- **Architecture implication:** a **pluggable data-provider abstraction** with per-provider ToS notes, and clear separation between (a) the open-source code and (b) the user's private data/keys (kept out of version control).
_Confidence: High (trademark/copyright); Medium (per-vendor redistribution specifics — verify ToS individually)._
_Sources: alphavantage.co/support ; eodhd.com/financial-academy/financial-faq/the-2026-market-data-api-scorecard-comparing-6-leading-providers_

### Compliance / disclaimer posture

- Include a prominent **"Educational use only — not investment advice"** disclaimer (mirrors NAIC's own posture), reducing regulatory risk for a tool that surfaces buy/sell signals.
- No handling of brokerage execution or personal financial accounts in scope → keeps the app outside regulated financial-advice/brokerage territory.
- Choose an explicit **OSS license** (e.g. MIT/Apache-2.0 permissive, or GPL-3.0 copyleft) — decision deferred to Guy; note it interacts with any third-party Rust crates' licenses.

---

## Technical Foundations (Rust desktop stack)

Verified June 2026. These inform the BMad **Architecture** phase; final selection is an architecture decision, not made here.

### Desktop GUI framework options

The Rust GUI ecosystem is maturing but uneven (a 2025 survey notes most libraries are not yet "production-ready" for i18n/accessibility without effort). Three credible candidates for steadyinvest, scored against our two needs — **(A) faithful form-layout fidelity** and **(B) interactive analytical charts incl. movable judgment lines**:

| Framework | Style | Form fidelity (A) | Interactive charts (B) | Notes |
|---|---|---|---|---|
| **egui** (+ egui_plot / egui_charts) | Immediate-mode, pure Rust | Good (custom grids/canvas; styling is manual) | **Excellent** — immediate mode makes real-time recalculation & **draggable judgment lines** natural; egui_charts = TradingView-quality | Best fit for data-dense analytical UI; pure-Rust aligns with goals |
| **Slint** | Declarative DSL, native | **Excellent** — great for pixel-faithful form layouts, good tooling | Moderate (integrate plotters; fewer ready-made interactive plots) | Strong for form fidelity; charts need more work |
| **Tauri** (Rust core + web UI) | Webview, smallest-vs-Electron | Excellent (HTML/CSS) | Excellent (mature web charting) | Introduces a JS/web layer — departs from "pure Rust", adds build complexity |

_**Decision (Guy, 2026-06-05): Slint.**_ Guy already uses Slint on other projects, so familiarity and cross-project reuse are decisive. Slint is also the strongest option for **faithful form-layout fidelity** (a key requirement). **Trade-off to manage:** Slint's charting is less mature than egui's — the interactive growth/valuation charts and the signature **draggable "judgment" lines + buy/hold/sell zone bands** must be built via **plotters integration or a custom Slint canvas**, and should be **prototyped early** as the principal Slint risk. (egui remains the fallback if charting fidelity proves too costly.)
_Sources: boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html ; weeklyrust.substack.com/p/the-state-of-rust-gui-the-good-and_

### Charting libraries

- **egui_charts** — high-performance financial charting for egui (candlesticks, OHLC, line/area, 130+ indicators, drawing tools); embeddable in egui/Tauri. Strong match for the Growth Trend & valuation charts.
- **egui_plot** — lighter 2D plotting integrated with egui; sufficient for the SSG line charts and **custom draggable overlays** (judgment lines, buy/hold/sell zone bands).
- **plotters** — pure-Rust drawing lib with multiple backends; supports line, point, **candlestick**, histogram; good for static/exportable report charts (and usable from Slint/Tauri).
_Sources: github.com/emilk/egui_plot ; docs.rs/egui-charts ; github.com/plotters-rs/plotters_

### Local persistence

- **rusqlite (bundled SQLite)** — the obvious choice for an offline, owned desktop app: SQLite compiled into the binary (no system dependency), single local file, runs anywhere. Holds studies, fetched financial data cache, portfolios, and judgment inputs.
- **SQLx** — alternative if compile-time-checked SQL / async is wanted; heavier than needed for a single-user desktop tool.
- Avoid sled for primary storage (reported startup/memory issues at scale).
_Sources: aarambhdevhub.medium.com/rust-orms-in-2026-... ; users.rust-lang.org/t/native-alternative-for-sqlite/119051_

### Architecture implications (carried to Architecture phase)

1. **Data-provider abstraction** — a Rust trait (e.g. `MarketDataProvider`) with per-vendor adapters (EODHD, FMP, Finnhub…), user-supplied API keys stored outside version control (OS keychain / local config).
2. **Local-first** — all fetched data cached in SQLite; full offline operation after fetch (the TK6-style advantage).
3. **Calculation core as a pure library crate** — the SSG math (growth rates, P/E averages, zoning, U/D ratio, projected return) implemented as a deterministic, unit-tested, UI-independent crate; the GUI and any future CLI/export reuse it.
4. **Separation of open-source code vs user private data/keys** — keys & studies never committed; repo ships code + sample data only.
5. **Cross-platform** (Win/Mac/Linux) — a hard requirement that rules out Win-only stacks and is satisfied by egui/Slint/Tauri.
_Confidence: High (current ecosystem sources, June 2026)._

## Recommendations

### Technology adoption strategy
- Adopt a **pure-Rust, local-first** stack (egui + egui_plot/egui_charts + rusqlite) to honor the independent/offline/OSS goals; revisit Slint if UX prototyping shows form-fidelity gaps.
- Build the **SSG calculation engine first** as a standalone tested crate — it is the methodological heart and the lowest-risk, highest-reuse component.

### Innovation roadmap
- Phase 1: calculation core + single-stock Study (the SSG) with auto-fetch + interactive growth chart with judgment lines.
- Phase 2: Company Comparison + Portfolio Tracker + Portfolio Health Review.
- Phase 3: screening, diversification reports, `.ITK`/legacy import, multi-portfolio & risk views.

### Risk mitigation
- Mitigate GUI-ecosystem immaturity by keeping UI thin over the tested core, so a framework swap stays cheap.
- Mitigate data-vendor lock-in/ToS risk via the provider abstraction and clear per-provider redistribution notes.
- Mitigate legal risk via neutral terminology, original form design, and the educational-use disclaimer.

---

# Research Synthesis: A Modern, Independent Implementation of Classic Growth-Investing Analysis

## Executive Summary

The NAIC / BetterInvesting **Stock Selection Guide (SSG)** is a mature (70+ year), well-documented, deterministic methodology for judging whether a company is a **quality growth business** and whether its stock trades at a **reasonable price**. Its analytical value lies not in secret data but in a disciplined, repeatable procedure: chart 10 years of sales/earnings/price, test management quality (margins, ROE, debt), study the P/E history, project a 5-year high/low price, and convert that into **buy / hold / sell zones**, an **upside-downside ratio**, and a **projected total return**. Because the procedure is mathematical, it can be re-implemented freely; only the **names and exact form layouts** are protected.

The tooling market is dominated by subscription web apps tied to Morningstar data (CoreSSG/SSGPlus) and a sunset Windows-only desktop tool (Toolkit 6). This leaves a clear opening for **steadyinvest**: an **owned, offline-capable, cross-platform, open-source desktop app** with **pluggable financial-data providers**, faithful (but neutrally-labelled, originally-designed) forms, and an **interactive analytical layer** (live recalculation, draggable judgment lines, colored zones) on top.

**Key findings**

- The methodology is fully specifiable as a **pure, unit-testable calculation crate** (see formulas below) — the lowest-risk, highest-reuse core of the app.
- **US fundamental + estimate data is deep and cheap**; **EU/Swiss coverage needs a premium multi-exchange provider** (EODHD/Tradefeeds) → a provider abstraction is mandatory, not optional.
- **NAIC, BetterInvesting, SSG, PERT, and the form names are registered trademarks** and the forms are copyrighted → use neutral terminology + original design; ship no vendor data (user brings own API key).
- **GUI = Slint** (Guy's choice / cross-project reuse); principal risk is interactive charting → prototype the growth chart with judgment lines early via plotters or a custom canvas.

**Strategic recommendations**

1. Build the **Stock-Study calculation engine** (the 5-section SSG math) first, as a UI-independent tested crate.
2. Wrap it in a **Slint** desktop UI that reproduces the form structure with neutral labels + an interactive growth/valuation chart.
3. Add a **`MarketDataProvider` trait** with one concrete adapter first (EODHD or FMP — broad coverage + estimates), keys stored in the OS keychain.
4. Persist everything in **local SQLite (rusqlite)** for full offline ownership.
5. Sequence features as Study → Comparison → Portfolio → Screening/Reports (roadmap below).

## Table of Contents

1. Research Introduction & Methodology
2. Domain & Ecosystem Analysis *(above)*
3. Competitive Landscape — Feature Benchmark *(above)*
4. Legal, Trademark & Licensing Considerations *(above)*
5. Technical Foundations (Rust desktop stack) *(above)*
6. **Methodology Reference — The Stock Study (SSG) in full** *(below)*
7. **Consolidated Business Rules & Thresholds** *(below)*
8. **Form → Feature Mapping** *(below)*
9. **Data Requirements — the "Magic Numbers"** *(below)*
10. Strategic Insights, Roadmap & Conclusion *(below)*

---

## 6. Methodology Reference — The Stock Study (SSG) in full

> Neutral product name: **Stock Study**. Concept identical to the NAIC Stock Selection Guide. All formulas below are the methodology's actual calculations and are implementation-ready. `n` = number of year-intervals across the historical window.

### Section 1 — Growth Trend Analysis *(Visual Analysis of Sales, Earnings & Price)*

**Inputs:** ~10 years of annual **Sales**, **EPS**, (Pre-tax Profit), and yearly **High/Low price**; plus most-recent quarter and year-ago quarter Sales & EPS.

**Computed:**
- Historical Sales Growth (CAGR) = `(Sales_end / Sales_begin)^(1/n) − 1`
- Historical EPS Growth (CAGR) = `(EPS_end / EPS_begin)^(1/n) − 1`
- Recent Quarterly % Change = `(latest_qtr − year_ago_qtr) / year_ago_qtr × 100` (for Sales and EPS)

**User judgment (inputs):** Estimated Future Sales Growth %, Estimated Future EPS Growth % → projected Sales & EPS to year +5. Visual test: lines should be **up, straight, parallel** (steady growth) on a semi-log scale.

### Section 2 — Management Quality *(Evaluating Management)*

- **% Pre-Tax Profit on Sales (margin)** = `Pre-Tax Profit / Sales × 100` — per year, 5-yr average, and trend (up/down).
- **% Earned on Equity (ROE)** = `EPS / Book Value per Share × 100` (≡ Net Income / Shareholders' Equity) — per year, 5-yr average, trend.
- **% Debt to Capital** = `Total Debt / Total Capital × 100` (guideline below).

### Section 3 — Valuation History *(Price-Earnings History, last 5 years)*

Per year, columns A–H:
- A = High Price · B = Low Price · C = EPS
- **D = High P/E = A / C** · **E = Low P/E = B / C**
- F = Dividend per Share · **G = % Payout = F / C × 100** · **H = % High Yield = F / B × 100**

Aggregates:
- Avg High P/E = mean(D); Avg Low P/E = mean(E)
- **(8) Average P/E = (Avg High P/E + Avg Low P/E) / 2**
- **(9) Current P/E = Present Price / current TTM EPS**

### Section 4 — Risk / Reward Zones *(Evaluating Risk & Reward, next 5 years)*

- **(A) Forecast High Price** = `Forecast Avg High P/E × Estimated High EPS(yr+5)`
- **(B) Forecast Low Price** — choose one of:
  - (a) `Forecast Avg Low P/E × Estimated Low EPS`
  - (b) Avg Low Price of last 5 years
  - (c) Recent Severe Market Low Price
  - (d) **Price Dividend Will Support** = `Present Dividend / (Forecast High Yield H)`
- **(C) Zoning** — `Range = Forecast High − Forecast Low`. Default software split **25 / 50 / 25** (classic paper form uses thirds):
  - **Buy zone** = Low … Low + 25% of Range
  - **Hold/Maybe zone** = middle 50%
  - **Sell zone** = upper 25% … High
  - Locate Present Price → Buy / Hold / Sell.
- **(D) Up-Side / Down-Side Ratio** = `(Forecast High − Present Price) / (Present Price − Forecast Low)` → **target ≥ 3 : 1**.
- **(E) 5-Year Price Target (simple appreciation)** = `(Forecast High / Present Price) × 100 − 100` (%).

### Section 5 — 5-Year Return Projection *(Five-Year Potential)*

- **(A) Present Yield** = `Present Annual Dividend / Present Price × 100`
- **(B) Average Yield (next 5 yrs)** = `(Avg EPS next 5 yrs × Avg % Payout) / Present Price` (× 100)
- **(C) Estimated Average Total Annual Return** = `(5-yr Appreciation Potential / 5) + Average Yield` → a **simple** rate; convert to a **compound** rate via the NAIC conversion table. Also computed two ways: using Forecast **High** P/E and using Forecast **Average** P/E (annualized total return).
- **Result summary:** Zone (Buy/Hold/Sell), U/D Ratio, Total Return (High P/E), Projected Return (Avg P/E).

---

## 7. Consolidated Business Rules & Thresholds

These are the judgment guardrails the app should encode (as validations, warnings, and defaults):

- **Growth estimates:** future Sales growth **≤ 15%** without a specific reason, **never > 20%**; **Estimated EPS growth ≤ Sales growth**.
- **Margins:** if **% Pre-Tax Profit on Sales is declining**, flag the study as weak / candidate to discard.
- **ROE:** prefer **> ~10–15%**, stable or rising.
- **Debt:** **% Debt to Capital ≤ ~30%** for a typical company (flag above).
- **"GASP" P/E rule:** when historical P/Es are abnormally high/volatile, use **lower, conservative** forecast P/E values.
- **Buy discipline:** consider buying only when Present Price is in the **Buy zone** AND **U/D ≥ 3:1** AND projected total return meets the user's objective (commonly **~15%/yr**, i.e. roughly doubling in 5 years, for growth stocks).
- **Sell/hold:** Present Price in the **Sell zone**, or deteriorating quality (margins/ROE/growth), signals review/sell.
- **Rule of Five (expectation-setting):** across 5 studied companies, expect ~1 to disappoint, ~3 on target, ~1 to outperform → reinforces diversification.

---

## 8. Form → Feature Mapping

| NAIC form (trademarked) | steadyinvest feature (neutral) | Purpose | Built on |
|---|---|---|---|
| Stock Selection Guide | **Stock Study** | Deep single-company analysis, 5 sections, buy/hold/sell verdict | Calculation engine + interactive charts |
| Stock Comparison Guide | **Company Comparison** | Side-by-side of up to ~5 companies on ~30 metrics pulled from each Study | Reads saved Studies |
| Portfolio Management Guide | **Portfolio Tracker** | Track holdings' price/P/E zones over time; market-price-vs-P/E chart; cumulative earnings & current P/E | Studies + price feed |
| (PERT roll-up, SSGPlus) | **Portfolio Health Review** | Roll up each holding's quality/valuation signals into one portfolio view; diversification by size/sector | Aggregates Studies |
| Stock Check List | **Quick Screen / Starter Checklist** | Simplified beginner pass (sales & EPS compounding, price record, conclusion) before a full Study | Subset of the engine |

This mapping directly answers Guy's stated goals: *choose a new stock* (Stock Study, Quick Screen, Company Comparison), *manage portfolio state* (Portfolio Tracker), *decide buy/sell* (zoning + U/D in the Study; Portfolio Tracker zones), *manage risk* (U/D ratio, debt/margin flags, Portfolio Health Review diversification), and *save time* (auto-fetch of the magic numbers).

---

## 9. Data Requirements — the "Magic Numbers"

**Required per company** (≈10 yrs history + current + forward estimates):

| Field | Used in | Frequency |
|---|---|---|
| Net Sales / Revenue | §1 growth, §2 margin | annual (10y) + latest quarters |
| EPS (diluted) | §1, §3 (C), §5 | annual (10y) + TTM + quarterly |
| Pre-Tax Profit | §1, §2 (A) | annual (10y) |
| Shares Outstanding | quality/ROE, dilution | annual |
| Book Value / Equity | §2 (B) ROE | annual |
| Total Debt / Capital | §2 debt | annual |
| High & Low Price (per year) | §1, §3 (A,B) | annual (5–10y) |
| Present Price | §3 (9), §4, §5 | live/daily |
| Dividend per Share | §3 (F,G,H), §5 | annual + current |
| **Analyst estimates (ACE):** fwd Sales & EPS growth | §1 projections, §4 | forward 1–5y |

**Provider strategy:** a single `MarketDataProvider` trait; ship adapters incrementally. **EODHD** (broad US+EU/CH coverage, fundamentals + estimates) or **FMP** (good estimates) as the first adapter; **Finnhub/Alpha Vantage** as free fallbacks for US. Keys are user-supplied; data cached locally; **no vendor data committed to the repo**.

---

## 10. Strategic Insights, Roadmap & Conclusion

**Phased roadmap (feeds BMad epics):**
- **Phase 1 — Core Study:** calculation engine crate → single **Stock Study** with manual data entry → add **auto-fetch** (1 provider) → interactive **Growth Trend** chart with draggable judgment lines + colored buy/hold/sell zones.
- **Phase 2 — Portfolio & Comparison:** **Portfolio Tracker**, **Company Comparison**, **Portfolio Health Review** (diversification + quality roll-up).
- **Phase 3 — Breadth:** stock **screening**, additional reports, **multi-portfolio & risk views**, legacy `.ITK`/study import, export/share.

**Risk register (top):** (1) Slint interactive-charting maturity → early prototype; (2) data-vendor coverage/ToS for EU/CH + redistribution → provider abstraction + per-provider notes; (3) IP/trademark → neutral terms, original design, disclaimer, optional legal review; (4) Rust GUI ecosystem churn → keep UI thin over the tested core.

**Conclusion & next steps:** The methodology is fully captured and implementation-ready; the market gap is real and well-defined; the technical and legal constraints are understood and have concrete mitigations. **Recommended next BMad step: a Product Brief**, then PRD → UX (form fidelity + augmentation) → Architecture (Slint + provider trait + SQLite + calculation crate) → Epics & Stories following the phased roadmap above.

---

**Research Completion Date:** 2026-06-05
**Source Verification:** Local NAIC PDFs (primary) + current web sources (June 2026), cited inline per section
**Confidence Level:** High on methodology & IP; High on tech stack; Medium on per-vendor data ToS specifics (verify individually before declaring support)
