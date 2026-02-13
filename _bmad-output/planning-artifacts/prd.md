---
stepsCompleted: [step-01-init, step-02-discovery, step-03-success, step-04-journeys, step-05-domain, step-06-innovation, step-07-project-type, step-08-scoping, step-09-functional, step-10-nonfunctional, step-11-polish, step-e-01-discovery, step-e-02-review, step-e-03-edit]
inputDocuments: ["_bmad-output/planning-artifacts/product-brief-naic-2026-02-03.md", "_bmad-output/planning-artifacts/research/domain-naic-betterinvesting-methodology-research-2026-02-03.md", "_bmad-output/planning-artifacts/research/technical-market-data-apis-research-2026-02-03.md", "_bmad-output/brainstorming/brainstorming-session-2026-02-03.md", "_bmad-output/implementation-artifacts/epic-6-retro-2026-02-10.md"]
documentCounts: {briefCount: 1, researchCount: 2, brainstormingCount: 1, projectDocsCount: 1}
classification:
  projectType: web_app
  domain: fintech
  complexity: high
  projectContext: evolution
workflowType: 'prd'
lastEdited: '2026-02-11'
editHistory:
  - date: '2026-02-11'
    changes: 'Validation report improvements: added Journey 5 (thesis evolution — closes traceability gaps for FR3.4, FR4.2, Analysis Comparison criterion), added measurement methods to all NFRs, reclassified NFR4 as FR1.5 (behavioral → functional), removed implementation leakage from FR7.1 (bcrypt/argon2) and NFR1 (SPA), renumbered NFRs 4-6.'
  - date: '2026-02-11'
    changes: 'Sprint Change Proposal edits: removed FR4.4 (implementation leakage), removed NFR6 (no deployed data to preserve), added FR5.2 (per-portfolio config), sharpened FR5.6 (position sizing with example), sharpened FR7.1 (bcrypt/argon2 auth), added Explicitly Deferred section, renumbered NFRs and FR5.x.'
  - date: '2026-02-10'
    changes: 'Post-MVP evolution: expanded vision from SSG analysis tool to investment management platform with portfolio management, risk discipline, watchlists, DB persistence, and multi-user roadmap. Aligned with Epic 6 retrospective product vision. Added multi-stock comparison (FR4.3) with user-selectable base currency per party mode validation discussion.'
---

# Product Requirements Document: SteadyInvest

**Author:** Guy
**Date:** 2026-02-03
**Last Edited:** 2026-02-11
**Status:** Draft (Revised — Post-MVP Evolution)

## Executive Summary

**SteadyInvest** is an open-source investment analysis and portfolio management platform built on the NAIC Stock Selection Guide (SSG) methodology. The platform automates the labor-intensive SSG process for European and US markets, and evolves beyond analysis into a complete investment decision framework.

The platform addresses three moments in the investment lifecycle:

1. **Analyze** — The SSG chart tells you *what* to buy and at *what price* (delivered in MVP, Epics 1-6).
2. **Buy Smart** — Position sizing tells you *how much* to buy; stop loss reminders tell you *how to protect it*.
3. **Stay Balanced** — Exposure checks tell you *when to rebalance* your portfolio.

The MVP (Epics 1-6) delivers automated "One-Click History" for Swiss (SMI), German (DAX), and US markets with web-native logarithmic charting, quality dashboards, projection manipulation, thesis locking, PDF export, and system monitoring. The post-MVP roadmap extends **SteadyInvest** from an analysis tool into a practical investment management platform with database-persisted analyses, portfolio tracking with risk discipline, watchlists, and multi-user support.

## Success Criteria

### User Success (MVP — Delivered)

- **30-Minute Threshold**: Beginner analysts can complete a full NAIC analysis in under 30 minutes.
- **Analysis Confidence**: High trust in automated data accuracy, evidenced by minimal manual overrides.
- **Decision Empowerment**: Users transition from data entry to professional-grade auditing and selection.

### User Success (Post-MVP)

- **Portfolio Discipline**: Users receive actionable over-exposure and rebalancing alerts before making buy/sell decisions.
- **Analysis Comparison**: Users can retrieve and compare past analyses from the database to track thesis evolution over time.
- **Investment Workflow**: Complete analyze-size-protect-rebalance cycle without leaving the platform.

### Business & Technical Success

- **Strategic Position**: Established as the premier open-source tool for international (CH/DE) NAIC SSG investors within 6 months.
- **Data Parity**: Automated data matches official reports for 95% of mid/large-cap tickers.
- **System Performance**: "One-Click" 10-year data population completes in under 5 seconds.
- **Ticker Coverage**: Expand beyond initial test tickers to cover major indices (SMI, DAX, S&P 500) with reliable data retrieval.

## Project Scoping & Phased Development

### Phase 1: Fix & Foundation (Current)

Focus: Resolve MVP gaps, establish database persistence, and expand ticker coverage.

- **PDF Export Accessibility**: Ensure PDF export is reachable from the UI navigation menu (currently built but not accessible).
- **Ticker Coverage Expansion**: Broaden data provider coverage beyond the current ~2 test tickers to support major index constituents.
- **Database-Persisted Analyses**: Migrate from browser-only file downloads to server-side storage, enabling analysis retrieval and comparison.
- **Chart Improvements**: Increase SSG chart height for readability; add persistent legend below chart showing series names and colors.
- **Schema Design**: Design database schema with `user_id` and multi-portfolio columns from day one to support seamless multi-user migration in Phase 3.

### Phase 2: Portfolio & Watchlist

Focus: Transform from analysis tool to investment management platform.

- **Holdings Tracking**: Record stock purchases with quantity, price, and date; track current portfolio composition.
- **Position Sizing**: Propose buy amounts that maintain diversification targets per portfolio rules.
- **Exposure Rules**: Detect when a single stock exceeds a configurable portfolio threshold; suggest rebalancing when a stock rises significantly.
- **Trailing Stop Loss Reminders**: Prompt at purchase time to set broker stop losses (no real-time alerting — broker tools handle execution).
- **Watchlists**: Track stocks of interest with saved analysis references for future purchase decisions.
- **Per-Portfolio Configuration**: All risk thresholds and rules configurable independently per portfolio.
- **Multiple Portfolios**: One user can own multiple portfolios with independent rules and holdings.

### Phase 3: Multi-User

Focus: Authentication and personal workspaces.

- **User Authentication**: Secure login and session management.
- **Personal Workspaces**: Per-user analysis libraries, portfolios, and watchlists.
- **Per-User Portfolios**: Each user manages their own portfolios with independent configurations.

### Phase 4: Collaboration (Vision)

Focus: Shared analysis and team investment management.

- **Shared Analyses**: Users can share SSG analyses with other users or investment club members.
- **Team Portfolios**: Collaborative portfolio management for investment clubs.
- **Community Library**: Curated "Gold Standard" SSG analyses contributed by the community.

### Explicitly Deferred

The following features from the original product brief are not planned for any current phase:

- **Data Oracle / OCR**: AI-powered PDF annual report data ingestion. May be revisited if manual data entry becomes a significant user pain point again.
- **Market Expansion**: French (CAC 40) and UK (FTSE 100) market support. May be revisited based on user demand.
- **Collaboration (Phase 4)**: Shared analyses, team portfolios, and community library. Deferred until Phase 3 (Multi-User) is delivered and validated.

## User Journeys

### Journey 1: Markus, the Swiss Value Hunter (Primary — MVP)

Markus avoids the 2-hour manual entry chore for Swiss stocks. He uses **SteadyInvest** to instantly generate a 10-year chart for `NESN.SW`, benchmarks it against a German peer, and prepares a clear recommendation for his investment club in 15 minutes.

### Journey 2: Elena, the Club Moderator (Secondary — MVP)

Elena uses **SteadyInvest** to standardize reports across her investment club. By providing identical, data-accurate charts to all members, she shifts club discussions from data verification to business quality.

### Journey 3: David, the Data Steward (Admin — MVP)

David monitors API health. When a German provider updates their schema, he receives automated alerts and can submit a fix before users experience any downtime in the "One-Click" engine.

### Journey 4: Markus Reviews His Portfolio (Post-MVP)

After analyzing a new stock with the SSG, Markus checks his portfolio dashboard. **SteadyInvest** shows his current holdings, flags that his tech sector exposure is at 38% (above his 30% threshold), and suggests he should limit his new purchase to CHF 2,000 to stay within his diversification rules. He sets a trailing stop loss reminder and adds the stock to his watchlist for a better entry price.

### Journey 5: Markus Tracks His Thesis Evolution (Post-MVP)

It's quarterly review time. Markus pulls up his `NESN.SW` analysis from Q3 and opens today's fresh analysis side by side. He compares how his Sales CAGR projection shifted from 6% to 4.5% after a weaker quarter, reviews the metric deltas, and locks his updated thesis with a note: "Growth slowing — hold, do not add." He shares the locked analysis with Elena's club for their next meeting.

## Domain-Specific Requirements

### Compliance & Regulatory

- **Accounting Standards**: System must handle IFRS vs. GAAP differences for international market extraction.
- **Data Licensing**: Adherence to provider Terms of Service, including necessary attribution for open-source use.
- **Currency Normalization**: Consistent handling of reporting vs. trading currencies to prevent ratio distortion.

### Technical Constraints

- **Data Integrity**: Automated checks for historical gaps or unrealistic outliers.
- **Stock Split Logic**: Mandatory automated handling of splits and reverse splits to maintain chart accuracy.
- **API Management**: Robust handling of rate limits and timeout fallbacks during batch processing.

### Portfolio Risk Rules

- **Over-Exposure Detection**: Alert when a single stock exceeds a configurable percentage of total portfolio value.
- **Rebalancing Triggers**: Suggest partial selling when a stock rises significantly above its target allocation.
- **Position Sizing**: Calculate optimal buy amount that maintains diversification targets given current portfolio composition.
- **Stop Loss Discipline**: Prompt users to set broker-level stop losses at purchase time; no platform-level real-time price monitoring.
- **Per-Portfolio Isolation**: Each portfolio maintains independent risk thresholds and rules; changes to one portfolio do not affect others.

## Innovation & Novel Patterns

- **"One-Click" Internationalization**: Unique automation for non-US markets that are currently manual-only in most tools.
- **Open-Source Methodology**: A transparent, auditable codebase challenging the "black box" nature of proprietary investment software.
- **Dynamic Normalization**: Native cross-currency peer benchmarking without manual spreadsheet adjustments.
- **Portfolio Discipline Engine**: Practical risk management that enforces investor discipline at decision time (buy/rebalance), not through complex real-time monitoring. Leverages broker infrastructure (stop losses) rather than duplicating it.

## Project-Type Specification: Web App (SPA)

- **Architecture Style**: Single Page Application (SPA) for stateful, app-like interactivity.
- **Browser Support**: Standard evergreen browsers (Chrome, Firefox, Opera, Safari).
- **SEO/Security**: No public SEO required (local use focus). All transit secured via HTTPS.
- **Implementation Strategy**: Focus on batch historical population over real-time price streaming.
- **Responsive Design**: Desktop-first layout with mobile read-only mode (CAGR values displayed as text instead of interactive sliders on small screens).
- **Accessibility**: WCAG 2.1 Level A minimum; ensure keyboard navigation and screen reader support for core analysis workflows.
- **Multi-User Readiness**: Architecture designed for seamless migration to multi-tenancy when authentication is introduced.

## Functional Requirements (Capability Contract)

### FR#1: Search & Population

- **FR1.1**: Users can search international stocks by ticker (e.g., NESN.SW).
- **FR1.2**: System retrieves 10-year historicals (Sales, EPS, Prices) automatically.
- **FR1.3**: System adjusts data for historical splits and dividends.
- **FR1.4**: System normalizes multi-currency data for comparison, supporting a user-selectable base currency for cross-market analysis.
- **FR1.5**: System flags all detected data gaps explicitly to the user rather than silently interpolating missing values.

### FR#2: Analysis & Visualization

- **FR2.1**: System calculates 10-year Pre-tax Profit on Sales and ROE.
- **FR2.2**: System calculates 10-year High/Low P/E ranges.
- **FR2.3**: Users can manually override any automated data field.
- **FR2.4**: System renders logarithmic trends for Sales, Earnings, and Price.
- **FR2.5**: System generates trend line projections and "Quality Dashboards."
- **FR2.6**: Users can interactively manipulate projection trend lines (drag Sales/EPS CAGR); valuation metrics update in real time.

### FR#3: Reporting & Operations

- **FR3.1**: Users can export standardized SSG reports (PDF/Image) from the UI navigation menu.
- **FR3.2**: Users can save/load analysis files for review.
- **FR3.3**: Admins can monitor API health and flag data integrity errors.
- **FR3.4**: Users can lock an analysis thesis, capturing a timestamped snapshot of all projections and overrides for future reference.

### FR#4: Analysis Persistence (Phase 1)

- **FR4.1**: System stores completed analyses in the database with ticker, date, and snapshot data, enabling retrieval and comparison.
- **FR4.2**: Users can retrieve past analyses for the same ticker and compare thesis evolution across time (e.g., side-by-side metric deltas between quarterly reviews).
- **FR4.3**: Users can compare projected performance metrics across multiple tickers (not limited to two) in a compact summary view, enabling ranking and selection decisions. Percentage-based metrics (CAGRs, P/E, ROE) display without currency conversion; monetary values convert to a user-selectable base currency using current exchange rates.

### FR#5: Portfolio Management (Phase 2)

- **FR5.1**: Users can create multiple portfolios with independent names.
- **FR5.2**: Users can configure per-portfolio parameters: maximum per-stock allocation percentage, rebalancing thresholds, and risk rules. Each portfolio's configuration is independent.
- **FR5.3**: Users can record stock purchases (ticker, quantity, price, date) within a portfolio.
- **FR5.4**: System calculates current portfolio composition and per-stock allocation percentages.
- **FR5.5**: System detects over-exposure when a single stock exceeds its portfolio's configured maximum allocation threshold.
- **FR5.6**: System suggests a maximum buy amount for a given stock based on the portfolio's configured per-stock allocation threshold and current holdings (e.g., given a CHF 100K portfolio with a 10% max-per-stock rule and 0% current exposure, the system suggests a max buy of CHF 10K).
- **FR5.7**: System prompts trailing stop loss setup at purchase time.

### FR#6: Watchlist (Phase 2)

- **FR6.1**: Users can maintain a watchlist of stocks with notes and target buy prices.
- **FR6.2**: Watchlist entries can link to saved SSG analyses for quick reference.

### FR#7: Multi-User & Collaboration (Phase 3-4)

- **FR7.1**: Users can register and authenticate using username/password with industry-standard password hashing.
- **FR7.2**: Each user has a personal workspace with their own analyses, portfolios, and watchlists.
- **FR7.3**: Users can share analyses with other users or groups (Phase 4).

## Non-Functional Requirements (Quality Attributes)

### Performance & Reliability

- **NFR1**: Application initial load under 2 seconds on 10 Mbps broadband, as measured by Lighthouse performance audit.
- **NFR2**: "One-Click" 10-year population completes in < 5 seconds (95th percentile), as measured by application performance logs.
- **NFR3**: API integration engine maintains 99.9% success rate for primary CH/DE feeds, as measured by structured application logs over rolling 30-day windows.

### Security

- **NFR4**: All external API communications use encrypted HTTPS protocols, as verified by TLS certificate validation and network traffic inspection.

### Data Persistence & Portfolio Performance

- **NFR5**: Portfolio operations (position sizing, exposure checks) complete in < 1 second for portfolios up to 100 holdings, as measured by application performance logs.
- **NFR6**: Any historical analysis snapshot retrieves in < 2 seconds; multi-stock comparison queries complete in < 3 seconds for up to 20 analyses, as measured by application performance logs.
