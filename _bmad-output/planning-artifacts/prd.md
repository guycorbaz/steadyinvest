---
stepsCompleted: [step-01-init, step-02-discovery, step-03-success, step-04-journeys, step-05-domain, step-06-innovation, step-07-project-type, step-08-scoping, step-09-functional, step-10-nonfunctional, step-11-polish]
inputDocuments: ["_bmad-output/planning-artifacts/product-brief-naic-2026-02-03.md", "_bmad-output/planning-artifacts/research/domain-naic-betterinvesting-methodology-research-2026-02-03.md", "_bmad-output/planning-artifacts/research/technical-market-data-apis-research-2026-02-03.md", "_bmad-output/brainstorming/brainstorming-session-2026-02-03.md"]
documentCounts: {briefCount: 1, researchCount: 2, brainstormingCount: 1, projectDocsCount: 0}
classification:
  projectType: web_app
  domain: fintech
  complexity: high
  projectContext: greenfield
workflowType: 'prd'
---

# Product Requirements Document: naic

**Author:** Guy  
**Date:** 2026-02-03  
**Status:** Draft (Polished)

## Executive Summary

**naic** is an open-source, international investment analysis platform designed to automate the NAIC Stock Selection Guide (SSG) methodology. The platform addresses the "data entry tax" faced by European investors by providing a "One-Click History" for companies in the Swiss (SMI) and German (DAX) markets, alongside major US exchanges. By combining automated data harvesting with web-native logarithmic charting, **naic** shifts the analyst's focus from manual labor to high-level decision-making.

## Success Criteria

### User Success

- **30-Minute Threshold**: Beginner analysts can complete a full NAIC analysis in under 30 minutes.
- **Analysis Confidence**: High trust in automated data accuracy, evidenced by minimal manual overrides.
- **Decision Empowerment**: Users transition from data entry to professional-grade auditing and selection.

### Business & Technical Success

- **Strategic Position**: Established as the premier open-source tool for international (CH/DE) NAIC investors within 6 months.
- **Data Parity**: Automated data matches official reports for 95% of mid/large-cap tickers.
- **System Performance**: "One-Click" 10-year data population completes in under 5 seconds.

## Project Scoping & Phased Development

### Phase 1: Problem-Solving MVP (Must-Have)

Focus: Automating the most labor-intensive markets to validate core value.

- **"One-Click" Harvest**: US, CH (SWX), and DE (DAX) historical data.
- **SSG Visualization**: Web-native logarithmic charting (Sales, EPS, Price).
- **Quality Dashboard**: Pre-calculated ROE, Profit on Sales, and P/E trends.
- **Peer Comparison**: Side-by-side benchmarking for two stocks.

### Phase 2: Growth (Post-MVP)

- **Lifecycle Tracking**: Portfolio rebalancing alerts and saved analysis library.
- **Market Expansion**: Support for French (CAC 40) and UK (FTSE 100) markets.
- **User Accounts**: Cloud-synced watchlist and analysis persistence.

### Phase 3: Vision (Future)

- **The Data Oracle**: AI-powered OCR for PDF annual report data ingestion.
- **Kinetic Charting**: Interactive projection manipulations and trend shadowing.
- **Global Library**: Community-driven "Gold Standard" SSG database.

## User Journeys

### Journey 1: Markus, the Swiss Value Hunter (Primary)

Markus avoids the 2-hour manual entry chore for Swiss stocks. He uses **naic** to instantly generate a 10-year chart for `NESN.SW`, benchmarks it against a German peer, and prepares a clear recommendation for his investment club in 15 minutes.

### Journey 2: Elena, the Club Moderator (Secondary)

Elena uses **naic** to standardize reports across her investment club. By providing identical, data-accurate charts to all members, she shifts club discussions from data verification to business quality.

### Journey 3: David, the Data Steward (Admin)

David monitors API health. When a German provider updates their schema, he receives automated alerts and can submit a fix before users experience any downtime in the "One-Click" engine.

## Domain-Specific Requirements

### Compliance & Regulatory

- **Accounting Standards**: System must handle IFRS vs. GAAP differences for international market extraction.
- **Data Licensing**: Adherence to provider Terms of Service, including necessary attribution for open-source use.
- **Currency Normalization**: Consistent handling of reporting vs. trading currencies to prevent ratio distortion.

### Technical Constraints

- **Data Integrity**: Automated checks for historical gaps or unrealistic outliers.
- **Stock Split Logic**: Mandatory automated handling of splits and reverse splits to maintain chart accuracy.
- **API Management**: Robust handling of rate limits and timeout fallbacks during batch processing.

## Innovation & Novel Patterns

- **"One-Click" Internationalization**: Unique automation for non-US markets that are currently manual-only in most tools.
- **Open-Source Methodology**: A transparent, auditable codebase challenging the "black box" nature of proprietary investment software.
- **Dynamic Normalization**: Native cross-currency peer benchmarking without manual spreadsheet adjustments.

## Project-Type Specification: Web App (SPA)

- **Architecture Style**: Single Page Application (SPA) for stateful, app-like interactivity.
- **Browser Support**: Standard evergreen browsers (Chrome, Firefox, Opera, Safari).
- **SEO/Security**: No public SEO required (local use focus). All transit secured via HTTPS.
- **Implementation Strategy**: Focus on batch historical population over real-time price streaming.

## Functional Requirements (Capability Contract)

### FR#1: Search & Population

- **FR1.1**: Users can search international stocks by ticker (e.g., NESN.SW).
- **FR1.2**: System retrieves 10-year historicals (Sales, EPS, Prices) automatically.
- **FR1.3**: System adjusts data for historical splits and dividends.
- **FR1.4**: System normalizes multi-currency data for side-by-side comparison.

### FR#2: Analysis & Visualization

- **FR2.1**: System calculates 10-year Pre-tax Profit on Sales and ROE.
- **FR2.2**: System calculates 10-year High/Low P/E ranges.
- **FR2.3**: Users can manually override any automated data field.
- **FR2.4**: System renders logarithmic trends for Sales, Earnings, and Price.
- **FR2.5**: System generates trend line projections and "Quality Dashboards."

### FR#3: Reporting & Operations

- **FR3.1**: Users can export standardized SSG reports (PDF/Image).
- **FR3.2**: Users can save/share analysis files for collaborative review.
- **FR3.3**: Admins can monitor API health and flag data integrity errors.

## Non-Functional Requirements (Quality Attributes)

### Performance & Reliability

- **NFR1**: SPA initial load under 2 seconds on standard broadband.
- **NFR2**: "One-Click" 10-year population completes in < 5 seconds (95th percentile).
- **NFR3**: API integration engine maintains 99.9% success rate for primary CH/DE feeds.
- **NFR4**: System flags data gaps explicitly rather than silent interpolation.

### Security

- **NFR5**: All external API communications use encrypted HTTPS protocols.
