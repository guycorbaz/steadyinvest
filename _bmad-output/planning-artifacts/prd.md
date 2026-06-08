---
stepsCompleted: [1, 2, "2b", "2c", 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, "step-12-complete"]
status: complete
completedDate: "2026-06-06"
releaseMode: phased
inputDocuments:
  - _bmad-output/planning-artifacts/product-brief-steadyinvest.md
  - _bmad-output/planning-artifacts/product-brief-steadyinvest-distillate.md
  - _bmad-output/planning-artifacts/research/domain-naic-better-investing-research-2026-06-05.md
  - docs/NAIC/SSGHandbook.pdf
  - docs/NAIC/SSGPlus_QuickStart.pdf
  - docs/NAIC/Stock Selection Guide Tutorial.pdf
  - docs/NAIC/A-Beginners-Tour-of-the-SSG-Jan-2015.pdf
  - docs/NAIC/BI_Member_Benefits.pdf
  - docs/NAIC/forms/Stock Selection Guide and Report.pdf
  - docs/NAIC/forms/stock selection guide.pdf
  - docs/NAIC/forms/Stock Comparison Guide.pdf
  - docs/NAIC/forms/Portfolio Management Guide.pdf
  - docs/NAIC/forms/stock checklist.pdf
documentCounts:
  briefCount: 1
  researchCount: 1
  brainstormingCount: 0
  projectDocsCount: 0
classification:
  projectType: desktop_app
  projectTypeNote: "data-integration is a significant sub-system in its own right"
  domain: fintech
  domainQualifier: "investment analysis, educational, non-regulated by design (neutral signals, footer disclaimer on every page, no personalized advice, no remuneration)"
  complexity: high
  topRisk: "integrity of the data->calculation chain producing a silent, plausible, wrong buy/sell signal"
  secondaryRisk: "Slint interactive charting (draggable judgment lines)"
  projectContext: "greenfield (code) on brownfield methodological + IP terrain"
discoveryDecisions:
  - "v1 target markets = CH/EU from the start (not US-first); manual entry is a first-class data path, not a fallback"
  - "Buy-zone alerts are stated as a neutral fact ('price entered the zone YOU defined'), never as a recommendation"
  - "Footer disclaimer on every page: steadyinvest does not replace a financial advisor / professional experience"
  - "Code lives in a PRIVATE GitHub repo initially (not a public release), for private use first, to avoid potential legal exposure; revisit IP/ToS posture before making it public. An OSS license is applied from the start even while private (license != distribution; stays 'license-ready' for eventual opening). License = GPL-3.0 (copyleft); compatible with usual Rust crate licenses (MIT/Apache-2.0 can be incorporated; Apache-2.0 is explicitly GPL-3.0-compatible); watch for any incompatibly-licensed dependency"
  - "Data model stores (value, source, provenance) per cell; provider refresh is a reconciliation, never an overwrite; manual override wins but provider value is preserved"
  - "Provider returns per-cell coverage (present/absent/partial); missing data is a normal state, highlighted visually in the forms"
  - "FX: SSG calculations stay in the security's native currency; convert only at the final valuation and portfolio-aggregation layers; FX rates stored as dated, source-aware data"
  - "Correctness oracle (feasible): golden fixtures from hand-computed + obtained filled SSGs + property-based tests; backtest 'stop ~2 years ago vs real market' is method-exploration only, out of v1 validation scope"
  - "No vendor market data in the repo; test fixtures are synthetic; provider-agnostic = no embedded provider ToS"
  - "Strategic reframe (accepted): product is a durable PERSONAL SYSTEM OF INVESTMENT DISCIPLINE with cumulative MEMORY of judgments (a revisitable journal), not merely a time-saving study tool. Design the data model 'journal-ready' (timestamped judgment snapshots) from the start, even if full history/versioning UI lands later"
  - "Differentiation: durable edge is CH/EU coverage incumbents ignore, but only if SSG is ADAPTED to EU reality (multi-currency, dividend taxation by jurisdiction, IFRS vs US-GAAP, exchange fragmentation), not merely translated. Faithful form clone = table stake, not the moat"
  - "UX: TWO REGIMES sharing one data truth - (1) faithful paper-form grid for contemplation/judgment, (2) high-throughput entry/reconciliation surface (keyboard nav, paste a column of years, undo) for filling CH/EU gaps. Provenance shown by ATTENTION HIERARCHY not equal signposting: missing = the only state that shouts, stale = a discreet uniform murmur, auto-vs-manual = revealed on demand; strong colors reserved for judgment zones, never for provenance"
  - "UX: judgment lines = gesture+value duality (drag for intuition, type exact value for rigor, always synced), reversible (moving a line never destroys a saved input; undo + scenario compare), never auto-moved on a provider refresh, and NEVER a 'suggested line' (suggesting = recommending = breaks neutral posture)"
  - "AI interaction (the 'touch of madness'): AI is a GREFFIER OF MEMORY, not an advisor of the future. It makes PROPOSALS and CRITIQUES (coherence checks, discipline-drift detection, pre-mortem on past misses, fill candidates for missing/stale cells) but NEVER mutates data or judgments. Read-only on journal+engine; any AI output is a draft stamped source:ai-suggested requiring human validation in the UI (a 4th cell source alongside provider/manual/derived). AI must never suggest/place a judgment line (the future is the user's sovereign territory). Purity test for any API endpoint: if an AI response could replace the user's gesture on the line, it is forbidden; if it can only force him to look at his past gesture, it is allowed"
  - "AI runs 100% LOCALLY by default (consistent with offline/private), via a pluggable 'AI provider' abstraction (same pattern as MarketDataProvider) that allows an optional REMOTE AI later"
  - "Portfolio RISK MANAGEMENT is a deliberate overlay BEYOND canonical NAIC (NAIC is strong on stock selection, weak on portfolio risk). Keep it as a SEPARATE, optional, decoupled subsystem so it never weighs down the pure SSG engine. Includes: (a) position-sizing / concentration limits (avoid one holding being a majority of invested capital; thresholds must account for early-stage portfolios being naturally concentrated; extends diversification by holding/sector/size/currency, NAIC-aligned), and (b) trailing stop-loss protection"
  - "Trailing stop-loss model = portfolio 'capital at risk' / portfolio heat. Per position, risk = (purchase_price - stop) * quantity, counted ONLY when stop <= purchase_price; once the trailing stop ratchets above purchase price the capital-loss risk disappears. Portfolio risk of loss = sum of those positive differences, converted to portfolio reference currency (valuation/aggregation layer => FX applies), expressible as % of total invested capital. Show TWO views: capital-at-risk (vs purchase price) and open-profit-at-risk (vs current price). Trailing stop ratchets UP automatically (param: %/ATR/manual) and NEVER down; the parameter is the user's judgment. App does NOT execute orders (no brokerage, out of scope) - a stop breach is a NEUTRAL ALERT ('position X crossed the stop YOU set'), same posture as buy-zone alerts"
  - "Design tension to SURFACE (never auto-resolve): SSG sell discipline (sell in Sell zone or on quality degradation) can conflict with stop-loss sell discipline (sell on trailing-stop breach). A quality holding may hit its stop in a market dip while still a great long-term hold in its Buy zone. Show both as neutral facts; the user arbitrates between long-term-quality vs capital-protection philosophies. USER'S ARBITRATION RULE (Guy's default policy): the STOP-LOSS takes priority over the Sell zone because the stop mitigates the REAL (capital) risk while the Sell zone is only a theoretical valuation marker. When a holding clearly enters the Sell zone, it is a DECISION TRIGGER (not an order): Guy either sells manually OR raises the stop-loss. UX implication: a Sell-zone entry is a neutral alert that may offer the two manual actions (sell / adjust stop) but never auto-acts"
  - "API posture: NO API/server in v1. Build a VERSIONED DATA CONTRACT now - calc crate + judgment-journal exposed as clean serde types with explicit schema_version, decoupled from Slint and rusqlite - to preserve optionality at near-zero cost. Versions: version the DATA CONTRACT now (cumulative journal must last years), defer transport SemVer. First façade when needed = read-only MCP. Capability asymmetry (AI read-only, all writes via human UI validation) is enforced by construction, not by prompt, and is a NON-NEGOTIABLE NFR"
  - "NO onboarding wizard: multiple providers are usable and some need NO API key, so configuration is done as-needed in Settings (provider choice, optional API key in OS keychain, reference currency, risk thresholds, label set NAIC<->neutral), not via a forced first-run wizard"
  - "Per-cell/per-study user-set 'validated' flag (e.g. a checkbox) that the USER turns on AFTER reviewing the data. This is the primary guard against 'present-but-wrong' data (Murat's top risk): a human verification flag, not auto-plausibility. Data dimensions: source (provider/manual/derived) + freshness (current/stale/to-update) + validated (yes/no, user-set)"
  - "Backup/restore is delegated to an EXTERNAL system (e.g. NAS sync), manual or automatic. App responsibility = keep the journal as a single, well-located, copy-friendly local file (and document its location); it does not build elaborate in-app backup in v1"
  - "FX rule (confirmed): every study and calculation runs in the security's NATIVE currency; FX consolidation happens only at the END, at the portfolio/valuation layer, on data processed in original currency; FX rates are dated/source-aware and refreshed manually"
  - "Quick Screen / Starter Checklist = ROADMAP (not v1), to keep v1 focused"
  - "Missing data may be PERMANENT (data genuinely does not exist), not just 'to fill'. Distinguish a 'not available - accepted' state (assumed permanent gap) from 'to fill'. The engine computes on available data, BUT a study needs at least 5 YEARS of usable history to be meaningful: below 5 years -> WARN and mark the study 'insufficient history / low confidence', surfaced in the verdict (do NOT hard-block; the expert user can proceed, clearly labelled)"
  - "MULTI-PORTFOLIO (user holds positions at more than one bank). Portfolios (one per bank/account) are first-class. Risk/concentration/capital-at-risk computed PER PORTFOLIO and CONSOLIDATED across total capital (concentration 'no holding is a majority of invested capital' is inherently across all banks). SCOPING UPDATE (step 8): the leaner MVP (Phase 1) ships a SINGLE portfolio, single reference currency, with a SIMPLE capital-at-risk; full multi-portfolio + multi-currency/FX consolidation + full transaction ledger + complete risk overlay move to PHASE 2. Deeper multi-portfolio analytics remain later"
  - "Annual update of an existing study is a recurring v1 journey: on a new annual report, re-fetch + reconcile (manual wins, provider value preserved) a previously saved study"
  - "TRANSACTION LEDGER: portfolio positions are built from a buy/sell transaction journal (partial sells included). Each transaction = {date, type buy/sell, quantity, unit execution price, fees, currency}. Cost basis = weighted-average (derived from transactions); the per-transaction execution price is also stored"
  - "CAPITAL-AT-RISK consolidation hierarchy: computed PER CURRENCY in native currency (no FX mixing), then consolidated PER BANK and as a GLOBAL TOTAL, all expressed in ONE single global app reference currency. FX is applied ONLY at these consolidation points (per-currency buckets stay FX-free)"
  - "DIVIDENDS: the SSG/study return projection (Section 5) uses the GROSS dividend (method fidelity). The portfolio's reinvestable cash uses the NET dividend (gross minus withholding; Swiss pattern = 35% impot anticipe, refundable at tax declaration; withholding rate is per-jurisdiction). v1 simply reduces reinvestable cash to net; tracking the withholding as a recoverable receivable / refund = ROADMAP"
  - "Locale-aware number entry (decimal comma, thousands separators for CH/EU) to prevent input errors; decision RATIONALE (the 'why' of a buy/sell) is a first-class field of the journal (fuel for cumulative memory and the AI-greffier)"
  - "REPLACEMENT / capital-redeployment workflow: selling a holding is not an end - it triggers finding what to replace it with (freed cash should be redeployed, NAIC-aligned 'sell only for a better opportunity, stay invested'). On a sell / stop-trigger / Sell-zone entry, the app surfaces replacement candidates from the watchlist (e.g. nearest to / inside their Buy zone, best upside/downside), supports side-by-side Company Comparison, and launches a new Stock Study. The replacement flow must respect portfolio rules (preserve diversification, not re-concentrate by sector/currency; respect capital-at-risk). Neutral posture: surfaces candidates & comparisons (facts), never says 'buy this one'. Cumulative memory records 'sold X -> replaced by Y, date, rationale' for the AI-greffier to later interrogate replacement quality"
workflowType: 'prd'
---

# Product Requirements Document - steadyinvest

**Author:** Guy
**Date:** 2026-06-06

## Reference Documents

Source NAIC / BetterInvesting methodology documents (in [`docs/NAIC/`](../../docs/NAIC/)):

- [SSG Handbook](../../docs/NAIC/SSGHandbook.pdf)
- [SSGPlus Quick Start](../../docs/NAIC/SSGPlus_QuickStart.pdf)
- [Stock Selection Guide Tutorial](../../docs/NAIC/Stock%20Selection%20Guide%20Tutorial.pdf)
- [A Beginner's Tour of the SSG (Jan 2015)](../../docs/NAIC/A-Beginners-Tour-of-the-SSG-Jan-2015.pdf)
- [BetterInvesting Member Benefits](../../docs/NAIC/BI_Member_Benefits.pdf)
- Official forms:
  - [Stock Selection Guide and Report](../../docs/NAIC/forms/Stock%20Selection%20Guide%20and%20Report.pdf)
  - [Stock Selection Guide](../../docs/NAIC/forms/stock%20selection%20guide.pdf)
  - [Stock Comparison Guide](../../docs/NAIC/forms/Stock%20Comparison%20Guide.pdf)
  - [Portfolio Management Guide](../../docs/NAIC/forms/Portfolio%20Management%20Guide.pdf)
  - [Stock Check List](../../docs/NAIC/forms/stock%20checklist.pdf)

## Executive Summary

steadyinvest helps a single self-directed investor decide — on his own — whether to **buy, hold,
or sell** a stock, and **remember why he believed it**. It is an independent, offline-first
desktop application (Rust + Slint, local SQLite; GPL-3.0 intended, private repository for now)
that faithfully replicates the *method* of the NAIC/BetterInvesting Stock Selection Guide (SSG) —
the formulas, which are not protectable — while using **neutral terminology and original layouts**
(NAIC names and form designs are not copied). On top of the method it layers an interactive
analytical experience: live recalculation, draggable judgment lines, and color-coded
buy/hold/sell zones.

The product is **not merely a faster way to complete a stock study**. It is a durable personal
system of investment discipline with a **cumulative memory of judgments** — a sovereign,
revisitable journal of *why* each decision was made, that the user can confront against real
outcomes over time. It is framed around the full investing loop — *discover, study, buy, watch
portfolio risk, protect, exit, redeploy* — with every step recorded. **v1 delivers the core of
that loop** (see scope below); discovery/screening and replacement/rotation are roadmap.

**The problem.** Rigorous fundamental analysis works but is tedious, and existing tooling forces
a bad trade: subscription web tools (CoreSSG/SSGPlus) are online-only, membership-gated, and
locked to a single data vendor; the only true desktop tool, NAIC's Toolkit 6, is Windows-only and
discontinued (no new sales since 2021, per BetterInvesting/ICLUBcentral); spreadsheets are slow
and error-prone. None serves an investor trading across CH/EU/US markets with an owned, offline,
vendor-agnostic tool — and none preserves the cumulative record of judgment that builds
independent conviction.

**Target users.** Primary, and for now the only one: the author — an experienced individual
investor across European, Swiss and US markets. Designing for a single real user is a
**deliberate strength**: it removes the costliest design compromise — pleasing strangers — and
lets discipline dictate every trade-off. Secondary, later (only after revisiting IP/data-licensing
posture before any public release): other self-directed individual investors. v1 targets CH/EU
markets from the start, **where ≥10 years of fundamentals are available** (coverage depth is a
known dependency to validate against the chosen data provider).

### What Makes This Special

- **CH/EU coverage, adapted — not translated:** multi-currency (CHF/EUR/USD with FX), dividend
  treatment, IFRS vs US-GAAP, exchange fragmentation. The durable edge incumbents ignore. Patchy
  provider coverage is a *normal state*, handled by **first-class manual entry** and per-cell
  source/provenance. Form fidelity is table-stakes, not the moat.
- **Sovereignty and cumulative memory:** offline, private, owned, GPL-3.0; no subscription, no
  vendor lock-in (the user brings their own API key — default adapter EODHD, but the provider
  layer is agnostic; no vendor data is redistributed). A study is an event; the **time-series of
  judgments is an appreciating asset**.
- **Risk guardian beyond NAIC (v1 differentiator):** NAIC is strong on selection, weak on
  portfolio risk. An optional, decoupled overlay adds **concentration limits** and a
  **trailing-stop "capital-at-risk" model** — portfolio risk of loss = the summed gap between
  purchase price and stop while the stop sits at or below cost. The stop-loss takes priority over
  the theoretical Sell zone because it mitigates the *real* (capital) risk.
- **Neutral by design:** the app surfaces facts, never recommendations — "the price entered the
  zone *you* defined" — with a footer on every page stating it does not replace a financial
  advisor, and the user as sole decider. This is a *design intent* to remain educational and
  outside regulated advice — not a legal opinion.

**Core insight.** The value is not saved time — it is **forged, owned, durable conviction**.
Discipline comes from friction (the user draws the judgment lines himself); independence comes
from owning both the process and its record.

**What's next (vision, post-v1).** An optional, **local-first AI "clerk of memory"** that makes
proposals and critiques by interrogating the *past* (coherence checks, discipline-drift
detection, pre-mortems) — never mutating data or judgments, never touching the future. v1 does
not build it; v1 only builds the **versioned data contract** that keeps it (and screening,
rotation, Company Comparison, Portfolio Health Review) cheap to add later.

**Definition of success (primary).** The author uses steadyinvest for **every** buy/hold/sell
decision over a sustained period and can, at any later date, **replay the full reasoning** behind
each past decision — with calculations he trusts as correct.

### Scope (v1)

**In:** faithful Stock Study (auto-fetch + first-class manual entry, per-cell provenance);
interactive growth/valuation chart (draggable judgment lines, colored zones, live recalc); study
calculations in native currency; watchlist with neutral buy-zone alerts; a **single-portfolio,
single-currency** holdings register with a **simple capital-at-risk**; local SQLite with a
journal-ready, versioned data model. (Full multi-portfolio, multi-currency/FX consolidation, the
transaction ledger and the complete risk overlay are **Phase 2**.)

**Out (non-goals):** brokerage / order execution; regulated or personalized investment advice;
real-time intraday data (daily EOD suffices); multi-user / accounts / cloud sync; public
distribution (private repo until IP/data-licensing posture is revisited). Roadmap (not v1):
multi-portfolio & multi-currency/FX consolidation, the full transaction ledger and complete risk
overlay, discovery/screening, replacement/rotation, Company Comparison, Portfolio Health Review,
the AI clerk-of-memory, legacy study import, export/share.

## Project Classification

- **Project type:** Desktop application (Rust + Slint, cross-platform Windows/macOS/Linux,
  offline-first, local SQLite). Data integration is a significant sub-system in its own right.
- **Domain:** Fintech — investment analysis; designed to remain **educational and non-regulated**
  (neutral signals, no personalized advice, no remuneration, no brokerage/money movement). Design
  intent, not a legal opinion.
- **Complexity:** High. **Top risk:** integrity of the data→calculation chain producing a *silent,
  plausible, wrong* buy/sell signal — mitigated by a deterministic, unit-tested calculation crate
  with golden-fixture and property-based tests, plus per-cell provenance and input plausibility
  checks. Secondary risk: interactive Slint charting (draggable judgment lines).
- **Project context:** Greenfield (code) on **brownfield methodological + IP terrain** (a
  70-year documented method to reproduce faithfully; NAIC names and form layouts are protected
  expression to neutralize).
- **Licensing posture:** GPL-3.0 *intended*, subject to a dependency-license audit (notably
  Slint's licensing tier and the one-way GPL-3.0 ↔ Apache-2.0 compatibility across the Rust crate
  tree); private repo initially, no public distribution until IP/data-licensing posture is
  revisited.

## Success Criteria

### User Success

- Completes a **trustworthy Stock Study in a few minutes** when provider coverage is good — and
  remains **fully able to complete it by manual entry** when coverage is partial (missing data
  is never a hard blocker).
- Every study yields **correct buy/hold/sell zoning, upside/downside ratio, and a 5-year return
  projection**; quality flags (declining margins, high debt, weak ROE) surface automatically.
- Can, at **any later date, replay the full reasoning** behind a past decision — judgment lines,
  inputs, per-cell provenance, and notes — confronting it against real outcomes.
- Sees the portfolio's **capital-at-risk at a glance**, and receives **neutral alerts** when a
  holding enters its buy zone or crosses the stop the user set.
- "Aha" moments: dragging a judgment line and watching the zones **recolor live**; reopening a
  two-year-old study and seeing **exactly why** the decision was made.

### Project Success (personal, non-commercial)

- The author **adopts steadyinvest as his sole tool** for buy/hold/sell decisions over a
  sustained period (≥ 6–12 months), replacing spreadsheets and the prior tooling.
- **Sustainable & maintainable:** a thin UI over a tested calculation core, so the tool survives
  GUI-framework churn and stays cheap to keep current.
- **Open-source-ready:** GPL-3.0 applied; structured (neutral labels, synthetic fixtures, no
  vendor data) so it could be made public later with minimal rework, after the IP/licensing review.
- Explicitly **no** revenue, user-growth, or engagement targets — out of scope for a personal tool.

### Technical Success

- **Deterministic calculation engine** matches reference SSGs to a defined numeric tolerance:
  100% of golden-fixture studies pass; property-based invariants hold; tests green in CI.
- **FX correctness:** calculations in native currency; conversion only at the valuation/portfolio
  layer; round-trip (A→B→A) idempotent within epsilon.
- **Full offline operation** after fetch; every data point persisted with source + provenance +
  timestamp (per cell).
- **Interactive chart:** drag + live recalculation with imperceptible latency (target < ~100 ms);
  judgment lines support gesture **and** exact-value entry, fully reversible (undo).
- **Versioned data contract** (`schema_version`) decoupled from Slint and SQLite.
- **Cross-platform** builds run on Windows, macOS, and Linux.

### Measurable Outcomes

- Stock Study completion: **< ~5 minutes** with good coverage (vs. hours by hand).
- Engine: **100%** of golden-fixture studies match to tolerance; property tests green.
- Manual path: a study with **0% provider coverage is still completable end-to-end**.
- Offline: a full study **and** portfolio risk review complete with networking disabled.
- Risk: the portfolio **capital-at-risk figure is visible and recomputed on every price refresh**.
- Adoption: author uses it for **100% of decisions** over the trial period and can **replay every
  one**.

## Product Scope

### MVP — Minimum Viable Product

Faithful **Stock Study** (auto-fetch + first-class manual entry, per-cell provenance);
**interactive growth/valuation chart** (draggable judgment lines, colored zones, live recalc);
study **calculations in native currency**; **watchlist + neutral buy-zone alerts**; a
**single-portfolio, single-currency holdings register** with a **simple capital-at-risk**;
**local SQLite** with a journal-ready, versioned data model; **deterministic, tested calculation
engine**; neutral-by-design posture (footer disclaimer, facts not recommendations). *(Full
multi-portfolio, multi-currency/FX, transaction ledger and complete risk overlay = Phase 2.)*

### Growth Features (Post-MVP)

**Multi-portfolio (one per bank) + multi-currency/FX consolidation**; **full transaction ledger**
(partial sells, weighted-average cost basis, fees) **+ complete risk overlay** (concentration +
trailing-stop capital-at-risk per currency → per bank → global); **dividends** (gross in study /
net reinvestable); **Company Comparison**; **Portfolio Health Review** (diversification & quality
roll-up); **discovery/screening** + **Quick Screen**; full **replacement/rotation** flow; additional
data-provider adapters; PDF/print export of a study; optional OS-native notifications.

### Vision (Future)

Optional **local-first AI "clerk of memory"** (proposals & critiques on the past, never
recommending) with a later **remote-AI** option; **legacy study import** (.ITK / SSGPlus);
deeper multi-portfolio risk analytics; export/share; eventual **public open-source release**
after IP/data-licensing review.

## User Journeys

**Persona — "Guy", the self-directed investor.** Experienced across CH/EU/US markets, knows the
SSG method by heart, has done studies by hand in spreadsheets for years. Wants a fast,
independent, offline tool he fully owns — that never tells him what to do, but holds the memory
of why he decided what he decided. He is the primary and, in v1, the only user.

**Setup & configuration (no wizard).** Because several data providers are usable — and some need
no API key — there is no forced onboarding wizard. Configuration is done as needed in **Settings**:
choose a provider and (optionally) store its API key in the OS keychain, set the single global
reference currency for consolidation, define risk thresholds (concentration, trailing-stop
parameters), pick the label set (NAIC ↔ neutral, a swappable layer), and the locale number format.
Guy can start a study immediately and configure only what he needs, when he needs it.

### Journey 1 — A new Stock Study, good coverage (happy path)

Guy reads about a US large-cap and wants to know if it's a quality business at a fair price. He
creates a new Study, types the ticker; the app auto-fetches ~10 years of fundamentals and prices
from his provider, pre-filling the familiar grid. He scans the growth trend, drags the future
sales/EPS growth lines — the zones recolor **live** — checks the upside/downside ratio and the
projected return, notes the present price sits in the Buy zone. Quality flags are quiet (margins
stable, debt low). In a few minutes he has a trustworthy verdict and a saved study with his
reasoning attached. *New reality:* the hours-long spreadsheet chore is now a few-minute,
judgment-supported decision.
**Reveals:** Study creation, provider auto-fetch, faithful editable grid, growth/valuation charts
with draggable judgment lines + live recalc, zoning/U-D/return engine (in native currency),
quality-flag rules, save.

### Journey 2 — A CH/EU small-cap, partial coverage + validation (edge case)

Guy studies a Swiss mid-cap. The provider returns only 7 of 10 years; three cells are **visibly
empty**. Some he can fill from the annual report; for two early years the data **simply doesn't
exist**, so he marks them **"not available — accepted"** (a permanent gap, distinct from
"to fill"). He pastes the figures he has, fixes a thousands/millions mismatch; the cells now carry
a "manual" provenance distinct from the auto-fetched ones. Knowing his top risk is
*plausible-but-wrong* data, he reviews each figure and ticks the per-cell **"validated"** checkbox —
his explicit human sign-off. The study still has **8 usable years (≥ the 5-year floor)**, so it runs
normally in CHF (native currency); had it dropped below five, the app would compute on what's there
but **label the study "insufficient history / low confidence"** and carry that into the verdict.
*New reality:* gaps — temporary or permanent — are a normal, honest state, and a thin history is
flagged, never silently trusted.
**Reveals:** per-cell coverage (present / to-fill / **not-available-accepted**) with gap
highlighting; first-class manual entry (keyboard nav, paste, undo); per-cell source/provenance;
**user-set "validated" flag**; **5-year minimum → warn + low-confidence label**; native-currency
calculation; reconciliation (manual wins, provider preserved).

### Journey 2b — Annual update of an existing study (recurring)

A year later the company's annual report lands. Guy reopens the saved study and triggers a
re-fetch; the app **reconciles** new provider data against what's there — manual entries and his
judgment lines are preserved, the "validated" flags on changed cells reset so he re-checks what
actually moved. He extends the projection, the zones recompute, and the study's history shows what
changed and when. *New reality:* keeping a study current is a quick, safe annual ritual, not a
rebuild from scratch.
**Reveals:** reopen + re-fetch, reconciliation rules (manual/judgment preserved, validated-flag
reset on change), study update over time, change visibility.

### Journey 3 — Watching portfolio risk across banks (manual refresh, multi-portfolio)

Guy holds positions at **two banks** — two portfolios in the app, some accounts in different
currencies. When he chooses to, he triggers a **manual refresh**. Each holding's zone recomputes; a
watchlist candidate has **entered its Buy zone** (neutral alert). He sees **capital-at-risk per
currency** (each computed in its native currency), **consolidated per bank**, and a **global total
in his single reference currency** (FX applied only at consolidation). A **concentration** check on
the **total** invested capital warns that one holding is approaching a majority share — regardless
of which bank or currency holds it. One position's trailing stop has ratcheted above its weighted-
average cost: its capital risk is now zero. *New reality:* one honest picture of risk across all his
banks and currencies, on data he refreshed on purpose.
**Reveals:** multi-portfolio holdings register (one per bank/account, multi-currency), manual price
refresh with displayed freshness, per-holding zone recompute, trailing-stop capital-at-risk
**per currency → per bank → global total** (FX only at consolidation), concentration on **total**
capital, neutral buy-zone & stop alerts.

### Journey 3b — When a provider fails (error path)

Guy triggers a refresh but his provider is down (outage, rate limit, or expired API key). The app
does not break or block: the affected data is **flagged "not up-to-date / to update"** (a discreet
stale marker, never colliding with the buy/hold/sell zone colors), last-known values remain in
place, and he can keep working offline or fill/override by hand. A clear message names the cause
(network / quota / key) and he retries later. *New reality:* a data outage degrades gracefully into
a visible, honest "stale" state — it never produces a silent wrong signal.
**Reveals:** graceful provider-failure handling, stale/to-update flagging with timestamps,
last-known-value retention, actionable error messaging (network/quota/key), offline continuity,
manual override as the always-available fallback.

### Journey 4 — A sell signal, and what replaces it

A holding crosses into the Sell zone. The app surfaces it as a fact and offers two manual actions
— **sell**, or **raise the stop-loss** — never deciding for him (the stop takes priority over the
theoretical Sell zone). Guy chooses to sell (a sell transaction, possibly partial). Freed capital
shouldn't sit idle: the app surfaces replacement candidates from his watchlist (nearest to /
inside their Buy zone, best U-D), flags any that would re-concentrate a sector or currency, and
lets him open a fresh Study on the best one. *New reality:* selling triggers disciplined
redeployment, not a vague "what now?".
**Reveals:** Sell-zone / stop-breach neutral triggers with manual actions, stop-priority rule,
partial sells, replacement-candidate surfacing from watchlist, concentration/diversification
checks, hand-off into a new Study; the "sold X → replaced by Y, date, rationale" record.
*(Replacement/rotation flow is roadmap; the trigger + manual sell/raise-stop is v1.)*

### Journey 5 — Confronting a past judgment (cumulative memory)

Two years on, the Swiss mid-cap has roughly doubled. Guy reopens the original study. There are his
judgment lines, his inputs with their provenance and validation, his decision rationale and notes —
exactly the reasoning he committed to back then. He compares his projected zone to what actually
happened, and learns something about his own optimism. *New reality:* his studies are not throwaway
events but a journal that makes him a better investor over time.
**Reveals:** durable, reopenable studies; journal-ready versioned data model preserving judgment +
provenance + validation + rationale + notes over time; before/after comparison against real
outcomes.

### Journey 6 — The clerk of memory (vision, post-v1)

Later, an optional local AI reads (only reads) his journal. As he drags an optimistic growth line,
it asks: "In 2024 you set this same slope on three names; two disappointed. Still confident?" It
proposes, critiques, and interrogates his past — but never moves the line, never recommends a buy.
*New reality:* his own history holds him accountable.
**Reveals (future):** read-only AI provider over the versioned contract, ai-suggested drafts
requiring human validation, coherence / discipline-drift / pre-mortem prompts. *(Not v1.)*

### Journey Requirements Summary

- **Stock Study engine & forms:** creation, faithful editable grid, deterministic SSG engine in
  native currency (zoning, U-D, projection using **gross** dividends, quality flags),
  save/reopen/update.
- **Data layer:** provider auto-fetch (agnostic; some providers keyless, key optional in OS
  keychain); per-cell coverage (present / to-fill / not-available-accepted) with gap highlighting;
  first-class manual-entry surface; user-set **"validated"** flag (reset on change);
  **5-year minimum → warn + low-confidence label**; reconciliation (manual wins, provider preserved);
  native-currency calc, FX consolidated only at the portfolio layer (dated FX).
- **Charts/interaction:** growth/valuation charts, draggable judgment lines (gesture + exact value),
  live recalc, colored zones, undo.
- **Watchlist & alerts:** watchlist, neutral buy-zone & stop alerts.
- **Transactions & cost basis:** buy/sell **transaction ledger** (partial sells), per-transaction
  {date, type, quantity, unit execution price, fees, currency}; **weighted-average cost basis**.
- **Dividends:** **gross** in the study (Section 5); **net** for the portfolio's reinvestable cash
  (per-jurisdiction withholding; CH 35%); withholding-refund tracking = roadmap.
- **Portfolio risk:** **multi-portfolio holdings register (one per bank, multi-currency)**; manual
  price refresh with displayed freshness; per-holding zones; trailing-stop **capital-at-risk per
  currency (native) → per-bank consolidation → global total in the single reference currency** (FX
  only at consolidation); concentration on **total** capital; graceful provider-failure handling
  (stale / to-update flagging, never a silent wrong signal).
- **Sell & replace:** neutral sell/stop triggers with manual actions (sell / raise stop), stop-
  priority rule, replacement-candidate surfacing (roadmap), decision record.
- **Cumulative memory:** durable studies, journal-ready versioned data model, before/after replay.
- **Configuration:** Settings (no wizard) — provider/key, **single global reference currency**,
  risk thresholds, swappable label set, locale number format.
- **Data safety:** journal kept as a single copy-friendly local file; backup/restore delegated to an
  external system (e.g. NAS sync), manual or automatic.
- **Journal:** durable studies; transactions; per-cell provenance + validated flag; **decision
  rationale (first-class)**; notes; versioned, journal-ready data model.
- **Neutral posture (cross-cutting):** facts not recommendations, footer disclaimer, user sole
  decider.
- **Future / roadmap:** read-only local-first AI; Quick Screen / Starter Checklist; replacement
  rotation; Company Comparison; Portfolio Health Review; legacy import; export/share.

## Domain-Specific Requirements

### Regulatory posture — non-regulated *by design*

- **Not a regulated financial service.** No brokerage/order execution, no money movement, no
  custody, no KYC/AML, no PCI-DSS — none apply, because the app neither holds assets nor moves
  funds. Standard fintech compliance is therefore **out of scope** by construction.
- **Boundary to maintain (the design line):** the app surfaces **facts, never personalized
  recommendations** ("the price entered the zone *you* defined"); no remuneration; a footer on
  every page states it is educational and **does not replace a financial advisor**.
- **Jurisdictional note:** advice-regulation triggers differ — CH (FINMA / LSFin), EU (MiFID II),
  US (Investment Advisers Act) — but all hinge on *personalized advice for remuneration*, which is
  out of scope. This is a **design intent, not a legal opinion**; revisit before any public release.

### Methodology fidelity & calculation integrity (the real "compliance")

- The **deterministic SSG engine must match the canonical method** exactly: golden-fixture tests
  (hand-computed + obtained filled SSGs) + property-based invariants; the **top domain risk** is a
  *silent, plausible, wrong* buy/sell signal.
- **5-year minimum** of usable history → below it, compute but **flag "insufficient history /
  low confidence"**; per-cell **user-set "validated"** flag as the human guard against
  present-but-wrong data; sanity checks on inputs.
- **Multi-currency / FX correctness:** calculations in the security's **native currency**;
  FX only at the consolidation layer; **capital-at-risk per currency → per bank → global total**.

### Data licensing & provider terms (the genuine legal dragon)

- **No vendor data in the repository**; test fixtures are **synthetic**; the provider layer is
  **agnostic**; the user brings **their own API key** (stored in the OS keychain).
- ToS distinction: not redistributing covers *distribution*, but **usage/retention terms remain
  the end-user's responsibility** and a per-provider check (some ToS restrict durable storage —
  which the cumulative-memory model relies on).
- Private repo for now; **no public distribution** until this posture is revisited.

### Intellectual property / trademark

- The **method (formulas, ratios, the analytical logic) is not protectable** — implemented freely.
- **Protected expression to avoid:** the marks (NAIC, BetterInvesting, Stock Selection Guide,
  PERT…) as product/feature labels, the copyrighted **form layouts**, verbatim instructional prose,
  and logos → **neutral labels + original visual design**, kept in a **swappable label layer**.
- An **"independent project, not affiliated"** notice; optional **IP-lawyer review before any
  public release**. License = **GPL-3.0** (subject to a dependency-license audit).

### Security & privacy

- **API keys in the OS keychain**, never in version control.
- **Local-first, single-user, offline**: no cloud, no accounts, no third-party PII → minimal
  privacy surface; the only sensitive material is the user's own keys and journal.
- **Data safety:** the journal is a single, copy-friendly local file; backup/restore delegated to
  an external system (e.g. NAS sync).

### Audit & traceability (native to the journal)

- Per-cell **source + provenance + validated + timestamp**, **decision rationale**, and a
  **versioned data contract** give a natural, durable audit trail of how each judgment was formed —
  serving both the cumulative memory and any future read-only AI.

### Domain risk mitigations (summary)

| Risk | Mitigation |
|---|---|
| Silent wrong buy/sell signal | tested deterministic engine + golden/property tests; validated flags; stale flagging; 5-year floor |
| FX noise in trend/quality | native-currency calculation; FX only at consolidation |
| Provider coverage gaps (CH/EU) | first-class manual entry; accepted-permanent-gap state |
| IP / trademark exposure | neutral labels + original design; private repo; pre-release legal review |
| Data-licensing / ToS | no vendor data shipped; user's own key; per-provider ToS is the user's responsibility |
| Loss of irreplaceable journal | external backup (NAS sync); single copy-friendly file |

## Innovation & Novel Patterns

### Detected Innovation Areas

- **The AI "clerk of memory" — inverting the AI-advisor pattern (Desktop AI signal).** Instead of
  an AI that predicts or recommends, a **local-first, read-only** AI that interrogates the user's
  **past** judgments (coherence checks, discipline-drift detection, pre-mortems) and is
  **architecturally forbidden** from recommending or touching the future (capability asymmetry, not
  a prompt rule). Novel because it *hardens* discipline instead of outsourcing it — the opposite of
  every "AI stock advisor".
- **Cumulative judgment memory as a first-class asset.** Turning a stock-study tool into a
  **longitudinal journal of conviction** — the time-series of judgments, provenance, validation and
  rationale, revisitable and confrontable against real outcomes. Incumbents monetize the *recurring
  study*; none preserve the *memory* that makes the user independent.
- **A risk overlay grafted onto a quality-growth method.** Combining NAIC's *selection* discipline
  with a trend-following **capital-at-risk model** (trailing-stop "portfolio heat",
  per-currency → per-bank → global), with the **stop prioritized over the theoretical Sell zone**.
  A cross-discipline marriage NAIC itself does not do.
- **Source-aware, validation-first data model.** Per-cell **(value, source, provenance, validated,
  freshness)** with **missing-as-a-normal-state** and **first-class manual entry** — treating patchy
  CH/EU coverage as expected, not exceptional. Unusual for an SSG-lineage tool.

### Market Context & Competitive Landscape

Per the domain research (June 2026): incumbents are **CoreSSG / SSGPlus** (web, Morningstar-locked,
subscription) and **Toolkit 6** (Windows-only, discontinued). **None** combines offline + owned +
provider-agnostic + **CH/EU-adapted** + **cumulative judgment memory** + **local read-only AI**.
That intersection is the empty quadrant steadyinvest occupies — not a faithful clone (table-stakes),
but the *memory + risk + neutral-AI* layer on top.

### Validation Approach

- **AI posture:** validate the read-only / never-recommends boundary by **capability-asymmetry
  tests** (the AI surface exposes no write/act path); usefulness validated by whether past-
  interrogation prompts actually change the user's behavior.
- **Capital-at-risk model:** validate the per-currency → per-bank → global math with hand-computed
  cases (independent of the SSG engine).
- **Cumulative memory:** validated in practice by reopening old studies (Journey 5); the "stop ~2y
  ago vs real market" backtest stays **method-exploration**, out of v1 validation.
- **Signature interaction (draggable judgment lines on Slint):** prototype **early** — it is the
  principal technical unknown.

### Risk Mitigation

- **AI scope creep / posture erosion** → capability asymmetry enforced by construction; AI is
  post-v1, and v1 only builds the versioned contract it will sit on.
- **Slint interactive-charting maturity** → week-1 spike; **egui** kept as an explicit fallback.
- **Over-engineering the journal** → ship the versioned data contract now, defer the AI/analytics
  features.

## Desktop Application — Specific Requirements

### Project-Type Overview

A single-user, **offline-first** native desktop application (Rust + Slint) for Windows, macOS and
Linux, owning all data locally. No web/SEO and no mobile concerns apply. The "System automation"
innovation signal is **not** pursued in v1 (the only system touchpoints are the keychain, the file
system, and locale).

### Platform Support

- **Windows, macOS, Linux** from a single Rust + Slint codebase; native rendering.
- The **interactive charting** (draggable judgment lines + zone bands) is the principal
  cross-platform technical unknown → **spike early**; **egui** kept as an explicit fallback.

### System Integration

- **OS keychain** for provider API keys (never in version control); keys are optional (some
  providers are keyless).
- **File system:** the journal is a **single SQLite file** at a documented, user-known location,
  **copy-friendly** so an external system (e.g. NAS sync) can back it up.
- **Locale:** number formatting (decimal comma, thousands separators) follows OS locale and is
  configurable — preventing manual-entry errors.
- **Notifications:** **in-app only** in v1 (alerts surface on manual refresh); OS-native
  notifications are a later nice-to-have.

### Update Strategy

- **No auto-update in v1.** Updates are manual (`git pull` + rebuild, or replacing the binary); a
  simple "check GitHub releases" link may come later.
- Because updates are manual but the **cumulative journal must survive them**, the local store
  carries a `schema_version` and the app performs **forward-safe schema migrations** on version
  bumps — the versioned data contract guarantees old journals open in new builds.

### Offline Capabilities

- **Full offline operation:** a complete Stock Study and a portfolio risk review run with no
  network. The only online action is a **user-initiated manual price/data refresh**.
- Fetched data is **cached locally** with source + timestamp; on provider failure, affected data is
  flagged **stale / to-update** (never a silent wrong signal). FX rates are likewise cached and
  dated.

### Implementation Considerations

- **Thin UI over a tested calculation crate**; a `MarketDataProvider` trait for vendor adapters;
  a **versioned serde data contract decoupled from Slint and SQLite** (so CLI/AI façades can be
  added later at near-zero cost).
- **GPL-3.0** dependency-license audit (notably Slint's licensing tier and Apache-2.0 ↔ GPL-3.0
  one-way compatibility across the crate tree).

## Project Scoping & Phased Development

**Delivery mode:** phased (per the product brief). A **lean MVP first** (chosen): prove the
methodological core and the single-stock workflow before tackling portfolio complexity.

### MVP Strategy & Philosophy

**MVP approach:** problem-solving + experience MVP — prove the core loop (*study → judge → simple
risk → decide*) is fast, trustworthy, and **genuinely used by the author himself**. Success is
adoption-by-self for real decisions, not external traction.
**Resource requirements:** solo developer (the author), Rust + Slint; **thin UI over a tested
calculation core** so scope stays controllable.

### MVP Feature Set (Phase 1)

**Core journeys supported:** 1 (new study, good coverage), 2 (CH/EU partial coverage + validation),
2b (annual update), 5 (reopen a past study), and a **simplified** 3/4 (single-portfolio risk +
manual sell/raise-stop).

**Must-have capabilities:**
- Faithful **Stock Study**: auto-fetch + **first-class manual entry**; per-cell coverage
  (present / to-fill / not-available-accepted), source/provenance, **user-set "validated"** flag;
  **5-year floor → low-confidence label**; calculations in **native currency**.
- **Deterministic, tested SSG engine** (golden-fixture + property tests).
- **Interactive growth/valuation chart**: draggable judgment lines (gesture + value), colored zones,
  live recalc, undo.
- **Watchlist** + neutral **buy-zone alerts** (in-app).
- **Single portfolio, single reference currency:** a simple holdings register (ticker, quantity,
  purchase price, trailing stop) with a **simple capital-at-risk** (Σ (purchase − stop) × qty while
  stop ≤ cost); neutral **sell / stop triggers** with manual actions (sell / raise stop).
- **Local SQLite**, journal-ready **versioned data model** + schema migrations; per-cell journal
  with **decision rationale**.
- Cross-cutting: neutral posture (footer disclaimer), Settings (no wizard), locale, OS keychain,
  external (NAS) backup of a single file.

### Post-MVP Features

Per-phase feature lists are in **Product Scope** above and carried per-requirement via the phase
tags (**[P2]/[P3]/[V]**) in **Functional Requirements**. In brief: **Phase 2** = multi-portfolio,
multi-currency/FX consolidation, full transaction ledger, complete risk overlay, dividends,
replacement surfacing; **Phase 3** = Company Comparison, Portfolio Health Review,
discovery/screening + Quick Screen, more provider adapters, PDF/print export, optional OS-native
notifications; **Vision** = read-only AI clerk-of-memory (+ remote option), legacy import, deeper
multi-portfolio analytics, withholding-refund tracking, export/share, eventual public release.

### Risk Mitigation Strategy

- **Technical:** the **draggable judgment lines on Slint** are the top unknown → **week-1 spike**,
  **egui** fallback. The *silent wrong signal* risk → deterministic engine with golden/property
  tests, validated flags, stale flagging, 5-year floor.
- **Market / adoption:** the only user is the author → de-risked by building exactly his real
  workflow; validated by whether he actually uses it for every decision.
- **Resource (solo dev):** the lean MVP defers portfolio complexity to Phase 2; ship the
  **versioned data contract** now so later phases and the AI façade are cheap to add.

## Functional Requirements

> Phase tags: **[P1]** MVP · **[P2]** Portfolio depth · **[P3]** Growth · **[V]** Vision.
> FRs reference pinned definitions in **Appendix A — Definitions** (exact formulas/thresholds are
> finalized in the NFRs / Architecture phase). Implementation choices (Rust/Slint, local store,
> OS secret store, direct-manipulation gestures, single backup file) are captured as
> constraints/NFRs, not as FRs.

### Stock Study & Methodology Engine
- FR1 **[P1]:** The user can create a Stock Study for a security.
- FR2 **[P1]:** The user can persist and reopen a study with its full state intact.
- FR3 **[P1]:** The user can update an existing study (re-fetch / edit) and extend its projection.
- FR4 **[P1]:** The system computes the SSG output set (enumerated in Appendix A) deterministically
  from a study's inputs.
- FR5 **[P1]:** All study calculations are performed in the security's native currency.
- FR6 **[P1]:** The user can set judgment inputs (future growth, forecast P/E, low-price method) and
  see results recompute.
- FR7 **[P1]:** The system raises methodology quality flags per the thresholds in Appendix A.
- FR8 **[P1]:** With fewer than five usable years (Appendix A), the study is computed on available
  data and carries a **queryable low-confidence state**.

### Calculation Integrity & Trust
- FR9 **[P1]:** The user can load and run bundled **golden reference studies**; the system reports
  any deviation beyond a defined tolerance.
- FR10 **[P1]:** The system detects and surfaces **input plausibility issues** (unadjusted split /
  series break, currency mismatch, fiscal-period misalignment, out-of-bound values per Appendix A)
  as user-visible warnings, distinct from quality flags.
- FR11 **[P1]:** The user can view a verdict's **traceability** — its inputs, their provenance, and
  the rule that produced the result.
- FR12 **[P1]:** The verdict's presentation is **degraded or withheld testably** when a load-bearing
  input is not validated or the study is low-confidence (Appendix A defines "load-bearing input").
- FR13 **[P1]:** All user-facing signals are **neutral** — no output contains an action/recommendation
  verb from the banned-verb list in Appendix A (verifiable).
- FR14 **[V]:** The AI module is **verifiably read-only** — any write to studies/judgments/verdicts/
  transactions is rejected and logged.

### Data Acquisition, Provenance & Providers
- FR15 **[P1]:** The user can auto-fetch a security's fundamentals, prices and estimates from a
  configured provider.
- FR16 **[P1]:** The user can enter, override and later correct any data field by hand.
- FR17 **[P1]:** Each data cell carries an independently queryable **source** (provider/manual/derived).
- FR18 **[P1]:** Each data cell carries an independently queryable **provenance and timestamp**.
- FR19 **[P1]:** Per-cell coverage is represented as **present / to-fill / not-available-accepted**.
- FR20 **[P1]:** The user can mark a cell or study **"validated"**; the flag resets on a cell when its
  value changes.
- FR21 **[P1]:** The user can trigger a **manual refresh** of provider data.
- FR22 **[P1]:** On refresh, a **manual value takes precedence** over a fetched value while the fetched
  value is preserved (non-destructive reconciliation).
- FR23 **[P1]:** On provider failure, last-known values are retained and affected data is flagged
  **stale/to-update**.
- FR24 **[P1]:** A provider failure's **cause** (network, quota/rate-limit, invalid/absent key) is
  recorded and reported.
- FR25 **[P1]:** The user can use keyless providers, and **add/replace/delete/test** a provider API key
  stored in the OS secret store.
- FR26 **[P2]:** The user can configure a preferred provider and a **fallback chain per field type**
  (price, fundamentals, FX), with the effective provider recorded.
- FR27 **[P2]:** The system respects a provider's declared **quotas/rate-limits** and **batches**
  watchlist/portfolio fetches to stay within them.
- FR28 **[P2]:** The system acquires, timestamps and retains **FX rates** per currency pair with a
  freshness state; FX is applied only at consolidation.
- FR29 **[P1]:** The system **recomputes deterministically** on a change of input, judgment, price, FX
  rate, or schema migration, distinguishing the cause.

### Charts & Judgment Interaction
- FR30 **[P1]:** The user can view growth and valuation charts for a study.
- FR31 **[P1]:** The user can set a judgment line by **exact value or direct manipulation** (kept in
  sync), with live recalculation of zones.
- FR32 **[P1]:** The user can undo judgment changes; adjusting a line never destroys a saved input.
- FR33 **[P1]:** The system never auto-places or suggests a judgment line.

### Watchlist & Alerts
- FR34 **[P1]:** The user can maintain a watchlist (add, edit, remove, reorder).
- FR35 **[P1]:** The system raises a **neutral in-app alert** when a watched security enters its buy zone.

### Portfolio, Transactions & Holdings
- FR36 **[P1]:** The user can record holdings in a **single portfolio** (security, quantity, purchase
  price) in a single reference currency, and edit or remove a holding.
- FR37 **[P2]:** The user can maintain **multiple portfolios** (one per bank/account).
- FR38 **[P2]:** The user can hold securities denominated in **multiple currencies**.
- FR39 **[P2]:** The user can record buy/sell **transactions** including partial sells (date, quantity,
  unit price, fees, currency) and edit/delete them; the cost basis is derived per Appendix A
  (weighted-average).
- FR40 **[P1]:** The user can trigger a manual price refresh recomputing each holding's zone and showing
  freshness.
- FR41 **[P2]:** The user can record **dividends**; the study uses gross, the portfolio's reinvestable
  cash uses net per the withholding rule in Appendix A.

### Risk Management
- FR42 **[P1]:** The user can set a **trailing stop** per holding; it ratchets up only.
- FR43 **[P1]:** The system computes a **simple capital-at-risk** for the single portfolio per the
  formula in Appendix A.
- FR44 **[P2]:** The system computes **capital-at-risk per currency → per bank → global total** in the
  reference currency (FX only at consolidation).
- FR45 **[P2]:** The system checks **concentration** against total invested capital and warns near a
  configured majority share.
- FR46 **[P1]:** On Sell-zone entry or stop breach, the system surfaces a **neutral fact** and offers
  manual actions (sell / raise stop), never auto-acting.
- FR47 **[P1]:** The **stop-loss takes priority over the Sell zone** (isolated business rule).
- FR48 **[P2]:** On a sell, the system surfaces **replacement candidates** from the watchlist and flags
  re-concentration by sector/currency.

### Cumulative Memory & Journal
- FR49 **[P1]:** The user can capture a **decision rationale** as a first-class field on studies and
  transactions.
- FR50 **[P1]:** The user can reopen a past study and **visually compare** its recorded projection to
  the security's actual trajectory since.
- FR51 **[P1]:** The system durably preserves the **time-series** of judgments, provenance, validation,
  rationale and notes.

### Reporting & Printing
- FR52 **[P1]:** The user can **print / export to PDF a Stock Study** in a layout close to the original
  form, using neutral labels and **no NAIC marks/logos or verbatim instructional text**.
- FR53 **[P2/P3]:** The user can print / export the **other forms** (Company Comparison, Portfolio) in
  the same faithful-but-neutral layout, each with its feature phase.

### Application Shell & Data Management
- FR54 **[P1]:** The user can **list, search, sort and filter** saved studies and open them from a home
  dashboard.
- FR55 **[P1]:** The user can **delete or archive** a study (with confirmation); deletions never corrupt
  the journal time-series.
- FR56 **[P1]:** The user can switch a study between an **entry regime** (dense editing) and a
  **contemplation regime** (reading/judgment), with the active regime clearly indicated.
- FR57 **[P1]:** The user can view a consistent **legend** for freshness/provenance/coverage/confidence
  states.
- FR58 **[P1]:** Every main surface presents an **actionable empty state** and clear neutral
  error/feedback messages.
- FR59 **[P1]:** The user can **export/import a single study** to a portable versioned file (round-trip
  preserves identity), enabling golden-study and seeding.
- FR60 **[P1]:** The user can **export/import the whole journal** in a versioned format, validated on
  import (reject/migrate on version mismatch).
- FR61 **[P1]:** The user can **restore from a backup** with integrity and version-compatibility checks
  before overwrite.
- FR62 **[P1]:** The user can access **non-blocking contextual help / glossary** and a read-only
  **demonstration study**.

### Configuration, Posture & Operation
- FR63 **[P1]:** The user can configure providers/keys, the single global reference currency, risk
  thresholds, the label set (NAIC↔neutral) and locale number format — without a blocking setup flow.
- FR64 **[P1]:** A disclaimer (educational, not a financial advisor) is **always visible**, and the
  product never issues recommendations.
- FR65 **[P1]:** The user can run the full study and portfolio-risk workflow **offline**; the only online
  action is a user-initiated refresh.
- FR66 **[P1]:** The journal is kept in a **portable local store** an external system (e.g. file sync) can
  back up.

> **Definitions referenced by these FRs** (SSG output set, quality-flag thresholds, "usable year"
> & low-confidence rule, plausibility rules, "load-bearing input", neutrality banned-verb list,
> golden tolerance, stale threshold, cost basis, dividend net rule, capital-at-risk formula) are
> consolidated in **Appendix A — Definitions** at the end of this document.

## Non-Functional Requirements

### Correctness & Calculation Integrity (top priority)

- **NFR-C1:** The calculation engine is **deterministic** — identical inputs always produce
  identical outputs, bit-stable across runs and platforms.
- **NFR-C2:** Engine output **matches every bundled golden reference study**: exact match on zoning
  and verdict, and within **±0.5%** on derived numeric values (tolerance configurable).
- **NFR-C3:** Property-based **invariants hold**: zone bounds ordered (low < buy < hold < sell <
  high); upside/downside ≥ 0; capital-at-risk ≥ 0; FX round-trip A→B→A within 1e-6.
- **NFR-C4:** FX is applied **only at consolidation**; per-currency study results are independent of
  the chosen reference currency.
- **NFR-C5:** The engine + risk crate are gated in **CI** by golden-fixture and property tests
  (target ≥ 95% coverage of the calculation paths); a failing test blocks merge.

### Performance

- **NFR-P1:** Judgment-line recalculation and zone re-render feel **live** — within **~100 ms**
  perceived while dragging.
- **NFR-P2:** Opening or recomputing a full study completes within **~1 s** on typical hardware.
- **NFR-P3:** A manual portfolio refresh (tens of holdings) completes within a few seconds, network
  permitting, and **never blocks the UI**.
- **NFR-P4:** The app reaches an interactive state within **~3 s** of launch.

### Security & Privacy

- **NFR-S1:** Provider API keys live **only in the OS secret store** — never in the repo, plaintext
  config, logs, exports or backups.
- **NFR-S2:** **No telemetry/analytics**; the only network calls are user-initiated provider/FX
  fetches.
- **NFR-S3:** All persistent data is **local**; nothing is sent to third parties beyond the chosen
  provider under the user's own key.
- **NFR-S4:** The AI module never exfiltrates the journal to a remote service **unless the user
  explicitly enables a remote AI**.

### Reliability & Data Integrity

- **NFR-R1:** The full study + portfolio-risk workflow runs **offline**; losing the network degrades
  only fetching, with **stale flagging** — never a silent wrong value.
- **NFR-R2:** Writes are **crash-safe/atomic** — an interrupted operation never corrupts the journal.
- **NFR-R3:** Schema migrations are **forward-safe**; an older journal always opens (or is migrated)
  in a newer build, with **no data loss**.
- **NFR-R4:** Reconciliation **never destroys** a manual value or judgment; the provider value is
  preserved alongside.
- **NFR-R5:** Export/import and restore **verify integrity and schema version**; a mismatched or
  corrupt file is rejected with a clear message, never partially applied.

### Portability & Compatibility

- **NFR-X1:** **Identical behavior and numeric results** across Windows, macOS, Linux.
- **NFR-X2:** **Locale-aware** number parsing/formatting (decimal comma, thousands), configurable
  independently of OS locale.
- **NFR-X3:** The journal file is **portable** across platforms.

### Usability & Accessibility (right-sized)

- **NFR-U1:** Buy/hold/sell zones are distinguishable **without relying on color alone**
  (color-blind-safe palette + a secondary cue) — color carries decision meaning.
- **NFR-U2:** Primary study and data-entry workflows are **fully keyboard-operable**.
- **NFR-U3:** On-screen and printed layouts stay **recognizably close to the original form**
  (functional layout) with neutral labels.
- *Note:* full public-audience WCAG / Section-508 is out of scope for a single-user tool; revisit if
  published.

### Maintainability & Testability

- **NFR-M1:** The UI is a **thin layer** over a UI-independent tested calculation crate and a
  **versioned data contract decoupled** from Slint and the storage engine.
- **NFR-M2:** The data contract carries an explicit `schema_version`; any breaking change ships a
  migration.

### Constraints (technical & legal/IP)

- **Technical:** Rust + Slint (**egui** as a contingency for interactive charting); local embedded
  database (SQLite); offline-first; no server.
- **Legal / IP:** **GPL-3.0**, subject to a dependency-license audit (Slint tier; one-way
  Apache-2.0 ↔ GPL-3.0 compatibility). **No vendor market data** shipped (synthetic fixtures only).
  **Neutral labels — no NAIC marks/logos or verbatim instructional text.** "Educational, not
  advice" is a **design intent, not a legal opinion**; per-provider usage/retention ToS is the
  **end-user's responsibility**.

### Not Applicable

- **Scalability:** single-user, local, no growth/traffic concerns — intentionally out of scope.

## Appendix A — Definitions (pinned values referenced by FRs)

- **Capital-at-risk (single portfolio, FR43):** Σ over holdings of `max(0, (avg_cost − stop)) ×
  qty`, counted only where `stop ≤ avg_cost`; per currency natively, converted at current FX for the
  global total.
- **"Usable year" / low-confidence (FR8):** a year with all load-bearing fields present (sales, EPS,
  high/low price); **study is low-confidence when usable years < 5**.
- **Cost basis (FR39):** weighted-average, **fees included**.
- **Dividend net (FR41):** `gross × (1 − withholding_rate)`, rate per jurisdiction (CH = 35%);
  study uses gross.
- **Stale threshold (FR23):** price data older than the user-configured horizon (default: older than
  one trading day) is flagged stale.
- **Neutrality (FR13):** system-generated signals contain **no imperative action verb** (buy/sell/
  hold as a command); they state facts only. (Exact banned-verb list finalized in Architecture.)
- **SSG output set (FR4), plausibility rules (FR10), load-bearing input (FR12), golden tolerance
  (FR9):** finalized in Architecture.
