---
title: "Product Brief Distillate: steadyinvest"
type: llm-distillate
source: "product-brief-steadyinvest.md"
created: "2026-06-05"
purpose: "Token-efficient context for downstream PRD creation"
---

# steadyinvest — Detail Pack for PRD

> Companion to `product-brief-steadyinvest.md`. Primary domain reference: `_bmad-output/planning-artifacts/research/domain-naic-better-investing-research-2026-06-05.md` (full SSG formulas, business rules, provider analysis, IP, tech). Comm language FR; document output EN.

## Product essence
- Independent, **open-source** (GitHub) **desktop** app implementing the NAIC/BetterInvesting **quality-growth** method (Stock Selection Guide family) with a faithful-forms + interactive-analytics UX.
- **Primary user: the author (Guy)**; publishable for other individual investors / informal clubs. Don't optimize v1 for stranger onboarding, but keep general + documented.
- **Primary success metric:** complete a trustworthy stock study in *a few minutes* via auto-fetch (vs slow manual/spreadsheet).

## Scope signals (MVP & beyond)
- **Ambition = complete vision** (all capability blocks); **delivery sequenced via epics** so the Stock Study ships first.
- **Confirmed in v1 (cross-cutting):**
  - First-class **manual data entry/override** for every field (not a fallback) — keeps app usable when provider coverage is partial.
  - **Multi-currency CHF/EUR/USD** with FX conversion (native currency per security + aggregation).
  - **Watchlist + alerts** ("entered Buy zone").
- **Delivery phases:** (1) calc engine + Stock Study + auto-fetch + manual entry + interactive growth chart (judgment lines, colored zones) + watchlist/alerts; (2) Portfolio Tracker (regular price refresh, per-holding zones, multi-currency) + Company Comparison + Portfolio Health Review (diversification/quality); (3) screening, more reports, multi-portfolio & risk views, legacy import, export/share.
- **Out (now):** brokerage execution/account linking; community/social (member sentiment, online sharing); real-time **intraday as a hard requirement** (daily EOD suffices; intraday = later nice-to-have).
- **Roadmap (post-v1):** study history/versioning; legacy `.ITK`/SSGPlus import; PDF/print export of a study ("& Report").

## Requirements hints (functional)
- **Stock Study** = the 5 SSG sections, all calculations automated, editable judgment inputs, live recalculation.
- **Regular price refresh** of holdings + watchlist; configurable cadence (≥ daily EOD) + manual "refresh now"; **batch quotes** (1 call for N tickers) to respect API rate limits; timestamped, cached locally for offline use.
- **Quality flags** surface automatically: declining pre-tax margin, debt > ~30%, weak/declining ROE.
- **Buy discipline** signal: price in Buy zone AND U/D ≥ 3:1 AND projected return ≥ user objective (~15%/yr typical).

## SSG methodology — exact formulas to implement (calc engine)
- **§1 Growth:** Historical CAGR = (end/begin)^(1/n) − 1 for Sales & EPS. Quarterly %Δ = (latest − yearAgo)/yearAgo×100. User judgment: future Sales% & EPS% (projected to yr+5).
- **§2 Management:** %Pre-Tax Profit on Sales = PTP/Sales×100; ROE = EPS/BookValuePerShare×100; %Debt-to-Capital. Each: per-year + 5y avg + trend.
- **§3 Valuation (5y table A–H):** D HighP/E=A/C, E LowP/E=B/C, G %Payout=F/C×100, H %HighYield=F/B×100. AvgHighP/E=mean(D), AvgLowP/E=mean(E). **Average P/E=(AvgHighP/E+AvgLowP/E)/2**. **Current P/E=PresentPrice/TTM EPS**.
- **§4 Risk/Reward:** ForecastHigh = ForecastAvgHighP/E × EstHighEPS(yr+5). ForecastLow = one of {AvgLowP/E×EstLowEPS, avg low price 5y, recent severe low, dividend-supported = PresentDiv/HighYield}. **Zoning** Range=High−Low, default **25/50/25** (Buy/Hold/Sell). **U/D ratio=(High−Present)/(Present−Low)**, target **≥3:1**. 5y appreciation=(High/Present)×100−100.
- **§5 Return:** PresentYield=Div/Price×100; AvgYield=(AvgEPS5y×AvgPayout)/Price; **Total annual return**=(appreciation/5)+avgYield (simple→compound via table); compute via forecast High P/E and Average P/E.
- **Business thresholds:** future sales growth ≤15% w/o reason, never >20%; est EPS growth ≤ sales growth; discard if margins declining; debt ≤~30%; "GASP" → use lower forecast P/Es when historical P/Es abnormal. "Rule of Five" expectation (diversification rationale).

## Form → feature mapping
- SSG → **Stock Study**; Stock Comparison Guide → **Company Comparison** (~30 metrics, up to ~5 cos); Portfolio Management Guide → **Portfolio Tracker** (price/P/E zones over time, price-vs-P/E chart); PERT → **Portfolio Health Review** (quality/valuation roll-up + diversification); Stock Check List → **Quick Screen**.

## Data requirements & providers
- **Per company (~10y annual + current + estimates):** Sales, EPS(diluted), Pre-Tax Profit, Shares Outstanding, Book Value/Equity, Total Debt/Capital, yearly High/Low price, Present price (live/daily), Dividend/share, **analyst estimates (fwd Sales & EPS growth)**.
- **Provider strategy:** `MarketDataProvider` trait + incremental adapters. **Default = EODHD** (broad EU/CH/US/global, fundamentals + estimates). Alternatives/fallbacks: FMP (good estimates), Finnhub/Alpha Vantage (free US), Nasdaq/Sharadar (US deep), Twelve Data. Institutional (Refinitiv I/B/E/S, Zacks, FactSet) out of scope (enterprise).
- **Constraints:** user supplies own API key (OS keychain/local config, out of VCS); **no vendor data redistributed** in repo; EODHD free tier (20 calls/day) insufficient for active refresh → paid plan likely (state honestly).

## Technical context / preferences
- **Rust** core; **GUI = Slint** (Guy's existing expertise + cross-project reuse; strong for faithful form layouts). egui was the analytical-charting front-runner but **rejected in favor of Slint**; egui is the documented fallback only if Slint charting proves too costly.
- **Charting** = plotters or custom Slint canvas; must support draggable judgment lines + colored Buy/Hold/Sell zone bands + live recalc → **prototype first (main Slint risk)**.
- **Persistence** = embedded **SQLite via rusqlite** (bundled; binary runs anywhere); holds studies, cached financials, portfolios, judgment inputs; **local-first/offline**.
- **Calculation engine = UI-independent, unit-tested Rust crate** (reused by GUI + future CLI/export). Build it first (lowest risk, highest reuse).
- **Cross-platform** Win/Mac/Linux is a hard requirement (Guy develops on Linux).
- Architecture pillars: provider abstraction; local-first cache; pure calc crate; code/keys separation; cross-platform.

## UX constraints
- **High form fidelity is a hard constraint** (layout, sections, columns, field order, recognizable arrangement) — deviating too much confuses experienced users. Augmentation (charts/color/recalc) layers *on top of* the familiar structure.
- Augmentation features: interactive growth chart, draggable judgment lines, colored buy/hold/sell zones, real-time recalculation, signal color-coding (green/amber/red).

## IP / legal / compliance (reconciled)
- NAIC, BetterInvesting, Stock Selection Guide, Stock Comparison Guide, Portfolio Management Guide, PERT = **registered trademarks**; forms **copyrighted**. Methodology/formulas = **free to implement**.
- **Decision:** app is mainly personal → **OK to keep NAIC terminology as in-app labels**; keep labels in a **swappable i18n layer** (neutral set available — mapping in research doc). Product **name/logo stay neutral** ("steadyinvest"). **Forms kept visually close to originals** (functional layout not protectable); avoid only **logos + verbatim instructional prose + decorative elements**. Show **"independent, not affiliated; educational use, not investment advice"** disclaimer.
- **OSS license** undecided (MIT/Apache vs GPL) — interacts with crate licenses.

## Rejected / deferred (don't re-propose)
- egui as GUI (→ Slint chosen). · Intraday data as v1 hard requirement (→ EOD daily). · Community/social/member-sentiment/online-sharing features (→ personal app). · Morningstar as data source (not openly API-accessible at hobbyist tier; use EODHD/FMP etc.). · Bundling vendor data in repo (licensing forbids). · Full neutral-terminology-everywhere (→ keep NAIC labels, neutralize name/logo only).

## Open questions (for PRD/Architecture)
- Actual EODHD (and alternates) field coverage/quality for **smaller EU/CH names** — validate early; manual entry mitigates.
- **FX correctness:** which currency each financial figure is reported in; FX-rate source/timing for conversion & aggregation.
- **Methodology fit** for IFRS vs US-GAAP (pre-tax profit definitions) on non-US stocks.
- OSS license choice.
- Alert delivery mechanism (in-app vs desktop notification) and refresh scheduling model.

## Competitive intelligence (preserve)
- Incumbents: **CoreSSG/SSGPlus** (BetterInvesting, Morningstar-powered, web, subscription, 8000+ stocks); **Toolkit 6** (ICLUBcentral/Doug Gerlach, desktop, Windows-only, **sunset 2021**, `.ITK` files, offline); **ManifestInvesting** (PAR-based, since 2005).
- **Gap steadyinvest fills:** owned/offline/unlimited (TK6 strength) + analytical depth (SSGPlus) + **provider independence**, cross-platform.
- Table-stakes features (credibility w/ NAIC users): full 2-page SSG, quarterly (PERT-A), Visual Analysis graphs with **movable judgment lines**, Preferred Procedure Calculator, Judgment Audit, Stock Comparison Guide, Portfolio Management + PERT + diversification reports, `.ITK` import.
