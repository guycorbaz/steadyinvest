---
title: "Product Brief: steadyinvest"
status: "complete"
created: "2026-06-05"
updated: "2026-06-05"
inputs:
  - _bmad-output/planning-artifacts/research/domain-naic-better-investing-research-2026-06-05.md
  - docs/NAIC/ (SSG handbook, tutorials, official forms)
---

# Product Brief: steadyinvest

## Executive Summary

**steadyinvest** is an independent, open-source **desktop application** (Rust + Slint) that brings the time-tested NAIC / BetterInvesting **quality-growth investing methodology** — the Stock Selection Guide (SSG) family of analyses — to a modern, owned, offline-capable tool. It helps an individual investor **choose stocks, judge a fair price, decide when to buy or sell, and manage portfolio risk**, while **saving substantial time** by fetching company financials and analyst estimates automatically instead of by hand.

The methodology is proven but its tooling is dated and constrained: the official web apps are subscription-gated and locked to a single data vendor, and the only true desktop option (Toolkit 6) is Windows-only and discontinued. steadyinvest fills that gap with a **cross-platform, locally-owned** app that stays **faithful to the familiar forms** (so an experienced user is never disoriented) while adding an **interactive analytical layer**: live recalculation, draggable judgment lines, and color-coded buy/hold/sell zones.

Built primarily for its author and **publishable on GitHub** for other individual investors, steadyinvest is data-vendor-agnostic (the user supplies their own API key), works across **European, Swiss, US and other markets**, and keeps all data in a **local store** for full offline operation between price refreshes.

## The Problem

Disciplined fundamental analysis works, but doing it well is **tedious and time-consuming**. Completing a single rigorous stock study means gathering ~10 years of sales, earnings, margins, P/E history, prices and dividends, then running a chain of calculations to reach buy/hold/sell zones and a projected return. Today an investor following this method must either:

- **Pay for a subscription web tool** that is online-only, tied to one data vendor (Morningstar), and gated behind membership; or
- **Use a discontinued, Windows-only desktop tool** (no Mac/Linux, no new development); or
- **Do it by hand** in spreadsheets — accurate but slow, error-prone, and painful to keep current.

None of these gives an investor who trades **across multiple regions (EU/CH/US)** an option that is **independent, cross-platform, offline-capable, and free of data-vendor lock-in**. And keeping a portfolio's valuations current — refreshing prices to recompute zones and ratios — is manual drudgery.

## The Solution

steadyinvest reproduces the full methodology as software:

- **Stock Study** — a faithful, interactive version of the 5-section SSG (growth trend, management quality, valuation history, risk/reward zoning, 5-year return projection) with **all calculations automated** and **data auto-fetched** from the user's chosen provider — plus **first-class manual entry/override** so any field can be filled or corrected by hand when coverage is partial.
- **Interactive analysis layer** — growth/valuation charts with **draggable judgment lines**, **colored buy/hold/sell zones**, and **real-time recalculation** as the user adjusts estimates.
- **Watchlist & alerts** — follow candidate stocks and get notified when one **enters its Buy zone**.
- **Company Comparison** — evaluate several candidates side-by-side on the key metrics.
- **Portfolio Tracker & Health Review** — track holdings across **CHF/EUR/USD** (with FX), **refresh prices regularly**, recompute each holding's zone and the portfolio's quality/diversification picture.
- **Quick Screen** — a lightweight first-pass checklist before a full study.

The experience deliberately keeps the **layout and field arrangement close to the original forms** — a hard design constraint, because too much deviation would **confuse a user already familiar with the method**. The augmentation (charts, live recalculation, color) is layered *on top of* that familiar structure, not in place of it, turning a slow manual chore into a few-minute, judgment-supported workflow.

## What Makes This Different

- **Independent & owned** — no BetterInvesting subscription, no account, runs offline; the user owns the code and the data. (A data-provider API key is still required — often a paid plan for active portfolio refresh — but the choice of provider is the user's and is not locked in.)
- **Data-vendor-agnostic** — a pluggable provider layer (default **EODHD** for broad EU/CH/US/global coverage incl. fundamentals + analyst estimates); swap or add providers freely. The user brings their own API key; **no vendor data is redistributed**.
- **Genuinely cross-platform** — Windows, macOS, Linux (the discontinued desktop incumbent was Windows-only).
- **Form fidelity + modern interactivity** — familiar layout *and* live, visual, judgment-friendly analysis.
- **Open source** — inspectable, extensible, community-improvable.
- **Methodology-faithful core** — the calculations are implemented exactly, as a tested engine, not approximated.

## Who This Serves

- **Primary: the author** — an individual investor applying the quality-growth method across European, Swiss, US and other markets, who wants a fast, independent, offline tool he fully controls. Success = completing a trustworthy stock study in minutes and keeping his portfolio's signals current with minimal effort.
- **Secondary: other individual investors** who discover it on GitHub — self-directed investors (and informal investment-club members) who want an independent, cross-platform alternative to subscription tools. They benefit from the same workflow; the project stays general and documented enough to be useful to them, without optimizing the first version for stranger onboarding.

## Success Criteria

- **Primary signal — time saved on a stock study:** produce a complete, reliable study in **a few minutes** via auto-fetch, versus the much longer manual/spreadsheet path.
- **Decision support:** every study yields clear, correct buy/hold/sell zoning, upside-downside ratio, and projected return; quality flags (declining margins, high debt, weak ROE) surface automatically.
- **Currency with low effort:** portfolio holdings' prices refresh on a configurable cadence (at least daily EOD) and recompute zones/ratios without manual data entry.
- **Independence:** runs fully offline between refreshes; no dependency on any subscription service; data and code owned locally.
- **Methodological correctness:** calculations match the reference methodology (validated by a tested calculation engine).

## Scope

**Product ambition = the complete vision** (all three capability phases): Stock Study, Company Comparison, Portfolio Tracker & Health Review, and screening/diversification reports.

**Cross-cutting essentials — present from v1 (confirmed):**
- **First-class manual data entry / override** — every auto-fetched field can be entered or corrected by hand; the app stays fully usable when a provider's coverage is partial (critical for smaller EU/CH names). Not a fallback bolt-on — a primary input path.
- **Multi-currency with FX handling — CHF, EUR, USD** — per-security native currency plus consistent conversion for portfolio aggregation and cross-market comparison.
- **Watchlist with alerts** — track candidate stocks and get notified when one **enters its Buy zone** (leverages the regular price refresh).

**Delivery is sequenced** so usable value arrives early (managed via epics, not by cutting the vision):
1. **Calculation engine + Stock Study** with auto-fetch **and first-class manual entry**, the interactive growth chart (judgment lines, colored zones), and the **watchlist + buy-zone alerts**.
2. **Portfolio Tracker** (regular price refresh, per-holding zones, **multi-currency/FX**) **+ Company Comparison + Portfolio Health Review** (diversification/quality roll-up).
3. **Screening, additional reports, multi-portfolio & risk views, legacy study import, export/share.**

**Explicitly out (for now):** brokerage execution / account linking; community/social features (member sentiment, online study sharing); real-time intraday data as a hard requirement (daily EOD suffices; intraday is a later "nice-to-have").

## Technical Approach (high level)

- **Rust** core; **Slint** GUI (author's existing expertise, cross-platform, strong for faithful form layouts).
- **Calculation engine as a UI-independent, unit-tested crate** — the methodological heart; reused by GUI and any future CLI/export.
- **`MarketDataProvider` trait** with vendor adapters (EODHD first); **user-supplied API keys** kept out of version control (OS keychain / local config).
- **Local-first persistence** with embedded **SQLite (rusqlite)** — studies, cached financials, portfolios, judgment inputs; full offline operation.
- **Charting** via plotters or a custom Slint canvas (interactive judgment lines + zone bands) — flagged as the main technical risk to prototype early.

## Compliance & IP Posture

- **Educational use, not investment advice** — prominent disclaimer; no brokerage/advice functionality keeps the tool outside regulated territory.
- **Trademark/copyright (reconciled with the fidelity requirement):** the forms must stay **visually close to the originals** so users are never confused — and this is largely safe, because a **functional layout** (sections, columns, field order, recognizable arrangement) is not protectable. Only the precise creative *expression* is: **logos, verbatim instructional prose, decorative elements**. Approach: **faithfully reproduce the form layout/structure**; avoid only logos + copied instructional text; keep the **product name/branding neutral ("steadyinvest")**; show an **"independent, not affiliated"** notice. NAIC terminology may be used as in-app labels for the author's comfort, kept in a **swappable label layer**.
- **Data licensing:** no vendor data redistributed; users fetch under their own keys; per-provider terms respected.
- **OSS license:** to be chosen (permissive vs copyleft).

## Key Risks & Open Questions

Surfaced by the brief review; to resolve in PRD / Architecture:

- **Data fidelity for EU/CH stocks (biggest risk — mitigated).** The SSG needs specific fields (pre-tax profit, book value, 10 yrs of annual high/low prices, forward analyst estimates) and provider coverage for smaller European/Swiss names is unproven. **Mitigation decided:** first-class manual entry/override (in v1) keeps the app fully usable when auto-fetch is partial. Still validate provider coverage early.
- **Methodology fit across markets.** The method was designed around US data (Value Line); IFRS vs US-GAAP definitions (e.g. pre-tax profit) and non-US reporting need care so calculations stay meaningful abroad.
- **Interactive charting on Slint.** Draggable judgment lines + zone bands are the signature interaction and Slint's main technical unknown → prototype first.
- **Multi-currency/FX correctness (now in scope).** Handling CHF/EUR/USD across prices, financials and aggregation must be precise (which currency a given financial figure is reported in; FX-rate source and timing) — a design point for the PRD/Architecture.
- **Data cost vs "independence."** Independence is from BetterInvesting/subscription tooling; a (often paid) data API is still required for active refresh — frame honestly to users.

**Confirmed in v1:** first-class manual entry/override, multi-currency (CHF/EUR/USD) with FX, and a watchlist with buy-zone alerts.

**Opportunities still on the roadmap (not v1):** **study history/versioning** (track how judgment and zones evolved over time), **legacy study import** (`.ITK` / SSGPlus) to lower switching cost, and **PDF/print export** of a study (the "& Report" side of the original form).

## Vision

In 2–3 years, steadyinvest becomes the **reference open-source desktop tool** for disciplined, quality-growth fundamental analysis across global markets — the independent, offline, multi-vendor home for investors who want to *own* their process end to end. It grows from a faithful, faster SSG into a richer judgment-support environment (deeper portfolio risk and diversification analytics, more data providers and regions, study import/interoperability, and export/sharing) — without ever requiring a subscription or surrendering the user's data.
