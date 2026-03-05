# Sprint Change Proposal: NAIC Methodology Completion

**Date:** 2026-02-19
**Author:** Guy (with BMAD Correct Course workflow)
**Status:** Approved (2026-02-19) — Artifacts updated
**Scope Classification:** Moderate

---

## Section 1: Issue Summary

### Problem Statement

SteadyInvest faithfully implements the NAIC SSG chart (Section 1) but incompletely implements the full NAIC analytical framework defined across 5 canonical BetterInvesting forms. The most significant gap is the absence of dividend yield and total return calculations (SSG Section 5), which is the NAIC's primary output for comparing income and growth stocks. Secondary gaps include incomplete comparison metrics, missing structured decision tests, and a presentation that doesn't match the canonical NAIC form layout.

### Discovery Context

Identified during Story 8b.1 (SSG Handbook Audit and Chart Fixes). The user scanned 5 original BetterInvesting paper forms to PDF and compared them against the current implementation:

- **Stock Selection Guide** (2 pages) — Sections 3-5 partially implemented
- **Stock Checklist** (2 pages) — Educational guided worksheet, not implemented
- **Stock Comparison Guide** (1 page) — ~40% of 30 rows implemented
- **Portfolio Management Guide** (2 pages) — Not implemented (relevant to Epic 9)
- **Stock Selection Guide and Report** (4 pages) — Report section with structured tests not implemented

Reference documents stored in: `docs/NAIC/forms/`

### Evidence

- `steady-invest-logic/src/lib.rs` has no dividend, payout, or yield functions
- `quality_dashboard.rs` shows "N/A" for debt-to-capital
- `valuation_panel.rs` computes forecast prices but not combined total return
- Comparison grid (`compact_analysis_card.rs`) shows ~8 metrics vs. 30 in the NAIC form
- No data model fields for dividends per share or shares outstanding
- SSG Sections 3-5 layout does not match the canonical NAIC form structure

---

## Section 2: Impact Analysis

### Epic Impact

| Epic | Impact | Action |
|------|--------|--------|
| Epic 8b (SSG Methodology Alignment) | None — close as-is | Change status to `done` |
| **Epic 8c (new)** | New epic: NAIC SSG Methodology Completion | Insert between 8b and 9 |
| Epic 9 (Portfolio Management) | Additive — 2 new stories for PMG concepts | Add Stories 9.11, 9.12 |
| Epic 10 (Multi-User Authentication) | None | No change |
| Epic 11 (Collaboration) | None | No change |

**Epic order:** 8b (done) → **8c** → 9 → 10 → 11

**Rationale for 8c before 9:** Analysis methodology completeness (dividend yield, total return, structured tests) directly improves the quality of portfolio decisions in Epic 9. The NAIC investor workflow is: analyze first, then manage.

### Story Impact

**Epic 8c — 7 new stories:**

| Story | Description | Size | Dependencies |
|-------|-------------|------|--------------|
| 8c.1 | Data Model Expansion & Dividend Harvest | M | None (epic entry point) |
| 8c.2 | Dividend Yield, Payout Ratio & Total Return | M | 8c.1 |
| 8c.3 | 5-Tier P/E Breakdown & Price Zone Ranges | S | 8c.1 |
| 8c.4a | NAIC Section Structure & P/E History Table | M | 8c.2, 8c.3 |
| 8c.4b | Section 4-5 Derivation Layouts | M | 8c.4a |
| 8c.5 | Structured Management & Safety Tests | M | 8c.2, 8c.3 |
| 8c.6 | Enriched Stock Comparison Guide Grid | L | 8c.4b, 8c.5 |

**Dependency graph:**
```
8c.1 → 8c.2 ──┐
  │            ├→ 8c.4a → 8c.4b ──┐
  └──→ 8c.3 ──┤                   ├→ 8c.6
               └→ 8c.5 ───────────┘
```

Note: 8c.2 and 8c.3 are independent of each other. 8c.4a/8c.4b and 8c.5 are independent of each other.

**Epic 9 — 2 new stories (additive):**

| Story | Description | Dependencies |
|-------|-------------|--------------|
| 9.11 | P/E Guide Lines & Buy/Sell Thresholds | Epic 8c (P/E calculations) |
| 9.12 | Quarterly P/E Tracking & Price-P/E Chart | Epic 8c (P/E calculations) |

### Artifact Conflicts

| Artifact | Impact | Status |
|----------|--------|--------|
| PRD | Add FR2.7-2.11; expand FR4.3 | Action needed |
| Architecture | Additive data model + logic crate expansion, no conflicts | Action needed (documentation) |
| UI/UX Spec | Extend existing panels, add new sections | Action needed (minor) |
| Epics Document | Add Epic 8c (7 stories); add Stories 9.11-9.12 | Action needed |
| Sprint Status | Close 8b, add 8c entries, add 9.11-9.12 | Action needed |
| Change Requests | Items #2 and #3 addressed by Epic 8c | Update |
| Testing | New golden tests + comparison E2E updates | Action needed |
| CI/CD | No impact | No change |
| Database Schema | No impact (snapshot_data is JSON) | No change |

### Technical Impact

- **Data model:** `HistoricalYearlyData` gains 2 optional fields (`dividend_per_share`, `shares_outstanding`). Non-breaking — existing data without dividends continues to work.
- **Logic crate:** ~9 new functions in `steady-invest-logic` following Cardinal Rule. All with golden tests.
- **Harvest service:** Extended to fetch dividend/shares data if provider supports it. Manual override fallback otherwise.
- **Frontend:** Valuation panel restructured to NAIC Sections 3-5 format. New comparison table component. Subtle NAIC color accents via CSS variables.
- **Backend API:** `ComparisonSnapshotSummary` struct enriched with additional metrics. No new endpoints.
- **No architectural changes.** All work follows established patterns.

---

## Section 3: Recommended Approach

### Selected Path: Direct Adjustment

Add new Epic 8c and extend Epic 9 within the existing plan structure.

### Rationale

1. **No architectural changes needed** — data model expansion is additive (optional fields), logic crate follows Cardinal Rule patterns, frontend extends existing components
2. **Low risk** — adding calculations and display, not restructuring anything. All new logic goes into `steady-invest-logic` with golden tests against NAIC reference material
3. **High value** — completing the NAIC methodology makes SteadyInvest a *complete* SSG implementation rather than a chart-only tool. The "5-Year Potential" total return is the most important NAIC output for investment decisions
4. **Natural sequencing** — 8c (analysis methodology) before 9 (portfolio management) follows the NAIC investor workflow
5. **Educational philosophy** — the BetterInvesting guided assessment approach (FR2.8, FR2.10) transforms SteadyInvest from a data display tool into an investment education platform

### Effort Estimate

- **Epic 8c:** 7 stories (1 Small, 5 Medium, 1 Large)
- **Epic 9 additions:** 2 stories (additive, no change to existing 9.1-9.10)
- **Artifact updates:** PRD, Architecture, Epics, Sprint Status — moderate documentation effort

### Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Data provider doesn't supply dividend/shares data | Medium | Medium | Manual override fallback per FR2.3; research in Story 8c.1 |
| Valuation panel restructure breaks existing layout | Low | Medium | Incremental restructure; existing golden tests validate calculations |
| Epic 8c delays Epic 9 start | Certain | Low | Justified by analysis quality improvement; 8c has no Large-risk stories except 8c.6 |

### Timeline Impact

Epic 9 start delayed by the duration of Epic 8c (~7 stories). This is justified because:
- Total return calculations (8c) improve portfolio decision quality (9)
- Enriched comparison data (8c) benefits portfolio selection (9)
- P/E Guide Lines in 9.11-9.12 build on 8c's P/E work

---

## Section 4: Detailed Change Proposals

### 4.1 PRD — New Functional Requirements

**File:** `_bmad-output/planning-artifacts/prd.md`

**ADD after FR2.6 in Section "FR#2: Analysis & Visualization":**

- **FR2.7**: System calculates dividend yield (most recent fiscal year's total dividend per share / current price), payout ratio (dividend / EPS), and combined 5-year total return estimate (price appreciation potential + average dividend yield) per NAIC SSG Section 5 methodology. Results are expressed as both simple and compound annual rates. When dividend data is unavailable from the provider, the system accepts manual dividend input per FR2.3 and displays a data gap indicator.
- **FR2.8**: System evaluates structured NAIC decision criteria with guided assessment: Three Management Tests (Driving Force via sales expansion, Earned on Sales via profit margin trend, Earned on Equity vs. 10% benchmark) and Three Safety Tests of Price (probability of getting out even, upside-downside ratio >= 3:1, 100% appreciation possible in 5 years) with computed pass/fail suggestions, plain-language explanations referencing NAIC benchmarks, and analyst-overridable conclusions. The system presents each test as a guided question (matching the NAIC Report methodology) rather than just a score.
- **FR2.9**: System displays 5-tier P/E breakdown (Highest, Average High, Average, Average Low, Lowest from last 5 years) and three price zone dollar ranges (Buy zone, Maybe zone, Sell zone) derived from NAIC SSG Section 4C zoning methodology.
- **FR2.10**: System provides a guided analysis summary (inspired by the NAIC Stock Checklist conclusion format) presenting four key investment decisions — sales growth adequacy, earnings growth adequacy, future growth potential, and price acceptability — with contextual explanations and NAIC benchmark comparisons, enabling non-professional investors to reach informed conclusions. Draws on results from FR2.8 structured tests where applicable.
- **FR2.11**: System presents SSG analysis in the canonical NAIC 5-section structure (Visual Analysis, Evaluating Management, Price-Earnings History, Evaluating Risk and Reward, 5-Year Potential) with section numbering, NAIC-standard column layouts, step-by-step calculation derivations, and subtle visual accents echoing the NAIC form color language (green for SSG, red for comparison, blue for portfolio, gold for guided assessment) — enabling NAIC-trained investors to navigate the interface with zero cognitive switching cost.

**MODIFY FR4.3:**

OLD:
> FR4.3: Users can compare projected performance metrics across multiple tickers (not limited to two) in a compact summary view, enabling ranking and selection decisions. Percentage-based metrics (CAGRs, P/E, ROE) display without currency conversion; monetary values convert to a user-selectable base currency using current exchange rates.

NEW:
> FR4.3: Users can compare projected performance metrics across multiple tickers (not limited to two) in a view aligned with the NAIC Stock Comparison Guide, organized into four row groups (Growth Comparisons, Management Comparisons, Price Comparisons, Other Comparisons) enabling ranking and selection decisions. Comparison metrics include: growth rates (historical/projected Sales and EPS), management quality (profit margin, ROE, trends), price analysis (5-tier P/E breakdown, price zone ranges, upside-downside ratio), and yield metrics (current yield, combined estimated yield, payout ratio). Other comparisons include date of source material and exchange. Percentage-based metrics display without currency conversion; monetary values convert to a user-selectable base currency using current exchange rates.

**ADD to FR Coverage Map:**

| FR | Epic | Description |
|----|------|-------------|
| FR2.7 | 8c | Dividend yield, payout, total return |
| FR2.8 | 8c | Structured management & safety tests |
| FR2.9 | 8c | 5-tier P/E and price zone ranges |
| FR2.10 | 8c | Guided analysis summary |
| FR2.11 | 8c | NAIC section structure & visual fidelity |
| FR4.3 expand | 8c | Enriched comparison guide grid |

### 4.2 Epics Document — New Epic 8c

**File:** `_bmad-output/planning-artifacts/epics.md`

**ADD after Epic 8 section, before Epic 9:**

---

## Epic 8c: NAIC SSG Methodology Completion

Complete the NAIC analytical framework beyond the SSG chart by implementing dividend/yield calculations, structured decision tests, the full Stock Comparison Guide metrics, and NAIC-standard visual presentation with step-by-step derivations. Designed to serve non-professional investors through guided assessment aligned with the BetterInvesting educational philosophy.

**FRs covered:** FR2.7, FR2.8, FR2.9, FR2.10, FR2.11, FR4.3 (expansion)
**Depends on:** Epic 8b (completed)
**Prerequisite for:** Epic 9 (total return and enriched comparison data inform portfolio decisions)

### Story 8c.1: Data Model Expansion & Dividend Harvest

As an **analyst**,
I want dividend and shares outstanding data available for my analyses,
So that I can compute yield, payout ratio, and total return per the NAIC methodology.

**Scope:**
- Add `dividend_per_share` (Option<f64>) and `shares_outstanding` (Option<f64>) to `HistoricalYearlyData` in `steady-invest-logic`
- Research data provider availability for these fields
- Extend `harvest.rs` to populate fields when provider supports them
- Manual override fallback for missing data per FR2.3
- Data gap indicator when dividend data is unavailable
- Golden tests for data model changes

### Story 8c.2: Dividend Yield, Payout Ratio & 5-Year Total Return

As an **analyst**,
I want the system to calculate dividend yield, payout ratio, and combined total return,
So that I can evaluate the complete 5-year potential of a stock per NAIC SSG Section 5.

**Scope:**
- Implement in `steady-invest-logic`: `calculate_dividend_yield()`, `calculate_payout_ratio()`, `calculate_total_return_simple()`, `calculate_total_return_compound()`
- Add Section 3 columns F (Dividend Per Share), G (% Payout), H (% High Yield) to P/E history table
- Display Section 5 results: Current Yield (5A), Average Yield Over Next 5 Years (5B), Combined Estimated Annual Return (5C)
- Express results as both simple and compound annual rates
- Golden tests against NAIC Handbook examples
- FR2.7

### Story 8c.3: 5-Tier P/E Breakdown & Price Zone Ranges

As an **analyst**,
I want to see the full P/E range breakdown and explicit buy/maybe/sell price zones,
So that I can evaluate price history and identify entry/exit points per the NAIC methodology.

**Scope:**
- Implement in `steady-invest-logic`: `calculate_pe_breakdown_5tier()` returning Highest, Average High, Average, Average Low, Lowest P/E from last 5 years
- Implement `calculate_price_zones()` returning Buy/Maybe/Sell dollar ranges from SSG Section 4C zoning
- Display in SSG valuation panel and comparison grid
- Golden tests
- FR2.9

### Story 8c.4a: NAIC Section Structure & P/E History Table

As an **analyst**,
I want the SSG analysis presented in the canonical NAIC 5-section layout,
So that I can navigate the interface using my existing NAIC knowledge.

**Scope:**
- Restructure valuation panel into NAIC Sections 3/4/5 with section numbering and titles
- Section 3: 8-column P/E history table (A: High Price, B: Low Price, C: EPS, D: P/E High, E: P/E Low, F: Dividend, G: % Payout, H: % High Yield) with TOTAL, AVERAGE rows and Average P/E Ratio summary
- Subtle NAIC color accents via CSS custom properties: `--accent-ssg: #2E7D32` (green), `--accent-comparison: #C62828` (red), `--accent-portfolio: #1565C0` (blue), `--accent-checklist: #F9A825` (gold). Applied as thin 3px left-border on section headers.
- FR2.11 (partial)

### Story 8c.4b: Section 4-5 Derivation Layouts

As an **analyst**,
I want to see step-by-step calculation derivations for risk/reward and total return,
So that I understand exactly how forecast prices and returns are computed.

**Scope:**
- Section 4 "show your work" derivation chain: "Avg. High P/E (X) × Est. High EPS ($Y) = Forecast High Price ($Z)" — for both high and low sides
- Section 4 zoning display: computed Buy/Maybe/Sell ranges with current price position indicator
- Section 4D upside-downside ratio derivation
- Section 4E price target (% appreciation)
- Section 5 total return derivation: Current Yield + Average Yield + Price Appreciation = Combined Return (simple → compound conversion)
- Intermediate values surfaced from logic crate (no new computations — display of existing intermediates)
- FR2.11 (completion)

### Story 8c.5: Structured Management & Safety Tests

As an **analyst**,
I want structured pass/fail tests and a guided analysis summary,
So that I can make disciplined investment decisions using the NAIC framework.

**Scope:**
- Implement in `steady-invest-logic`: `evaluate_management_tests()` and `evaluate_safety_tests()` returning `SuggestedAssessment` enum (Pass, Fail, Borderline) per test
- Three Management Tests: I (Driving Force — sales expansion rate), II (Earned on Sales — profit margin trend), III (Earned on Equity — vs. 10% benchmark)
- Three Safety Tests: I (Probability of Getting Out Even — has stock traded at current price in past 5 years?), II (Upside-Downside >= 3:1), III (100% appreciation possible in 5 years?)
- Plain-language explanations via `generate_guided_narrative()` in logic crate (template-driven, no LLM)
- Analyst-overridable conclusions stored in snapshot_data JSON
- Guided analysis summary: four Checklist-style decisions (sales adequate?, earnings adequate?, future growth meets objective?, price acceptable?)
- Gold accent on guided assessment section
- FR2.8, FR2.10

### Story 8c.6: Enriched Stock Comparison Guide Grid

As an **analyst**,
I want the comparison view to match the NAIC Stock Comparison Guide layout,
So that I can compare stocks using the full NAIC methodology.

**Scope:**
- New comparison table component (separate from compact_analysis_card, which remains for Library view)
- Four row groups matching NAIC form: Growth Comparisons (rows 1-4), Management Comparisons (rows 5-7), Price Comparisons (rows 8-23), Other Comparisons (rows 24-30)
- Include all metrics from 8c.2 (yield), 8c.3 (P/E breakdown, zones), 8c.5 (test results) in comparison
- Add: date of source material, exchange (where traded) in Other Comparisons
- Enrich backend `ComparisonSnapshotSummary` with additional metrics
- Graceful "N/A" for rows without data source (e.g., insider ownership, potential dilution)
- Red NAIC color accent on comparison section headers
- Update PDF report to include enriched comparison data
- FR4.3 (expansion)

---

### 4.3 Epics Document — Epic 9 Additions

**File:** `_bmad-output/planning-artifacts/epics.md`

**ADD after Story 9.10, before Epic 9 retrospective:**

### Story 9.11: P/E Guide Lines & Buy/Sell Thresholds

As an **investor** monitoring my portfolio holdings,
I want rolling P/E guide lines computed from historical averages,
So that I have objective buy/sell thresholds for ongoing monitoring.

**Scope:**
- Per NAIC Portfolio Management Guide Section 1:
  - Low P/E Guide Line = average of (sum of avg High + Low P/E) / 2 for previous 5 years
  - High P/E Guide Line = Low P/E Guide Line × 1.5
  - Per year: "Consider Buying Below" = Low P/E Guide Line × current EPS
  - Per year: "Consider Selling Above" = High P/E Guide Line × current EPS
- Implement in `steady-invest-logic` (Cardinal Rule)
- Display in Portfolio Dashboard (Story 9.5) as per-holding monitoring thresholds
- Guide lines update as new quarterly earnings data becomes available
- Blue NAIC PMG color accent per FR2.11

### Story 9.12: Quarterly P/E Tracking & Price-P/E Chart

As an **investor** performing quarterly portfolio reviews,
I want to track quarterly earnings, price, and P/E ratio over time,
So that I can monitor valuation trends at each review period.

**Scope:**
- Per NAIC Portfolio Management Guide Sections 3-4:
  - Section 3: Table tracking per quarter: 3-month EPS, trailing 4Q total earnings, price at review date, P/E ratio at review date (supports up to 3 review dates per row)
  - Section 4: Dual-axis monthly chart plotting market price (solid line) and P/E ratio (dashed line) over 5 years
- Integrate with Portfolio Dashboard (Story 9.5) as per-holding monitoring tool
- Blue NAIC PMG color accent per FR2.11

**UPDATE Epic 9 execution strategy:**
> Execution strategy: Three-sprint split — Sprint A (9.1-9.5 portfolio foundation), Sprint B (9.6-9.10 risk discipline + watchlist), Sprint C (9.11-9.12 NAIC portfolio monitoring).

### 4.4 Sprint Status Update

**File:** `_bmad-output/implementation-artifacts/sprint-status.yaml`

Changes:
- Epic 8b: `in-progress` → `done`
- Add Epic 8c block with 7 stories (all `backlog`)
- Add Stories 9.11, 9.12 (both `backlog`)

### 4.5 Architecture Document Update

**File:** `_bmad-output/planning-artifacts/architecture.md`

Add to Data Architecture section:
- `HistoricalYearlyData` expansion: `dividend_per_share` (Option<f64>), `shares_outstanding` (Option<f64>)
- Optional fields — non-breaking, existing data continues to work

Add to Implementation Patterns section:
- ~9 new functions in `steady-invest-logic` (Cardinal Rule compliant)
- Narrative templates in logic crate (shared frontend + PDF)
- `SuggestedAssessment` enum for management/safety tests
- CSS custom properties for NAIC color accents

Add FR2.7-2.11 to FR Coverage Map.

### 4.6 Change Request Cleanup

**File:** `docs/change_request.md`

- Item #2 (buy/hold/sell zone display): Addressed by Epic 8c (FR2.9 price zone dollar ranges)
- Item #3 (stock bar visibility): Addressed by Epic 8c (FR2.11 NAIC visual restructure)

---

## Section 5: Implementation Handoff

### Scope Classification: Moderate

Requires backlog reorganization (new epic, PRD updates, sprint status) but no architectural redesign.

### Handoff Plan

| Role | Agent | Responsibility |
|------|-------|----------------|
| Product Manager | John (📋) | Update PRD with FR2.7-2.11 and FR4.3 expansion |
| Scrum Master | Bob (🏃) | Update sprint-status.yaml; create stories via `/bmad-bmm-create-story` |
| Architect | Winston (🏗️) | Update architecture doc (documentation only — no design changes) |
| Developer | Amelia (💻) | Implement Epic 8c stories after story creation |

### Recommended Workflow Sequence

1. **Apply artifact updates** — PRD, Architecture, Epics, Sprint Status (this session or next)
2. **Close Epic 8b** — Optional retrospective via `/bmad-bmm-retrospective`
3. **Sprint Planning for Epic 8c** — `/bmad-bmm-sprint-planning`
4. **Create Story 8c.1** — `/bmad-bmm-create-story` (data model + harvest research)
5. **Implement Epic 8c** — Story cycle: Create → Dev → Code Review → next story
6. **Epic 8c Retrospective** — Optional
7. **Sprint Planning for Epic 9** — `/bmad-bmm-sprint-planning` (includes new stories 9.11-9.12)

### Success Criteria

- All 5 NAIC SSG sections fully implemented with NAIC-standard layout
- Dividend yield and combined total return computable for stocks with dividend data
- Structured Management and Safety Tests with guided assessment
- Stock Comparison Guide grid covering all 4 row groups
- Golden tests validating calculations against NAIC Handbook examples
- NAIC form color accents visible on section headers
- Existing functionality unaffected (non-breaking changes)
