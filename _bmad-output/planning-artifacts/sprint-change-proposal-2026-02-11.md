# Sprint Change Proposal: Post-MVP Evolution

**Date:** 2026-02-11
**Author:** Bob (Scrum Master)
**Scope Classification:** Moderate
**Status:** Approved (2026-02-11)

---

## Section 1: Issue Summary

After successful delivery of all 6 MVP epics (23 stories), the product vision has expanded from an SSG analysis tool to a complete investment management platform. The revised PRD introduces a "Three Moments" framework (Analyze, Buy Smart, Stay Balanced) with 13 new functional requirements across 3 new development phases.

Planning artifacts (PRD, Architecture, UX Design) were revised on 2026-02-10. The epics document and sprint status have not yet been updated. Minor PRD quality issues identified by the validation report need fixing before epic creation.

**Trigger:** Post-Epic 6 retrospective and product vision planning session (2026-02-10).

**Nature:** Product evolution (not a bug, failure, or strategic pivot).

---

## Section 2: Impact Analysis

### Epic Impact

| Epic | Status | Impact |
|------|--------|--------|
| Epics 1-6 (MVP) | Done | No changes needed |
| Epic 7 (new) | Backlog | Phase 1: Fix & Foundation |
| Epic 8 (new) | Backlog | Phase 2: Portfolio & Watchlist |
| Epic 9 (new) | Backlog | Phase 3: Multi-User |
| Epic 10 (Collaboration) | Deferred | Explicitly deferred until Phase 3 validated |

**Dependencies:** Epic 7 → Epic 8 → Epic 9 (sequential).

### Story Impact

No existing stories are affected. All new stories will be created as part of Epics 7-9.

**New Epic FR Coverage:**

| Epic | FRs Covered |
|------|-------------|
| Epic 7: Fix & Foundation | FR3.1 fix, FR4.1-FR4.3, NFR7 |
| Epic 8: Portfolio & Watchlist | FR5.1-FR5.7, FR6.1-FR6.2, NFR6 |
| Epic 9: Multi-User | FR7.1-FR7.2 |

### Artifact Conflicts

| Artifact | Status | Action |
|----------|--------|--------|
| PRD | Needs minor fixes | 5 approved change proposals |
| Architecture | Already revised | No further changes |
| UX Design | Already revised | No further changes |
| Epics Document | Needs update | Add Epics 7-9 with story breakdowns |
| Sprint Status | Needs update | Add new epic/story entries |

### Technical Impact

- New database migrations required (Epic 7 introduces analysis persistence schema)
- New E2E test coverage needed for persistence, portfolio, and auth flows
- CI/CD pipeline may need DB migration step
- No changes to existing codebase required until Epic 7 implementation begins

---

## Section 3: Recommended Approach

**Selected:** Direct Adjustment

**Rationale:** This is a clean product evolution from a fully delivered MVP. No rework, rollback, or scope reduction is needed. Planning artifacts are 90% aligned. The standard BMAD workflow cycle (Create Epics → Readiness Check → Sprint Planning → Story Cycle) can resume after minor PRD fixes.

**Alternatives Considered:**
- Rollback: Not viable — all MVP work is solid and foundational
- MVP Review: Not viable — MVP is delivered and working

**Effort:** Low for course correction; moderate-to-high for Epic 7-9 implementation
**Risk:** Low — extending proven patterns on a stable foundation
**Timeline Impact:** None on MVP (delivered). New phases add scope but are cleanly phased.

---

## Section 4: Detailed Change Proposals

### 4.1 PRD: FR#4 Analysis Persistence — Remove FR4.4, Refine FR4.1-FR4.2

**OLD:**
```
- **FR4.1**: System stores completed analyses in the database with ticker, date, and snapshot data.
- **FR4.2**: Users can retrieve past analyses for the same ticker and compare thesis evolution across time (e.g., how projections changed between quarterly reviews).
- **FR4.3**: Users can compare projected performance metrics across multiple tickers...
[FR4.4 existed: "Database schema includes user_id column"]
```

**NEW:**
```
- **FR4.1**: System stores completed analyses in the database with ticker, date, and snapshot data, enabling retrieval and comparison.
- **FR4.2**: Users can retrieve past analyses for the same ticker and compare thesis evolution across time (e.g., side-by-side metric deltas between quarterly reviews).
- **FR4.3**: Users can compare projected performance metrics across multiple tickers...
[FR4.4 removed — architecture decision, documented in architecture.md]
```

### 4.2 PRD: Remove NFR6

**OLD:**
```
- **NFR6**: System supports seamless migration to multi-user and multi-portfolio mode without data loss or schema-breaking changes.
```

**NEW:** Removed. Application has no deployed users or production data. Multi-user migration can freely redesign the schema.

**Renumbering:**
- NFR7 (portfolio operations < 1s) → NFR6
- NFR8 (analysis retrieval < 2s, comparison < 3s) → NFR7

### 4.3 PRD: FR#5 Portfolio Management — Add Configuration FR, Sharpen Position Sizing

**OLD:**
```
- **FR5.1**: Users can create multiple portfolios with independent names and configurations.
- **FR5.2**: Users can record stock purchases (ticker, quantity, price, date) within a portfolio.
- **FR5.3**: System calculates current portfolio composition and per-stock allocation percentages.
- **FR5.4**: System detects over-exposure when a single stock exceeds its portfolio's configurable threshold.
- **FR5.5**: System suggests position sizes that maintain diversification targets.
- **FR5.6**: System prompts trailing stop loss setup at purchase time.
```

**NEW:**
```
- **FR5.1**: Users can create multiple portfolios with independent names.
- **FR5.2**: Users can configure per-portfolio parameters: maximum per-stock allocation percentage, rebalancing thresholds, and risk rules. Each portfolio's configuration is independent.
- **FR5.3**: Users can record stock purchases (ticker, quantity, price, date) within a portfolio.
- **FR5.4**: System calculates current portfolio composition and per-stock allocation percentages.
- **FR5.5**: System detects over-exposure when a single stock exceeds its portfolio's configured maximum allocation threshold.
- **FR5.6**: System suggests a maximum buy amount for a given stock based on the portfolio's configured per-stock allocation threshold and current holdings (e.g., given a CHF 100K portfolio with a 10% max-per-stock rule and 0% current exposure, the system suggests a max buy of CHF 10K).
- **FR5.7**: System prompts trailing stop loss setup at purchase time.
```

### 4.4 PRD: FR7.1 — Sharpen Authentication

**OLD:**
```
- **FR7.1**: Users can register and authenticate with secure credentials.
```

**NEW:**
```
- **FR7.1**: Users can register and authenticate using username/password with industry-standard password hashing (e.g., bcrypt/argon2).
```

### 4.5 PRD: Add Explicit Deferral Section

**Add after Phase 4 in Project Scoping & Phased Development:**

```
### Explicitly Deferred

The following features from the original product brief are not planned for any current phase:

- **Data Oracle / OCR**: AI-powered PDF annual report data ingestion. May be revisited if manual data entry becomes a significant user pain point again.
- **Market Expansion**: French (CAC 40) and UK (FTSE 100) market support. May be revisited based on user demand.
- **Collaboration (Phase 4)**: Shared analyses, team portfolios, and community library. Deferred until Phase 3 (Multi-User) is delivered and validated.
```

---

## Section 5: Implementation Handoff

### Scope Classification: Moderate

Backlog reorganization needed — new epics and stories must be created before development can resume.

### Handoff Plan

| Step | Agent | Workflow | Priority |
|------|-------|----------|----------|
| 1. Apply PRD edits | John, Product Manager | `/bmad-bmm-edit-prd` | High |
| 2. Create Epics 7-9 | John, Product Manager | `/bmad-bmm-create-epics-and-stories` | High |
| 3. Readiness check | Winston, Architect | `/bmad-bmm-check-implementation-readiness` | High |
| 4. Sprint planning | Bob, Scrum Master | `/bmad-bmm-sprint-planning` | High |
| 5. Story cycle | Bob / Amelia | `/bmad-bmm-create-story` → `/bmad-bmm-dev-story` → `/bmad-bmm-code-review` | Normal |

### Success Criteria

- PRD passes validation with no warning-level findings on modified FRs/NFRs
- Epics 7-9 defined with story breakdowns covering all new FRs
- Implementation readiness check passes
- Sprint plan created for Epic 7
- Development resumes with standard story cycle
