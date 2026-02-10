---
stepsCompleted: [step-01-validate-prerequisites, step-02-design-epics, step-03-create-stories, step-04-final-validation]
inputDocuments: ["_bmad-output/planning-artifacts/prd.md", "_bmad-output/planning-artifacts/architecture.md", "_bmad-output/planning-artifacts/ux-design-specification.md"]
---

# naic - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for naic, decomposing the requirements from the PRD, UX Design if it exists, and Architecture requirements into implementable stories.

## Requirements Inventory

### Functional Requirements

FR1: Users can search international stocks by ticker (e.g., NESN.SW).
FR2: System retrieves 10-year historicals (Sales, EPS, Prices) automatically.
FR3: System adjusts data for historical splits and dividends.
FR4: System normalizes multi-currency data for side-by-side comparison.
FR5: System calculates 10-year Pre-tax Profit on Sales and ROE.
FR6: System calculates 10-year High/Low P/E ranges.
FR7: Users can manually override any automated data field.
FR8: System renders logarithmic trends for Sales, Earnings, and Price.
FR9: System generates trend line projections and "Quality Dashboards."
FR10: Users can export standardized SSG reports (PDF/Image).
FR11: Users can save/share analysis files for collaborative review.
FR12: Admins can monitor API health and flag data integrity errors.

### NonFunctional Requirements

NFR1: SPA initial load under 2 seconds on standard broadband.
NFR2: "One-Click" 10-year population completes in < 5 seconds (95th percentile).
NFR3: API integration engine maintains 99.9% success rate for primary CH/DE feeds.
NFR4: System flags data gaps explicitly rather than silent interpolation.
NFR5: All external API communications use encrypted HTTPS protocols.

### Additional Requirements

#### Technical (from Architecture)

- Starter Template: **Loco** (Rust framework).
- Database: PostgreSQL with SeaORM.
- Frontend: Leptos (CSR/WASM).
- Deployment: Containerized (Docker).
- Domain Logic: Isolated in `crates/naic-logic` for cross-boundary math consistency.
- Security: Single-user system, restricted to local subnets by default.
- "Audit-Depth" pattern for Admin features to verify high-density data.
- Reporting Service: `backend/src/services/reporting.rs` for PDF generation.
- Charting Engine: `charming` library (Rust/WASM).

#### UX (from UX Design Specification)

- "Institutional HUD" scheme: High-contrast, deep black background (#0F0F12).
- "Command Strip": Persistent, slim vertical sidebar for navigation.
- "Kinetic Charting": Direct manipulation of trendlines via dragging handles.
- "Monospace Data Cell": JetBrains Mono for perfect vertical alignment in financial grids.
- Responsive Strategy: Desktop-first "instrument" with tablet review and mobile signal modes.
- Accessibility: WCAG AA, 7:1 contrast, 100% keyboard parity for analysis workflow.
- "Zen-to-Power" transition: Minimalist search expanding into high-density analyst HUD.

### FR Coverage Map

FR1: Epic 1 - Search international stocks by ticker.
FR2: Epic 1 - Retrieve 10-year historicals automatically.
FR3: Epic 1 - Adjust data for historical splits and dividends.
FR4: Epic 1 - Normalize multi-currency data.
FR5: Epic 2 - Calculate 10-year Pre-tax Profit on Sales and ROE.
FR6: Epic 3 - Calculate 10-year High/Low P/E ranges.
FR7: Epic 3 - Manual override of automated data fields.
FR8: Epic 2 - Render logarithmic trends (Sales, Earnings, Price).
FR9: Epic 3 - Generate trend line projections (Kinetic Charting).
FR10: Epic 4 - Export standardized SSG reports (PDF/Image).
FR11: Epic 4 - Save/share analysis files.
FR12: Epic 5 - Monitor API health and flag data integrity errors.

### Technical Debt & Quality Coverage

Epic 6: Quality improvements, bug fixes, testing infrastructure, and comprehensive code documentation to ensure production-readiness and maintainability.

## Epic List

### Epic 1: The One-Click Engine (Core Data Ingestion)

Users can search for a ticker and instantly see a 10-year historical data set (Sales, EPS, Price) that is split-adjusted and currency-normalized.
**FRs covered:** FR1, FR2, FR3, FR4

#### Story 1.1: Project Initialization (Loco Starter)

As a Developer,
I want to initialize the project using the Loco starter template and configure the base environment (Rust, Postgres),
So that I can begin implementing the naic analysis engine on a production-grade foundation.

**Acceptance Criteria:**

- **Given** the technical requirements for a Loco-based Rust application
- **When** I run `loco generate app --name naic --db postgres`
- **Then** the system should create the basic MVC structure, SeaORM configuration, and Docker scaffolding
- **And** the `naic-logic` crate should be initialized for shared business logic
- **And** the project must build and start successfully in a local development environment

#### Story 1.2: Ticker Search with Autocomplete

As a Value Hunter,
I want to search for international stocks using a smart autocomplete bar,
So that I can quickly find the exact ticker and exchange (SMI, DAX, US) I need to analyze.

**Acceptance Criteria:**

- **Given** the user is on the main minimalist search screen
- **When** they type at least 2 characters of a company name or ticker
- **Then** the system should display a real-time list of matching tickers, company names, and exchanges
- **And** selecting a result should trigger the "One-Click" population process for that ticker

#### Story 1.3: Automated 10-Year Historical Retrieval

As a Value Hunter,
I want the system to automatically fetch 10 years of Sales, EPS, and Price data upon ticker selection,
So that I can avoid the manual "data entry tax."

**Acceptance Criteria:**

- **Given** a ticker has been selected via search
- **When** the ingestion engine starts
- **Then** the system should retrieve Sales, EPS, and High/Low Price data for the last 10 completed fiscal years
- **And** the data retrieval must complete in under 5 seconds (NFR2)
- **And** any missing data points must be explicitly flagged with an "Integrity Alert" (NFR4)

#### Story 1.4: Historical Split and Dividend Adjustment

As a Value Hunter,
I want all historical prices to be automatically adjusted for stock splits and dividends,
So that my growth charts reflect real economic performance without artificial distortions.

**Acceptance Criteria:**

- **Given** raw historical data has been fetched
- **When** the system identifies historical stock splits or significant dividends
- **Then** it must apply back-adjustment to all Price and EPS figures prior to the event
- **And** the UI must show a "Split-Adjusted" indicator for the data set

#### Story 1.5: Multi-Currency Normalization

As a Value Hunter,
I want historical data reported in foreign currencies (e.g., CHF, EUR) to be normalized to my preferred currency,
So that I can perform accurate side-by-side benchmarking.

**Acceptance Criteria:**

- **Given** a stock reports in a currency different from the user's preferred currency
- **When** performing calculations or rendering charts
- **Then** the system must convert all historical figures using the historical exchange rates for each period
- **And** the UI must explicitly state the reporting currency vs. the display currency (UX requirement)

### Epic 2: Kinetic SSG Visualization (Core Analysis)

Users can visualize the 10-year history on a logarithmic chart and calculate key NAIC quality ratios (ROE, Profit on Sales).
**FRs covered:** FR5, FR8

#### Story 2.1: Logarithmic SSG Chart Rendering

As a Value Hunter,
I want to see historical Sales, Earnings, and Price plotted on a logarithmic scale,
So that I can visually assess the relative growth rates regardless of the stock's absolute price.

**Acceptance Criteria:**

- **Given** a 10-year data set has been retrieved and normalized
- **When** the Analyst HUD expands
- **Then** the system should render a logarithmic chart showing Sales, EPS, and Price history
- **And** the chart must support high-DPI "Institutional" aesthetics (UX requirement)
- **And** the render time must be under 2 seconds (NFR1)

#### Story 2.2: Historical Growth Trend Visualization

As a Value Hunter,
I want the chart to display trend lines for 10-year Sales and Earnings growth,
So that I can identify the long-term stability and consistency of the business.

**Acceptance Criteria:**

- **Given** the logarithmic chart is displayed
- **When** the "Analyze Trends" mode is active
- **Then** the system should overlay best-fit linear regression lines for Sales and Earnings on the log scale
- **And** the Compound Annual Growth Rate (CAGR) for each must be displayed as a summary statistic

#### Story 2.3: Quality Dashboard (ROE & Profit on Sales)

As a Value Hunter,
I want a dedicated table showing 10-year trends for Pre-tax Profit on Sales and Return on Equity (ROE),
So that I can verify the company's operational efficiency and management quality.

**Acceptance Criteria:**

- **Given** historical financial statements are available
- **When** viewing the "Quality Dashboard" panel
- **Then** the system should present a monospace grid showing the last 10 years of ROE and Pre-tax Profit on Sales
- **And** the grid must use JetBrains Mono for perfect vertical alignment (UX requirement)
- **And** year-over-year trends must be visually highlighted (e.g., heat-mapped or arrow indicators)

### Epic 3: Tactical Valuation (Advanced Analysis)

Users can project future growth and determine valuation ranges (P/E) through direct chart manipulation (Kinetic Charting).
**FRs covered:** FR6, FR7, FR9

#### Story 3.1: Kinetic Trendline Projection (Direct Manipulation)

As a Value Hunter,
I want to project future Sales and Earnings growth by dragging handles on the trendlines,
So that I can intuitively set my estimated growth rates without typing numbers.

**Acceptance Criteria:**

- **Given** the logarithmic chart is displayed
- **When** the user drags the growth handle of a Sales or Earnings trendline
- **Then** the line should pivot to the new growth rate in real-time
- **And** the projected CAGR percentage should update instantaneously via WASM signals

#### Story 3.2: High/Low P/E Range Calculation & Projection

As a Value Hunter,
I want the system to calculate historical P/E ranges and allow me to project a future "Average High" and "Average Low" P/E,
So that I can establish a reasonable valuation floor and ceiling.

**Acceptance Criteria:**

- **Given** historical Price and EPS data is populated
- **When** the user accesses the "Valuation" panel
- **Then** the system should calculate the Average High and Average Low P/E for the last 10 years
- **And** the user can project a "Future Average High P/E" and "Future Average Low P/E" to establish target price zones

#### Story 3.3: Manual Data Override System

As a Value Hunter,
I want to manually override any automated data point (e.g., to exclude a one-time non-recurring gain),
So that I remain the final arbiter of data accuracy.

**Acceptance Criteria:**

- **Given** the 10-year financial grid is visible
- **When** the user double-clicks a specific cell (e.g., EPS for 2021)
- **Then** they can enter a manual override value
- **And** the system should recalculate all dependent ratios and trendlines immediately
- **And** the cell must be visually marked as "Manually Overridden" with an audit trail note

### Epic 4: Professional Reporting & Sharing (Collaboration)

Users can lock their thesis, save/share analysis files, and export professional PDF/Image reports for their investment clubs.
**FRs covered:** FR10, FR11

#### Story 4.1: Thesis Locking & Snapshot Generation

As a Value Hunter,
I want to lock my analysis and growth projections with a summary note,
So that I have a permanent record of my investment thesis at a specific point in time.

**Acceptance Criteria:**

- **Given** an active analysis session with growth projections
- **When** the user clicks "Lock Thesis"
- **Then** the system should capture a snapshot of all data points, projections, and valuation targets
- **And** prompt the user to enter a text-based "Analyst Note"
- **And** save the snapshot to the database as a "Locked Analysis"

#### Story 4.2: Professional SSG Report Export (PDF/Image)

As a Club Moderator,
I want to export a clean, high-precision PDF or image of the analysis,
So that I can share standardized reports with the rest of my investment club.

**Acceptance Criteria:**

- **Given** a locked analysis snapshot
- **When** the user selects "Export PDF"
- **Then** the system should generate a professional-grade report containing the SSG Chart, Quality Dashboard, and Valuation summary
- **And** the report must follow the "Institutional" design aesthetic (no clutter)
- **And** the PDF must be generated via the backend reporting service (Architecture requirement)

#### Story 4.3: Analysis File Persistence (Open/Save)

As a Value Hunter,
I want to save my analysis session to a local file and reopen it later,
So that I can build a long-term library of stock research.

**Acceptance Criteria:**

- **Given** an unsaved analysis session
- **When** the user selects "Save to File"
- **Then** the system should allow downloading the session data as a portable file (e.g., .naic or JSON)
- **And** the "Open File" function must restore all data, overrides, and projections perfectly

### Epic 5: Operational Excellence (Admin)

Admins can monitor API health and audit data integrity to ensure the platform remains the "Gold Standard" for international stocks.
**FRs covered:** FR12

#### Story 5.1: API Health Monitoring Dashboard

As an Admin,
I want to monitor the status and rate limits of all connected financial data APIs (CH, DE, US),
So that I can preemptively fix connection issues or provider downtime.

**Acceptance Criteria:**

- **Given** an admin user logged in
- **When** viewing the "System Monitor" dashboard
- **Then** the system should display real-time status (Online/Offline) and latency for all primary data providers
- **And** current rate limit consumption should be visible as a percentage of quota

#### Story 5.2: Data Integrity Audit Log

As an Admin,
I want a centralized log of all "Integrity Alerts" and manual overrides,
So that I can identify systemic data quality issues or faulty provider feeds.

**Acceptance Criteria:**

- **Given** the system is processing or storing ticker data
- **When** an anomaly is detected or a user performs a manual override
- **Then** the system must record the event in a central "Audit Log" identifying the ticker, field, and source of change
- **And** the admin can filter and export this log for quality control

#### Story 5.3: System Health & Latency Monitor

As an Admin,
I want to see a persistent system health indicator (the "Bloomberg Speed" indicator),
So that I can ensure the platform consistently meets the 2-second render performance target.

**Acceptance Criteria:**

- **Given** the application is running
- **When** any page or chart renders
- **Then** a persistent health indicator in the footer should display the exact render time in milliseconds
- **And** the indicator should glow Crimson if render time exceeds 500ms (NFR target)

### Epic 6: MVP Refinement & Polish (Quality & Technical Debt)

After Epic 5, the application is a functional MVP with all core features working. Epic 6 addresses critical refinements, bug fixes, and technical debt to ensure production-readiness and maintainability.
**FRs covered:** Technical debt resolution, quality improvements

#### Story 6.1: Investigate and Fix CAGR/EPS Slider Behavior

As a Value Hunter,
I want the Sales CAGR and EPS CAGR sliders to control their respective projections correctly,
So that my growth rate adjustments produce accurate valuation calculations.

**Acceptance Criteria:**

- **Given** the SSG chart is displayed with projection sliders active
- **When** I move the Sales CAGR slider
- **Then** only the Sales projection trendline should update (not EPS)
- **And** the projected Sales CAGR percentage should reflect the slider position
- **When** I move the EPS CAGR slider
- **Then** only the EPS projection trendline should update (not Sales)
- **And** the projected EPS CAGR percentage should reflect the slider position
- **And** the calculated target prices must accurately reflect the correct projections
- **And** all signal bindings in `ssg_chart.rs` and `chart_bridge.js` are verified correct

#### Story 6.2: Visual and Graphical Refinements

As a Value Hunter,
I want the application's visual design to match the "Institutional HUD" aesthetic throughout,
So that the interface feels professional and polished.

**Acceptance Criteria:**

- **Given** the UX Design Specification defines the "Institutional HUD" aesthetic
- **When** reviewing all screens and components
- **Then** chart aesthetics must be visually polished and consistent
- **And** layout spacing and alignment must be refined across all components
- **And** color scheme must be consistent with the deep black background (#0F0F12)
- **And** typography must use correct fonts (JetBrains Mono for data cells)
- **And** all interactive elements must have appropriate hover and active states

#### Story 6.3: UX Consistency Pass

As a Value Hunter,
I want a consistent user experience across all features and workflows,
So that the application feels cohesive and intuitive.

**Acceptance Criteria:**

- **Given** the application has multiple features and panels
- **When** navigating between different sections
- **Then** navigation patterns must be consistent (Command Strip behavior)
- **And** data entry patterns must follow the same interaction model
- **And** error states and validation messages must use consistent styling and tone
- **And** keyboard shortcuts must work consistently across all screens
- **And** loading states and feedback must follow the same visual language

#### Story 6.4: Responsive Design Improvements

As a Value Hunter,
I want the application to work well on tablet devices for review sessions,
So that I can reference my analysis away from my desktop workstation.

**Acceptance Criteria:**

- **Given** the UX specification defines "Desktop-first instrument with tablet review mode"
- **When** accessing the application on a tablet (iPad, Android tablet)
- **Then** the layout must adapt gracefully without breaking the analyst workflow
- **And** charts must remain readable and interactive on tablet screens
- **And** data grids must use horizontal scrolling where necessary to maintain data integrity
- **And** the Command Strip navigation must adapt to tablet screen sizes
- **And** touch interactions must work smoothly for all interactive elements

#### Story 6.5: E2E Test Suite Implementation

As a Developer,
I want comprehensive end-to-end tests covering critical user journeys,
So that we catch integration issues before they reach users (like the chart rendering bugs in Epic 5).

**Acceptance Criteria:**

- **Given** the application has multiple integrated features
- **When** implementing E2E tests using Playwright
- **Then** tests must cover the complete ticker search → data retrieval → chart rendering → analysis workflow
- **And** tests must verify interactive slider functionality (preventing slider inversion bugs)
- **And** tests must verify navigation accessibility (preventing unreachable features)
- **And** tests must validate data override and thesis locking workflows
- **And** tests must run in CI/CD pipeline on every commit
- **And** test failures must block deployment to prevent shipping broken features

#### Story 6.6: Comprehensive Rust Documentation Pass

As a Developer,
I want all Rust code to follow best practices with comprehensive documentation,
So that the codebase is maintainable and new team members can onboard effectively.

**Acceptance Criteria:**

- **Given** the codebase spans backend, frontend, and logic crate
- **When** reviewing all Rust modules
- **Then** all public functions and methods must have doc comments (`///`) explaining purpose, parameters, and return values
- **And** all structs, enums, and type definitions must be documented with `///` comments
- **And** complex algorithms and tricky code sections must have inline explanatory comments
- **And** each module must have module-level documentation (`//!`) explaining its purpose and structure
- **And** non-trivial functions must include usage examples in doc comments
- **And** panic conditions and error handling must be explicitly documented
- **And** documentation must cover: Backend (`backend/src/**`), Frontend (`frontend/src/**`), Logic crate (`crates/naic-logic/**`)
