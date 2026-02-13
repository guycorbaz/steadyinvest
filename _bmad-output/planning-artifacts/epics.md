---
stepsCompleted: [step-01-validate-prerequisites, step-02-design-epics, step-03-create-stories, step-04-final-validation]
inputDocuments:
  - '_bmad-output/planning-artifacts/prd.md'
  - '_bmad-output/planning-artifacts/architecture.md'
  - '_bmad-output/planning-artifacts/ux-design-specification.md'
  - '_bmad-output/planning-artifacts/sprint-change-proposal-2026-02-11.md'
  - '_bmad-output/implementation-artifacts/epic-6-retro-2026-02-10.md'
---

# SteadyInvest - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for SteadyInvest, decomposing the requirements from the PRD, UX Design, and Architecture into implementable stories for the post-MVP evolution (Phases 1-4).

**Context:** Epics 1-6 (MVP) are delivered and complete. This document covers Epics 7+ for the post-MVP roadmap.

## Requirements Inventory

### Functional Requirements

**MVP — Delivered (Epics 1-6):**

- FR1.1: Users can search international stocks by ticker (e.g., NESN.SW).
- FR1.2: System retrieves 10-year historicals (Sales, EPS, Prices) automatically.
- FR1.3: System adjusts data for historical splits and dividends.
- FR1.4: System normalizes multi-currency data for comparison, supporting a user-selectable base currency for cross-market analysis.
- FR1.5: System flags all detected data gaps explicitly to the user rather than silently interpolating missing values.
- FR2.1: System calculates 10-year Pre-tax Profit on Sales and ROE.
- FR2.2: System calculates 10-year High/Low P/E ranges.
- FR2.3: Users can manually override any automated data field.
- FR2.4: System renders logarithmic trends for Sales, Earnings, and Price.
- FR2.5: System generates trend line projections and "Quality Dashboards."
- FR2.6: Users can interactively manipulate projection trend lines; valuation metrics update in real time.
- FR3.1: Users can export standardized SSG reports (PDF/Image) from the UI navigation menu. **NOTE: Built but needs UI routing fix (Epic 6 retro action item #4).**
- FR3.2: Users can save/load analysis files for review.
- FR3.3: Admins can monitor API health and flag data integrity errors.
- FR3.4: Users can lock an analysis thesis, capturing a timestamped snapshot of all projections and overrides for future reference.

**Phase 1 — Fix & Foundation (New):**

- FR4.1: System stores completed analyses in the database with ticker, date, and snapshot data, enabling retrieval and comparison.
- FR4.2: Users can retrieve past analyses for the same ticker and compare thesis evolution across time (e.g., side-by-side metric deltas between quarterly reviews).
- FR4.3: Users can compare projected performance metrics across multiple tickers (not limited to two) in a compact summary view, enabling ranking and selection decisions. Percentage-based metrics display without currency conversion; monetary values convert to a user-selectable base currency.

**Phase 2 — Portfolio & Watchlist (New):**

- FR5.1: Users can create multiple portfolios with independent names.
- FR5.2: Users can configure per-portfolio parameters: maximum per-stock allocation percentage, rebalancing thresholds, and risk rules.
- FR5.3: Users can record stock purchases (ticker, quantity, price, date) within a portfolio.
- FR5.4: System calculates current portfolio composition and per-stock allocation percentages.
- FR5.5: System detects over-exposure when a single stock exceeds its portfolio's configured maximum allocation threshold.
- FR5.6: System suggests a maximum buy amount for a given stock based on the portfolio's configured per-stock allocation threshold and current holdings.
- FR5.7: System prompts trailing stop loss setup at purchase time.
- FR6.1: Users can maintain a watchlist of stocks with notes and target buy prices.
- FR6.2: Watchlist entries can link to saved SSG analyses for quick reference.

**Phase 3-4 — Multi-User & Collaboration (New):**

- FR7.1: Users can register and authenticate using username/password with industry-standard password hashing.
- FR7.2: Each user has a personal workspace with their own analyses, portfolios, and watchlists.
- FR7.3: Users can share analyses with other users or groups (Phase 4).

### NonFunctional Requirements

- NFR1: Application initial load under 2 seconds on 10 Mbps broadband, as measured by Lighthouse performance audit.
- NFR2: "One-Click" 10-year population completes in < 5 seconds (95th percentile), as measured by application performance logs.
- NFR3: API integration engine maintains 99.9% success rate for primary CH/DE feeds, as measured by structured application logs over rolling 30-day windows.
- NFR4: All external API communications use encrypted HTTPS protocols, as verified by TLS certificate validation and network traffic inspection.
- NFR5: Portfolio operations (position sizing, exposure checks) complete in < 1 second for portfolios up to 100 holdings, as measured by application performance logs.
- NFR6: Any historical analysis snapshot retrieves in < 2 seconds; multi-stock comparison queries complete in < 3 seconds for up to 20 analyses, as measured by application performance logs.

### Additional Requirements

**From Architecture:**

- Brownfield project — no starter template needed. Existing Loco + Leptos + SeaORM + MariaDB stack continues.
- Database schema expansion: `analysis_snapshots`, `comparison_sets`, `comparison_set_items` (Phase 1); `portfolios`, `portfolio_holdings`, `watchlist_items` (Phase 2).
- Data migration: `locked_analyses` → `analysis_snapshots` with column mapping (`analysis_data` → `snapshot_data`, `locked_at` → `captured_at`, default `user_id = 1`).
- All new tables include `user_id` column from day one (multi-user readiness).
- Append-only snapshot model — saves create new rows, never update existing.
- Immutability contract — locked analyses reject modification at controller layer.
- Pre-migration backup script: `scripts/migrate-safe.sh`.
- New backend service directory: `backend/src/services/` (snapshot_service, comparison_service, portfolio_service, exposure_service).
- New frontend state module: `frontend/src/state/` with global signals (Active Portfolio, Currency Preference).
- API expansion: ~29 new routes across Phases 1-3.
- Cardinal Rule: all calculation logic in `crates/steady-invest-logic` — never duplicated.
- Open decision: Static Chart Image Capture — recommended Option A (lock-time browser capture).
- Open decision: Performance test harness — recommended Option C (timed assertions in integration tests) for API NFRs.

**From UX Design:**

- Five Core Views: Analysis (`/`), Comparison (`/compare`), Library (`/library`), Portfolio (`/portfolio`), Watchlist (`/watchlist`).
- Command Strip navigation expansion from 4 to 5+ destinations.
- Compact Analysis Card component for Comparison grid and Library browse.
- History Timeline Sidebar for thesis evolution within Analysis view.
- Portfolio Dashboard with holdings table, exposure bars (CSS-based, not canvas), multi-portfolio selector.
- Position Sizing Calculator (inline and panel modes).
- Wisdom Card (advisory tone, inline and panel modes).
- Inline portfolio context: progressive density status line in Analysis view.
- Currency Selector (Phase 2 — Phase 1 displays native currencies).
- Watchlist View with price-reached indicators and linked analyses.
- Mobile review mode: read-only, static chart images, no kinetic charting.
- Responsive breakpoints: Desktop Wide (1280+), Standard (1024-1279), Tablet (768-1023), Mobile (<767).
- WCAG 2.1 Level AA accessibility for all new views.
- Static chart snapshot capture at thesis lock time for mobile and PDF use.

**From Epic 6 Retrospective (Action Items):**

- PDF Export UI Accessibility: ensure PDF export reachable from UI navigation menu.
- Increase SSG Chart Height for readability.
- Add Persistent Legend Below SSG Chart.
- Validate CI/CD E2E Pipeline (.github/workflows/e2e.yaml).

### FR Coverage Map

| FR | Epic | Description |
|----|------|-------------|
| FR3.1 fix | 7 | PDF export UI routing fix |
| FR4.1 | 7 | Analysis persistence to database |
| FR4.2 | 8 | Thesis evolution / history comparison |
| FR4.3 | 8 | Multi-stock comparison ranked grid |
| FR5.1 | 9 | Multiple portfolios with names |
| FR5.2 | 9 | Per-portfolio configuration |
| FR5.3 | 9 | Record stock purchases |
| FR5.4 | 9 | Portfolio composition calculation |
| FR5.5 | 9 | Over-exposure detection |
| FR5.6 | 9 | Position sizing suggestions |
| FR5.7 | 9 | Stop loss prompting |
| FR6.1 | 9 | Watchlist with notes and target prices |
| FR6.2 | 9 | Watchlist linked to saved analyses |
| FR7.1 | 10 | User authentication |
| FR7.2 | 10 | Personal workspaces |
| FR7.3 | 11 | Shared analyses (deferred) |

**Coverage: 15/15 new FRs mapped + 1 MVP fix (FR3.1). 0 orphans.**

## Epic List

### Epic 7: Analysis Persistence & MVP Fixes (Phase 1a)

Users can save analyses to a database, browse their analysis library, and access all MVP features without gaps. Foundation infrastructure (schema, migration, exchange rate service, static chart capture) is established for subsequent epics.

**FRs covered:** FR3.1 (UI fix), FR4.1
**NFRs covered:** NFR6 (snapshot retrieval < 2s)
**Additional scope:**
- Pre-migration backup script (`scripts/migrate-safe.sh`) — first story, non-negotiable
- Database schema: `analysis_snapshots`, `comparison_sets`, `comparison_set_items` tables with `user_id` columns
- Data migration: `locked_analyses` → `analysis_snapshots`
- Snapshot CRUD API (`/api/v1/snapshots`)
- Exchange rate service + endpoint (`/api/v1/exchange-rates`) — foundation for Epic 8 comparison currency conversion
- Static chart image capture at thesis lock time (Option A: browser canvas export, non-blocking, nullable column)
- Library view (`/library`) with Compact Analysis Cards
- Command Strip expansion (Library entry)
- SSG chart height increase
- Persistent legend below SSG chart
- PDF export UI navigation routing fix
- CI/CD E2E pipeline validation

## Epic 7: Analysis Persistence & MVP Fixes

Users can save analyses to a database, browse their analysis library, and access all MVP features without gaps. Foundation infrastructure (schema, migration, exchange rate service, static chart capture) is established for subsequent epics.

### Story 7.1: MVP Fixes — PDF Export, Chart Height & Legend

As an **analyst**,
I want PDF export accessible from the UI menu, a taller SSG chart, and a persistent legend,
So that I can use all MVP features without gaps and read the chart more easily.

**Acceptance Criteria:**

**Given** the user is on the Analysis view with a populated SSG chart
**When** the user opens the Command Strip or navigation menu
**Then** a PDF/Image export action is visible and clickable
**And** clicking it produces the same PDF export that was built in Story 4.2

**Given** the SSG chart is rendered with a 10-year, 6-series dataset
**When** the page loads on a standard desktop (1280px+)
**Then** the chart renders at an increased min-height (at least 500px) for better readability of overlapping logarithmic series

**Given** the SSG chart is rendered with any dataset
**When** the chart finishes rendering
**Then** a persistent legend is visible below the chart showing series names and colors (Sales, EPS, High Price, Low Price, and projection lines)
**And** the legend is always visible without requiring mouse hover interaction

**Given** the Analysis view with chart, legend, and PDF export
**When** rendered at all four breakpoints (desktop wide 1280px+, desktop standard 1024-1279px, tablet 768-1023px, mobile <767px)
**Then** all elements render correctly without layout breakage or overflow

### Story 7.2: Pre-Migration Backup & Snapshot Schema

As an **analyst**,
I want my existing locked analyses preserved safely in a new database schema,
So that my previous work is not lost as the platform evolves.

**Acceptance Criteria:**

**Given** the developer runs `scripts/migrate-safe.sh`
**When** the script executes
**Then** a timestamped MariaDB backup is created before any migration runs
**And** the script then invokes `cargo loco db migrate`
**And** the script exits with a non-zero code if the backup fails (migration does not proceed)

**Given** the migration runs successfully
**When** the `analysis_snapshots` table is created
**Then** it includes columns: `id`, `user_id` (FK to users, default 1), `ticker_id` (FK to tickers), `snapshot_data` (JSON), `thesis_locked` (bool), `chart_image` (nullable BLOB/MEDIUMBLOB), `notes` (nullable text), `captured_at` (datetime)
**And** appropriate indexes exist on `user_id`, `ticker_id`, and `captured_at`

**Given** existing data in the `locked_analyses` table
**When** the data migration script runs
**Then** all rows are copied to `analysis_snapshots` with: `analysis_data` → `snapshot_data`, `locked_at` → `captured_at`, `thesis_locked` = true, `user_id` = 1
**And** the `locked_analyses` table is dropped after successful migration
**And** the migration script includes comments documenting the column mapping

### Story 7.3: Analysis Snapshot API

As an **analyst**,
I want to save, retrieve, and manage my analysis snapshots via the API,
So that my analyses persist in the database and are retrievable at any time.

**Acceptance Criteria:**

**Given** the user completes an analysis (with or without locking the thesis)
**When** a `POST /api/v1/snapshots` request is sent with ticker_id, snapshot_data (JSON), thesis_locked (bool), and optional notes
**Then** a new row is created in `analysis_snapshots` (append-only — never updates existing rows)
**And** the response returns the created snapshot with its ID and `captured_at` timestamp
**And** the operation completes in < 2 seconds (NFR6)

**Given** saved snapshots exist in the database
**When** a `GET /api/v1/snapshots` request is sent with optional query filters (ticker_id, thesis_locked)
**Then** matching snapshots are returned ordered by `captured_at` descending
**And** the response includes id, ticker_id, thesis_locked, notes, and captured_at (not the full snapshot_data for list queries)

**Given** a specific snapshot ID
**When** a `GET /api/v1/snapshots/:id` request is sent
**Then** the full snapshot including `snapshot_data` JSON is returned
**And** retrieval completes in < 2 seconds (NFR6)

**Given** an unlocked snapshot exists
**When** a `DELETE /api/v1/snapshots/:id` request is sent
**Then** the snapshot is soft-deleted
**And** locked snapshots reject deletion with a 403 response and message "Locked analyses cannot be deleted"

**Given** a locked snapshot exists
**When** any `PUT` or `PATCH` request is sent to modify it
**Then** the request is rejected with a 403 response enforcing the immutability contract

### Story 7.4: Static Chart Image Capture at Lock Time

As an **analyst**,
I want a static image of my SSG chart captured when I lock a thesis,
So that I can view the chart on mobile and include it in PDF exports without re-rendering.

**Acceptance Criteria:**

**Given** the user has a populated SSG chart and clicks "Lock Thesis"
**When** the lock action is triggered
**Then** the frontend captures the current chart as a PNG image via the charming/ECharts instance export API (`echartsInstance.getDataURL()` or equivalent WASM binding) — note: the capture mechanism is charting-library-dependent, not raw canvas `toDataURL()`
**And** the image bytes are included in the `POST /api/v1/snapshots` payload
**And** the image is stored in the `chart_image` column of the snapshot row

**Given** the canvas export fails for any reason (browser API unavailable, chart not rendered)
**When** the lock action is triggered
**Then** the snapshot saves successfully without a chart image (`chart_image` = NULL)
**And** the failure is logged to the browser console for debugging
**And** the user's thesis lock workflow is not blocked or interrupted

**Given** a snapshot with a stored chart image
**When** the snapshot is retrieved via `GET /api/v1/snapshots/:id`
**Then** the chart image is available as a base64-encoded field or a separate image endpoint

### Story 7.5: Exchange Rate Service

As an **analyst** working with international stocks,
I want the system to provide current exchange rates,
So that cross-currency monetary values can be displayed accurately in future comparison features.

**Acceptance Criteria:**

**Given** the exchange rate service builds on the existing `exchange_rates` model and migration already in the codebase, and is configured with a rate provider
**When** a `GET /api/v1/exchange-rates` request is sent
**Then** current exchange rates are returned for at least EUR, CHF, and USD currency pairs
**And** rates include a `rates_as_of` timestamp indicating data freshness

**Given** a rate provider is selected (ECB for EUR crosses + lightweight API for CHF/USD)
**When** the service fetches rates
**Then** results are cached for a configurable duration (default: 24 hours)
**And** subsequent requests within the cache window return cached rates without external API calls

**Given** the external rate provider is unavailable
**When** the service attempts to fetch rates
**Then** stale cached rates are returned (if available) with a staleness indicator
**And** the failure is logged for admin monitoring (integrates with existing provider health service)
**And** the API does not return an error to the user if cached rates exist

**Given** no cached rates exist and the provider is unavailable
**When** a `GET /api/v1/exchange-rates` request is sent
**Then** the API returns a 503 with a message indicating exchange rate data is temporarily unavailable

### Story 7.6: Library View & Analysis Browsing

As an **analyst**,
I want to browse all my saved analyses in a dedicated Library view,
So that I can find, review, and manage my analysis history across all tickers.

**Acceptance Criteria:**

**Given** the user navigates to `/library` via the Command Strip
**When** the Library view loads
**Then** all saved analysis snapshots are displayed as Compact Analysis Cards
**And** each card shows: ticker symbol, analysis date, thesis locked status, and key metrics (projected Sales/EPS CAGRs, valuation zone)

**Given** the Library view is loaded with multiple analyses
**When** the user uses the search/filter controls
**Then** analyses can be filtered by ticker symbol (text search)
**And** analyses can be filtered by locked/unlocked status
**And** results update immediately without page reload

**Given** a Compact Analysis Card is displayed in the Library
**When** the user clicks the card
**Then** the full Analysis view opens with that snapshot's data loaded

**Given** the Command Strip navigation
**When** the Library view is added
**Then** a "Library" entry appears in the Command Strip with the `/library` route
**And** the Library entry is visually consistent with existing Command Strip entries

**Given** the Library view renders on mobile (<767px)
**When** the viewport is below the mobile breakpoint
**Then** Compact Analysis Cards stack vertically in a single column
**And** the view is read-only consistent with mobile review mode

**Given** the Library view
**When** rendered at all four breakpoints (desktop wide, desktop standard, tablet, mobile)
**Then** all elements render correctly without layout breakage or overflow

### Story 7.7: CI/CD E2E Pipeline Validation

As an **admin**,
I want the CI/CD E2E test pipeline validated in real GitHub Actions,
So that code changes are reliably tested before deployment.

**Acceptance Criteria:**

**Given** the `.github/workflows/e2e.yaml` file exists in the repository
**When** a push or pull request triggers the workflow
**Then** the pipeline runs all 23+ existing E2E tests to completion
**And** the pipeline reports pass/fail status accurately

**Given** the E2E pipeline runs in GitHub Actions
**When** the pipeline requires MariaDB and the backend service
**Then** the workflow correctly provisions database services and backend startup
**And** tests can reach the running application

**Given** any test failures occur in the pipeline
**When** the pipeline completes
**Then** failure output includes sufficient detail (test name, assertion message, screenshot if applicable) to diagnose the issue without local reproduction

---

## Epic 8: Analysis Comparison & Thesis Evolution

Users can compare projected performance across multiple tickers in a ranked grid and track how their analysis of a single stock evolved over quarterly reviews.

### Story 8.1: Comparison Schema & API

As an **analyst**,
I want to save and retrieve multi-stock comparison sets,
So that I can preserve my ranking decisions and revisit them later.

**Acceptance Criteria:**

**Given** the migration runs successfully
**When** the `comparison_sets` table is created
**Then** it includes columns: `id`, `user_id` (FK to users, default 1), `name`, `base_currency` (VARCHAR(3)), `created_at`, `updated_at`

**Given** the migration runs successfully
**When** the `comparison_set_items` table is created
**Then** it includes columns: `id`, `comparison_set_id` (FK), `analysis_snapshot_id` (FK to specific snapshot version), `sort_order`

**Given** the user wants an ad-hoc comparison without saving
**When** a `GET /api/v1/compare?ticker_ids=1,2,3&base_currency=CHF` request is sent
**Then** the system returns the latest snapshot for each ticker with key comparison metrics (projected CAGRs, P/E range, valuation zone)
**And** the response completes in < 3 seconds for up to 20 analyses (NFR6)

**Given** the user wants to save a comparison set
**When** a `POST /api/v1/comparisons` request is sent with name, base_currency, and snapshot IDs
**Then** the comparison set is persisted with items referencing specific snapshot versions (not "latest")
**And** re-analyzing a stock does not alter existing comparison sets

**Given** saved comparison sets exist
**When** `GET /api/v1/comparisons` is called
**Then** all comparison sets for the user are returned with name, base_currency, item count, and created_at

**Given** a specific comparison set ID
**When** `GET /api/v1/comparisons/:id` is called
**Then** the full comparison set is returned with all referenced snapshot data
**And** `PUT` updates the set; `DELETE` removes it

### Story 8.2: Comparison View & Ranked Grid

As an **analyst**,
I want to compare multiple stocks in a ranked grid view,
So that I can identify the best investment candidates at a glance.

**Acceptance Criteria:**

**Given** the user navigates to `/compare` via the Command Strip
**When** the Comparison view loads
**Then** the user can add analyses to the comparison by selecting from the Library or entering ticker symbols
**And** a "Comparison" entry is visible in the Command Strip

**Given** multiple analyses are loaded in the Comparison view
**When** Compact Analysis Cards populate the grid
**Then** each card displays: ticker symbol, analysis date, projected Sales CAGR, projected EPS CAGR, estimated P/E range, valuation zone indicator
**And** cards are sortable by any displayed metric column (click column header to sort)

**Given** the ranked grid contains 5+ analyses
**When** the user sorts by projected EPS CAGR descending
**Then** cards reorder immediately without page reload
**And** the sort indicator is visible on the active column

**Given** the user has built a useful comparison
**When** the user clicks "Save Comparison"
**Then** the comparison set is persisted via `POST /api/v1/comparisons`
**And** the user can name the comparison set

**Given** the user clicks a Compact Analysis Card in the grid
**When** the card is selected
**Then** the full Analysis view opens with that snapshot loaded

**Given** the Comparison view renders on mobile (<767px)
**When** the viewport is below the mobile breakpoint
**Then** cards stack vertically in a single column with key metrics visible
**And** sorting is accessible via a dropdown selector instead of column headers

**Given** the Comparison view
**When** rendered at all four breakpoints (desktop wide, desktop standard, tablet, mobile)
**Then** all elements render correctly without layout breakage or overflow

### Story 8.3: Comparison Currency Handling

As an **analyst** comparing stocks across Swiss, German, and US markets,
I want monetary values converted to my chosen base currency,
So that I can make fair comparisons without manual currency math.

**Acceptance Criteria:**

**Given** a comparison includes analyses with different reporting currencies
**When** the comparison grid renders
**Then** percentage-based metrics (CAGRs, P/E, ROE) display without any currency conversion
**And** monetary values (price targets, market cap if shown) convert to the active base currency using rates from `/api/v1/exchange-rates`

**Given** the Comparison view toolbar
**When** the user selects a different base currency from the currency dropdown
**Then** all monetary values in the grid re-convert to the new currency immediately
**And** percentage metrics remain unchanged
**And** the currency override applies only to this comparison session (does not change global default)

**Given** the global state module (`frontend/src/state/`)
**When** it is created for this story
**Then** a `Currency Preference` global signal is defined (`RwSignal<CurrencyCode>`)
**And** the signal serves as the default base currency across all views
**And** the Comparison view's per-session override takes precedence when active

**Given** the active base currency is CHF
**When** monetary values are displayed
**Then** values show with a contextual currency indicator (e.g., "CHF 145.20 · Values in CHF") anchored to the first monetary value in the view

**Given** exchange rates are unavailable (503 from exchange rate service)
**When** the Comparison view renders mixed-currency analyses
**Then** monetary values display in their native currencies with a notice: "Exchange rates unavailable — values shown in original currencies"

### Story 8.4: Thesis Evolution API

As an **analyst**,
I want to retrieve the history of all my analyses for a single stock,
So that I can see how my projections changed over time.

**Acceptance Criteria:**

**Given** multiple snapshots exist for the same ticker (e.g., 3 quarterly analyses of NESN.SW)
**When** a `GET /api/v1/snapshots/:id/history` request is sent (where :id is any snapshot for that ticker)
**Then** all snapshots for the same `ticker_id` and `user_id` are returned ordered by `captured_at` ascending
**And** each entry includes: id, captured_at, thesis_locked, notes, and key metrics extracted from snapshot_data (projected Sales CAGR, EPS CAGR, P/E estimate)

**Given** only one snapshot exists for a ticker
**When** the history endpoint is called
**Then** a single-item array is returned (no error)

**Given** the history endpoint is called with a valid snapshot ID
**When** the response is returned
**Then** the response includes a `metric_deltas` object comparing consecutive snapshots (e.g., Sales CAGR changed from 6% to 4.5% between Q2 and Q3 analyses)

**Given** the history response
**When** the data is used for side-by-side comparison
**Then** each snapshot includes sufficient data for the frontend to render comparison cards without additional API calls

### Story 8.5: History Timeline Sidebar & Side-by-Side Comparison

As an **analyst**,
I want to see a timeline of my past analyses for the current stock and compare them side by side,
So that I can track how my thesis evolved and make better-informed decisions.

**Acceptance Criteria:**

**Given** the user is on the Analysis view with a populated chart for a ticker that has past analyses
**When** the user clicks the "History" toggle button
**Then** a Timeline Sidebar appears alongside the SSG chart showing all past analyses for this ticker
**And** each entry shows: date, thesis locked status, key CAGR values

**Given** the Analysis view layout
**When** the History Timeline Sidebar is implemented
**Then** the Analysis view uses a composite CSS Grid layout with named regions: `status` (hidden, reserved for Epic 9), `chart`, `sidebar`, `hud`
**And** the sidebar region is hidden by default and revealed when History is toggled
**And** the chart area resizes to accommodate the sidebar (push layout preferred; overlay fallback if chart resize causes rendering issues)

**Given** the Timeline Sidebar is open with multiple past analyses listed
**When** the user selects a past analysis entry
**Then** two side-by-side Snapshot Comparison Cards appear showing the selected past analysis alongside the current analysis
**And** each card displays: projected Sales CAGR, projected EPS CAGR, P/E estimates, valuation zone
**And** metric deltas are highlighted between the two cards (e.g., "Sales CAGR: 6.0% → 4.5% ▼")

**Given** a past analysis has a stored chart image
**When** displayed in the side-by-side comparison
**Then** the static chart image is shown for the past analysis (since it cannot be re-rendered with current charting)

**Given** the Analysis view renders on tablet (768px-1023px)
**When** the History toggle is active
**Then** the Timeline Sidebar renders as a collapsible panel instead of a persistent sidebar

**Given** the Analysis view renders on mobile (<767px)
**When** the mobile breakpoint is active
**Then** the History toggle is available but opens a simplified list view (no side-by-side cards — single-column timeline with key metrics)

**Given** the Analysis view with History Timeline Sidebar
**When** rendered at all four breakpoints (desktop wide, desktop standard, tablet, mobile)
**Then** all elements render correctly without layout breakage or overflow

### Story 8.6: Responsive Design & E2E Tests for Comparison and History

As a **developer**,
I want E2E test coverage for the Comparison and History features,
So that regressions are caught automatically.

**Acceptance Criteria:**

**Given** the Comparison view is implemented
**When** E2E tests run
**Then** tests verify: navigation to `/compare`, adding analyses to comparison, card rendering with correct metrics, sorting by column, saving a comparison set

**Given** the History Timeline Sidebar is implemented
**When** E2E tests run
**Then** tests verify: History toggle opens sidebar, past analyses listed, selecting a past analysis shows side-by-side cards, metric deltas displayed correctly

**Given** the Comparison view at different breakpoints
**When** responsive tests run
**Then** desktop shows full grid with sortable columns
**And** tablet shows the same grid with adjusted column widths
**And** mobile stacks cards vertically with dropdown sort

**Given** currency handling in the Comparison view
**When** E2E tests run with mixed-currency analyses
**Then** tests verify: percentage metrics are unchanged after currency switch, monetary values update to new currency, currency indicator label is accurate

---

## Epic 9: Portfolio Management, Watchlists & Risk Discipline

Users can track portfolios with holdings, receive position sizing guidance and exposure alerts, set stop loss reminders, and monitor a watchlist of stocks of interest. Complete "Buy Smart + Stay Balanced" experience.

**Execution strategy:** Two-sprint internal split — Sprint A (9.1-9.5 portfolio foundation), Sprint B (9.6-9.10 risk discipline + watchlist).

### Story 9.1: Portfolio Schema & CRUD API

As an **investor**,
I want to create and manage multiple portfolios,
So that I can organize my investments into separate strategies (e.g., growth, income, speculative).

**Acceptance Criteria:**

**Given** the migration runs successfully
**When** the `portfolios` table is created
**Then** it includes columns: `id`, `user_id` (FK to users, default 1), `name` (varchar), `currency` (VARCHAR(3)), `rules_config` (JSON, nullable — used by Story 9.2 for per-portfolio risk thresholds), `created_at`, `updated_at`

**Given** the user wants to create a new portfolio
**When** a `POST /api/v1/portfolios` request is sent with name and currency
**Then** the portfolio is created and returned with its ID
**And** the portfolio name must be non-empty; duplicate names for the same user are rejected with a 422

**Given** portfolios exist for the user
**When** a `GET /api/v1/portfolios` request is sent
**Then** all portfolios are returned with id, name, currency, and created_at

**Given** a specific portfolio ID
**When** `GET /api/v1/portfolios/:id` is called
**Then** the portfolio details are returned including its configuration and holding count

**Given** a portfolio exists
**When** `PUT /api/v1/portfolios/:id` is sent with updated name or currency
**Then** the portfolio is updated
**And** `DELETE /api/v1/portfolios/:id` removes the portfolio and its associated holdings

**Given** all portfolio operations
**When** measured under load
**Then** each operation completes in < 1 second (NFR5)

### Story 9.2: Portfolio Configuration

As an **investor**,
I want to configure independent risk rules for each portfolio,
So that my growth portfolio can have different thresholds than my conservative portfolio.

**Acceptance Criteria:**

**Given** a portfolio exists
**When** a `PUT /api/v1/portfolios/:id` request includes a `rules_config` JSON object
**Then** the configuration is stored with fields: `max_allocation_pct` (default 10%), `rebalancing_threshold_pct` (default 5% above target), and additional custom rules
**And** each portfolio's configuration is independent — changing one does not affect others

**Given** a portfolio with `max_allocation_pct` set to 15%
**When** the portfolio configuration is retrieved
**Then** the response includes the full `rules_config` with the 15% threshold

**Given** a portfolio with no explicit configuration
**When** the configuration is retrieved
**Then** sensible defaults are applied: `max_allocation_pct` = 10%, `rebalancing_threshold_pct` = 5%

**Given** invalid configuration values (e.g., `max_allocation_pct` > 100 or < 0)
**When** the configuration update is submitted
**Then** the request is rejected with a 422 and a descriptive validation error

### Story 9.3: Holdings Management & Stop Loss Prompting

As an **investor**,
I want to record stock purchases in my portfolio and be reminded to set a stop loss,
So that I have an accurate record of my positions and don't forget risk protection.

**Acceptance Criteria:**

**Given** the migration runs successfully
**When** the `portfolio_holdings` table is created
**Then** it includes columns: `id`, `portfolio_id` (FK), `ticker_id` (FK), `quantity` (decimal), `purchase_price` (decimal), `purchase_currency` (VARCHAR(3)), `purchase_date` (date), `stop_loss_pct` (nullable decimal), `created_at`

**Given** the user wants to record a purchase
**When** a `POST /api/v1/portfolios/:id/holdings` request is sent with ticker_id, quantity, purchase_price, purchase_date
**Then** the holding is created and returned with its ID

**Given** the holdings creation form (frontend)
**When** the user fills in purchase details
**Then** a stop loss percentage input field is prominently displayed with advisory text: "Consider setting a trailing stop loss at your broker to protect this position"
**And** the field is optional but visually emphasized (not hidden or collapsed)

**Given** holdings exist in a portfolio
**When** `GET /api/v1/portfolios/:id/holdings` is called
**Then** all holdings are returned with ticker symbol, quantity, purchase_price, purchase_date, stop_loss_pct, and current estimated value (based on most recent historical price)

**Given** a holding exists
**When** `PUT /api/v1/portfolios/:id/holdings/:holding_id` is sent
**Then** the holding can be updated (quantity, stop_loss_pct)
**And** `DELETE` removes the holding (e.g., after selling)

**Given** the holdings list response
**When** a holding has `stop_loss_pct` = NULL
**Then** a visual indicator shows "No stop loss set" to encourage the user to configure one at their broker

### Story 9.4: Portfolio Composition & Exposure Detection

As an **investor**,
I want to see my portfolio composition and be alerted when a stock exceeds my allocation threshold,
So that I maintain diversification discipline.

**Acceptance Criteria:**

**Given** a portfolio with holdings
**When** `GET /api/v1/portfolios/:id/exposure` is called
**Then** the response includes: total portfolio value, and for each holding: ticker, current value, allocation percentage (current value / total value × 100)
**And** the operation completes in < 1 second for up to 100 holdings (NFR5)

**Given** a portfolio with `max_allocation_pct` = 10% and a holding at 15% allocation
**When** the exposure endpoint is called
**Then** the over-exposed holding is flagged with `over_exposed: true` and `excess_pct: 5`
**And** a rebalancing suggestion is included: "Consider reducing [ticker] by [amount] to bring allocation to [target]%"

**Given** the `steady-invest-logic` crate
**When** portfolio composition and exposure detection logic is implemented
**Then** the calculation functions live in `crates/steady-invest-logic` (Cardinal Rule)
**And** functions include: `calculate_portfolio_composition()`, `detect_over_exposure()`
**And** each function has doc examples that serve as doctests

**Given** a portfolio where all holdings are within their allocation thresholds
**When** the exposure endpoint is called
**Then** no over-exposure flags are set
**And** the response still includes composition percentages for all holdings

**Given** current price data is sourced from the most recent historical price
**When** the exposure calculation runs
**Then** the response includes a `prices_as_of` timestamp indicating when prices were last updated
**And** this timestamp is displayed in the portfolio view to set user expectations

### Story 9.5: Portfolio Dashboard & Active Portfolio Signal

As an **investor**,
I want a dedicated Portfolio dashboard to review my holdings, exposure, and manage multiple portfolios,
So that I have a full "Control Tower" view for investment management.

**Acceptance Criteria:**

**Given** the user navigates to `/portfolio` via the Command Strip
**When** the Portfolio Dashboard loads
**Then** a multi-portfolio selector is visible, defaulting to the first portfolio (or prompting creation if none exist)
**And** "Portfolio" and "Watchlist" entries appear in the Command Strip

**Given** a portfolio is selected
**When** the dashboard renders
**Then** a holdings table displays: ticker, quantity, purchase price, current value, allocation %, stop loss status
**And** exposure bars visualize allocation per holding using CSS-based bars (not canvas)
**And** bars color-shift: Emerald (safe) → Amber (approaching threshold) → Crimson (exceeded)

**Given** the Active Portfolio global signal
**When** it is created in `frontend/src/state/`
**Then** it is defined as `RwSignal<Option<PortfolioId>>`
**And** changing the active portfolio in the Dashboard immediately updates the signal
**And** other views that consume the signal (Analysis inline context, Watchlist) react accordingly

**Given** the user switches portfolios in the selector
**When** a different portfolio is selected
**Then** the dashboard refreshes with the new portfolio's holdings and exposure
**And** the Active Portfolio signal updates globally

**Given** the Portfolio Dashboard on mobile (<767px)
**When** the mobile breakpoint is active
**Then** holdings display as simplified cards (ticker, value, allocation %)
**And** no write operations (add holding, edit) are available — read-only review mode

**Given** no portfolios exist for the user
**When** the Portfolio Dashboard loads
**Then** a portfolio creation flow is presented: name, currency, initial configuration (max allocation %)

**Given** the Portfolio Dashboard
**When** rendered at all four breakpoints (desktop wide, desktop standard, tablet, mobile)
**Then** all elements render correctly without layout breakage or overflow

### Story 9.6: Position Sizing

As an **investor** considering a stock purchase,
I want the system to suggest a maximum buy amount based on my portfolio rules,
So that I maintain diversification without manual calculations.

**Acceptance Criteria:**

**Given** a portfolio with configured `max_allocation_pct` and existing holdings
**When** `GET /api/v1/portfolios/:id/position-size?ticker_id=X&proposed_amount=Y` is called
**Then** the response includes: current allocation for ticker X (if held), maximum buy amount that keeps the ticker within `max_allocation_pct`, resulting allocation if the proposed amount is used
**And** the operation is stateless (no data is written)
**And** it completes in < 1 second (NFR5)

**Given** a CHF 100K portfolio with 10% max-per-stock rule and 0% current exposure to ticker X
**When** the position sizing endpoint is called
**Then** the suggested maximum buy is CHF 10,000

**Given** a CHF 100K portfolio with 10% max-per-stock rule and 7% current exposure to ticker X (CHF 7,000)
**When** the position sizing endpoint is called
**Then** the suggested maximum buy is CHF 3,000 (to reach 10%)

**Given** the `steady-invest-logic` crate
**When** position sizing logic is implemented
**Then** the function `calculate_position_size()` lives in `crates/steady-invest-logic` (Cardinal Rule)
**And** includes a doctest with the CHF 100K example from the PRD

**Given** the Position Sizing Calculator component
**When** rendered in `panel` mode (Portfolio Dashboard)
**Then** it displays input fields (ticker, proposed amount) and output (max buy, resulting allocation, exposure delta)
**When** rendered in `inline` mode (Analysis view status line)
**Then** it displays a compact summary: "Max buy: CHF X,XXX to stay within targets"

### Story 9.7: Wisdom Card & Inline Portfolio Context

As an **investor** reviewing an analysis,
I want to see portfolio context inline without navigating away,
So that I make disciplined decisions at the moment of conviction.

**Acceptance Criteria:**

**Given** the user is on the Analysis view and the analyzed stock is not in any portfolio or watchlist
**When** the inline status line renders
**Then** it shows: "Not in portfolio · Add? | Not on watchlist · Watch?"
**And** "Add?" and "Watch?" are clickable ghost actions
**And** clicking "Add?" opens the holdings creation form (Story 9.3) with the current ticker pre-filled in the active portfolio context
**And** clicking "Watch?" opens a quick-add watchlist form (Story 9.8 API) with the current ticker pre-filled

**Given** the analyzed stock is held in the active portfolio
**When** the inline status line renders
**Then** it expands with progressive density to show: current allocation %, sector exposure (if available), and position sizing suggestion
**And** a Wisdom Card appears explaining the portfolio impact: "Buy CHF X instead of Y → [sector] drops to Z%"

**Given** the Wisdom Card component
**When** rendered in `inline` mode (within Analysis view status line expansion)
**Then** it displays a compact advisory message with the suggested action
**When** rendered in `panel` mode (standalone in Portfolio Dashboard)
**Then** it displays full detail: current allocation, proposed change, resulting allocation, sector impact

**Given** the Wisdom Card advisory tone
**When** alerts are displayed
**Then** the language is advisory ("Here's what happens if...") not prohibitive ("You cannot...")
**And** alerts never block the user's workflow with modal dialogs
**And** each alert includes a suggested action executable in one click

**Given** the status line shows a threshold-exceeded alert (e.g., "Tech sector: 38% / 30% limit")
**When** a screen reader encounters the alert
**Then** it is announced via `aria-live="assertive"` (immediate)

**Given** the status line shows normal context (e.g., "Not in portfolio")
**When** a screen reader encounters it
**Then** it is announced via `aria-live="polite"` (at next pause)

**Given** the inline portfolio context status line and Wisdom Card
**When** rendered at all four breakpoints (desktop wide, desktop standard, tablet, mobile)
**Then** all elements render correctly without layout breakage or overflow
**And** on mobile, the status line displays in compact read-only mode (no "Add?" or "Watch?" actions)

### Story 9.8: Watchlist Schema & API

As an **investor**,
I want to maintain a watchlist of stocks I'm interested in buying,
So that I can track potential investments with target prices and linked analyses.

**Acceptance Criteria:**

**Given** the migration runs successfully
**When** the `watchlist_items` table is created
**Then** it includes columns: `id`, `user_id` (FK, default 1), `ticker_id` (FK), `target_buy_price` (nullable decimal), `target_currency` (VARCHAR(3)), `notes` (text), `analysis_snapshot_id` (nullable FK to specific snapshot version), `action_pending` (bool, default false), `created_at`, `updated_at`

**Given** the user wants to add a stock to their watchlist
**When** a `POST /api/v1/watchlist` request is sent with ticker_id, target_buy_price, notes, and optional analysis_snapshot_id
**Then** the watchlist item is created and returned

**Given** watchlist items exist
**When** `GET /api/v1/watchlist` is called
**Then** all items are returned with ticker symbol, target price, current price (most recent historical), notes, linked analysis ID, and action_pending flag

**Given** a watchlist item links to a specific analysis snapshot
**When** the user re-analyzes the same stock (creating a new snapshot)
**Then** the watchlist item retains the original snapshot reference
**And** updating the link requires explicit user action (re-link to new snapshot)

**Given** a watchlist item exists
**When** `PUT /api/v1/watchlist/:id` is sent
**Then** notes, target_buy_price, analysis_snapshot_id, and action_pending can be updated
**And** `DELETE /api/v1/watchlist/:id` removes the item (e.g., after purchase or disinterest)

### Story 9.9: Watchlist View

As an **investor**,
I want a dedicated Watchlist view to review stocks I'm tracking,
So that I can quickly identify stocks that have hit my target price and take action.

**Acceptance Criteria:**

**Given** the user navigates to `/watchlist` via the Command Strip
**When** the Watchlist View loads
**Then** all watchlist items are displayed with: ticker, target price, current price, price-reached indicator, notes summary, linked analysis indicator

**Given** a watchlist item where the current price is at or below the target buy price
**When** the item renders
**Then** a price-reached indicator (Emerald) is displayed
**And** the item is visually prioritized (e.g., sorted to top or highlighted)

**Given** a watchlist item with a linked analysis
**When** the user clicks the ticker or a "View Analysis" link
**Then** the Analysis view opens with the linked snapshot loaded

**Given** the user is on mobile and a stock hits its target price
**When** the Watchlist View renders
**Then** an "Action Pending" flag is available that the user can set as a triage marker
**And** the flag persists across mobile and desktop views

**Given** the user purchases a watchlisted stock
**When** the purchase is recorded in a portfolio
**Then** the watchlist entry can be archived (not auto-deleted — user controls this)

**Given** the Watchlist View renders on mobile (<767px)
**When** the mobile breakpoint is active
**Then** items display as a simple list with ticker, target price, current price, and price-reached indicator
**And** the view is read-only (no add/edit/delete on mobile)

**Given** the Watchlist View
**When** rendered at all four breakpoints (desktop wide, desktop standard, tablet, mobile)
**Then** all elements render correctly without layout breakage or overflow

### Story 9.10: Responsive Design & E2E Tests for Portfolio and Watchlist

As a **developer**,
I want E2E test coverage for Portfolio and Watchlist features,
So that the complete Phase 2 experience is regression-tested.

**Acceptance Criteria:**

**Given** the Portfolio Dashboard is implemented
**When** E2E tests run
**Then** tests verify: portfolio creation, holding recording, exposure bar rendering, over-exposure alert display, multi-portfolio switching

**Given** the Position Sizing Calculator is implemented
**When** E2E tests run
**Then** tests verify: correct max buy calculation for the PRD example (CHF 100K portfolio, 10% rule, CHF 10K max), both inline and panel rendering modes

**Given** the Wisdom Card and inline context are implemented
**When** E2E tests run
**Then** tests verify: status line shows "Not in portfolio" for unrelated stocks, status line expands with exposure data for held stocks, advisory tone in alerts (no "Warning:" or "You cannot")

**Given** the Watchlist View is implemented
**When** E2E tests run
**Then** tests verify: adding to watchlist, price-reached indicator when current ≤ target, clicking through to linked analysis, Action Pending flag toggle

**Given** Portfolio and Watchlist views at different breakpoints
**When** responsive tests run
**Then** desktop: full interactive dashboard with all controls
**And** tablet: simplified single-column layout, all controls available
**And** mobile: read-only cards for portfolio, simple list for watchlist, no write operations

---

## Epic 10: Multi-User Authentication

Users have personal accounts with private analyses, portfolios, and watchlists. Multiple users can use the platform independently.

### Story 10.1: User Registration & Password Security

As a **new user**,
I want to register an account with a username and password,
So that I have a personal identity on the platform.

**Acceptance Criteria:**

**Given** an unregistered user
**When** a `POST /api/v1/auth/register` request is sent with username, email, and password
**Then** a new user account is created
**And** the password is hashed using bcrypt or argon2 (never stored in plaintext)
**And** the response returns user ID and username (never the password hash)

**Given** a registration request with a username that already exists
**When** the request is processed
**Then** it is rejected with a 409 Conflict and message "Username already taken"

**Given** a registration request with a weak password (fewer than 8 characters)
**When** the request is processed
**Then** it is rejected with a 422 and a descriptive validation error

**Given** a registration request with an invalid email format
**When** the request is processed
**Then** it is rejected with a 422 and a descriptive validation error

**Given** the existing `users` table in the database
**When** the user model is extended for authentication
**Then** any necessary columns (password_hash, email) are added via migration
**And** the existing default user (ID 1) is preserved — it becomes the "legacy" user whose data will be reassigned during onboarding or remain as a shared default

### Story 10.2: User Login & Session Management

As a **registered user**,
I want to log in and maintain a session,
So that I can access my personal workspace securely.

**Acceptance Criteria:**

**Given** a registered user with valid credentials
**When** a `POST /api/v1/auth/login` request is sent with username and password
**Then** the password is verified against the stored hash
**And** a JWT access token is returned with configurable expiry (default: 24 hours)
**And** a refresh token is returned for session renewal

**Given** a login request with an incorrect password
**When** the request is processed
**Then** it is rejected with a 401 Unauthorized
**And** the error message does not reveal whether the username or password was wrong ("Invalid credentials")

**Given** a login request with a non-existent username
**When** the request is processed
**Then** it is rejected with a 401 Unauthorized with the same generic message (prevents username enumeration)

**Given** a valid refresh token
**When** a `POST /api/v1/auth/refresh` request is sent
**Then** a new access token is issued
**And** the old access token is invalidated (if token rotation is implemented)

**Given** an expired access token
**When** any API request is made with the expired token
**Then** the request is rejected with a 401 and the client can use the refresh token to obtain a new access token

### Story 10.3: Authentication Middleware & API Protection

As a **platform operator**,
I want all API endpoints protected by authentication,
So that users can only access their own data.

**Acceptance Criteria:**

**Given** the authentication middleware is implemented
**When** any request hits a `/api/v1/` endpoint (except public endpoints)
**Then** the middleware extracts and validates the JWT from the `Authorization: Bearer` header
**And** the `user_id` from the token is injected into the request context for downstream use

**Given** a request without an `Authorization` header
**When** it reaches a protected endpoint
**Then** it is rejected with a 401 Unauthorized

**Given** a request with an invalid or tampered JWT
**When** it reaches a protected endpoint
**Then** it is rejected with a 401 Unauthorized

**Given** the middleware exclusion list
**When** requests are made without a JWT to the following public endpoints
**Then** they are processed normally:
- `/api/v1/auth/register` (registration)
- `/api/v1/auth/login` (login)
- `/api/v1/auth/refresh` (token refresh)
- `/api/v1/exchange-rates` (global data — rates are the same for all users)
- `/api/v1/tickers` and ticker search endpoints (global data)

**Given** the middleware extracts `user_id` from the token
**When** a controller accesses the user context
**Then** the `user_id` is available without additional database lookups per request

### Story 10.4: Personal Workspace Isolation

As a **registered user**,
I want my analyses, portfolios, and watchlists to be private,
So that other users cannot see or modify my data.

**Acceptance Criteria:**

**Given** the authentication middleware provides `user_id` in the request context
**When** any data query executes (snapshots, portfolios, holdings, watchlist, comparison sets)
**Then** the query is scoped with `WHERE user_id = [authenticated_user_id]`
**And** no endpoint returns data belonging to a different user

**Given** user A creates a snapshot
**When** user B queries snapshots
**Then** user A's snapshot does not appear in user B's results

**Given** user A attempts to access user B's resource by guessing the ID
**When** `GET /api/v1/snapshots/:id` is called where the snapshot belongs to user B
**Then** the request is rejected with a 404 (not 403, to prevent information leakage about resource existence)

**Given** the existing default user (ID 1) has data from the single-user era
**When** a new user registers
**Then** the legacy data remains under user ID 1
**And** a data claim/migration flow is optionally provided: "Import existing analyses from the single-user setup?"

**Given** workspace isolation must be verified across all controllers
**When** the isolation audit is performed
**Then** the following controllers are explicitly verified for `user_id` scoping:
- `snapshots` controller (analysis snapshots)
- `comparisons` controller (comparison sets and items)
- `portfolios` controller (portfolios and holdings)
- `watchlist` controller (watchlist items)
- `harvest` controller (if user-scoped data exists)
**And** the following controllers are confirmed as global (no `user_id` scoping):
- `exchange_rates` controller (shared rates)
- `tickers` controller (shared ticker data)

### Story 10.5: Authentication UI & E2E Tests

As a **user**,
I want login and registration forms in the application,
So that I can create an account and sign in through the web interface.

**Acceptance Criteria:**

**Given** an unauthenticated user visits the application
**When** any protected route is accessed
**Then** the user is redirected to a login page
**And** the login page provides username and password fields plus a "Register" link

**Given** the registration form
**When** the user fills in username, email, and password and submits
**Then** the form calls `POST /api/v1/auth/register`
**And** on success, the user is automatically logged in and redirected to the Analysis view
**And** on failure, a clear error message is displayed (username taken, weak password, etc.)

**Given** the login form
**When** the user enters valid credentials and submits
**Then** the JWT is stored (localStorage or httpOnly cookie) and the user is redirected to their last viewed page or the Analysis view
**And** the Command Strip shows a user indicator (username or icon) with a logout option

**Given** the user clicks "Logout"
**When** the logout action is triggered
**Then** the JWT is cleared, the user is redirected to the login page, and all in-memory state is reset

**Given** E2E tests for authentication
**When** tests run
**Then** tests verify: registration with valid data succeeds, registration with duplicate username fails, login with correct credentials succeeds, login with wrong password fails, protected endpoints reject unauthenticated requests, workspace isolation prevents cross-user data access, logout clears session

---

## Epic 11: Collaboration (Deferred)

Users can share analyses with investment club members and collaborate on portfolio decisions.

**FRs covered:** FR7.3
**Depends on:** Epic 10 (multi-user authentication)
**Status:** Explicitly deferred until Phase 3 is delivered and validated. No stories created at this time. When Phase 3 is complete, story planning for FR7.3 (shared analyses, team portfolios) will be conducted as a new workflow cycle.

## Party Mode Design Decisions (2026-02-11)

The following architectural refinements were agreed during party mode review with Bob (SM), Winston (Architect), Sally (UX), and John (PM):

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Exchange rate service in Epic 7, not 8 | De-risks Epic 8; infrastructure ships with schema work |
| 2 | Epic 9 unified (Portfolio + Watchlists) | Bidirectional user journeys (4, 6); shared Active Portfolio signal |
| 3 | Static chart capture resolved in Epic 7 (Option A) | Lock-time browser capture; non-blocking; image column in schema from day one |
| 4 | Epic 8 establishes Analysis view composite layout | Named grid regions for future inline context; DoD criterion at code review |
