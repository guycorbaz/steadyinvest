---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8]
workflowType: 'architecture'
lastStep: 8
status: 'complete'
completedAt: '2026-02-04'
revisionStarted: '2026-02-10'
revisionScope: 'Post-MVP evolution — Phase 1-4 architecture expansion'
inputDocuments:
  - '_bmad-output/planning-artifacts/prd.md'
  - '_bmad-output/planning-artifacts/ux-design-specification.md'
  - '_bmad-output/implementation-artifacts/epic-6-retro-2026-02-10.md'
---

# Architecture Decision Document

_This document builds collaboratively through step-by-step discovery. Sections are appended as we work through each architectural decision together._

## Project Context Analysis

### Requirements Overview

**Functional Requirements:**
Architecture must support six core capability clusters, aligned to PRD phases:

1. **High-Speed Data Ingestion** (MVP — delivered): Automated retrieval and normalization of 10-year historicals from SMI, DAX, and US exchanges. Split/dividend adjustment, multi-currency normalization.
2. **Specialized Visualization** (MVP — delivered): Logarithmic trend lines for Sales, Earnings, and Price with interactive projection manipulation (kinetic charting). Quality dashboards, valuation panels. PDF export built (Story 4.2) but needs UI navigation routing fix (Epic 6 retro action item #4).
3. **Audit & Verification** (MVP — delivered): Manual data overrides, Data Integrity flagging, API health monitoring, system latency tracking.
4. **Analysis Persistence & Comparison** (Phase 1): Server-side storage of analysis snapshots with ticker/date indexing. Multi-ticker ranked comparison grid. Thesis evolution tracking across time. **Currency conversion boundary:** only monetary values (prices, market cap) convert to the user-selectable base currency via exchange rate service; percentage-based metrics (CAGRs, P/E, ROE) display without conversion. **Note:** Single-stock currency normalization (FR1.4 — e.g., Swiss investor viewing US stock revenue in CHF) is deferred to Phase 2 alongside portfolio currency needs; Phase 1 comparison uses current exchange rates for monetary columns only.
5. **Portfolio Management & Risk Discipline** (Phase 2): Multiple portfolios per user with independent rules. Holdings tracking, position sizing, over-exposure detection, rebalancing suggestions. Watchlists with analysis cross-references. Single-stock currency normalization enabled here.
6. **Authentication & Collaboration** (Phase 3-4): User registration and secure login. Personal workspaces with per-user analyses, portfolios, and watchlists. Shared analyses and team portfolios (Phase 4 vision).

**Non-Functional Requirements:**

- **Performance** (measured as API response time, single-user baseline; requires re-validation under concurrent load at Phase 3):
  - < 2s initial SPA load on standard broadband
  - < 5s "One-Click" 10-year population (95th percentile)
  - < 1s portfolio operations (up to 100 holdings, API response)
  - < 2s analysis snapshot retrieval (API response)
  - < 3s multi-stock comparison for up to 20 analyses (API response including exchange rate lookup)
- **Reliability**: 99.9% API integration success rate; zero math errors in quality ratios.
- **Security**: Encrypted HTTPS transit for all external feeds. Multi-user readiness in schema design from Phase 1.
- **Accessibility**: WCAG 2.1 Level AA; keyboard navigation and screen reader support across all views.

**Scale & Complexity:**

- Primary domain: Fintech / Investment Management
- Complexity level: High
- Architectural components:

| # | Component | Status | Notes |
|---|-----------|--------|-------|
| 1 | Frontend SPA (Leptos/WASM) | Delivered | 5 Core Views defined in Frontend Architecture section |
| 2 | API Service (Loco) | Delivered | REST endpoints, server functions |
| 3 | Data Engine (harvest pipeline) | Delivered | Batch historical population |
| 4 | Database (MariaDB + SeaORM) | Delivered | Schema evolves per phase |
| 5 | Shared Logic Crate (`naic-logic`) | Delivered | See Architectural Differentiators below |
| 6 | Exchange Rate Service | Partial | Model and migration exist |
| 7 | Admin Monitor | Delivered | Health, latency, audit |
| 8 | **Static Chart Image Capture** | **OPEN** | See below |

**OPEN DECISION: Static Chart Image Capture** — The key question is *when* the image needs to exist, which determines the technology:

- **Option A — Lock-time browser capture** (recommended): Image captured client-side when user locks a thesis. Zero server infrastructure. Only available for locked analyses. Aligns with UX spec's "Static Chart Snapshot" guideline.
- **Option B — On-demand headless rendering**: Server-side headless browser. Pixel-perfect fidelity for any analysis. Adds ~400MB container size and a non-Rust dependency.
- **Option C — Separate Rust pipeline**: Pure Rust server-side rendering. Stays in-ecosystem but creates dual-charting maintenance burden.

This is separate from the PDF export UI fix (already built, needs routing only).

### Architectural Differentiators

The shared logic crate (`crates/naic-logic`) is naic's structural competitive advantage. Identical NAIC calculation math — ROE, profit-on-sales, split adjustments, CAGR projections — compiles to both native Rust (server-side validation) and WASM (client-side instant projections). This means:

- **Auditability**: The math an investor sees in the browser is *literally the same compiled code* the server uses for persistence and comparison. No divergence possible.
- **Offline capability**: Client-side calculations work without a server round-trip, enabling the "kinetic charting" instant-feedback experience.
- **Open-source trust**: Any user can audit the single source of truth for all financial calculations.

This "single-crate, dual-target" pattern is a deliberate architectural choice, not a convenience. It must be preserved as the codebase evolves — any new calculation logic belongs in `naic-logic`, never duplicated between frontend and backend.

### Technical Constraints & Dependencies

- **IFRS/GAAP Normalization**: Logical dependency on robust accounting translation rules.
- **Financial API Stability**: External dependency on 3rd-party data providers.
- **Exchange Rate Service**: Required for multi-stock comparison monetary value conversion (Phase 1). Current rates sufficient — no historical rate series needed. **Existing infrastructure:** `exchange_rates` model and migration already in codebase.
- **Analysis Persistence**: **Existing infrastructure:** `locked_analyses` model/migration and `analyses` controller already in codebase. Phase 1 extends this foundation with snapshot storage and retrieval.
- **PDF Export**: Built in Story 4.2, functional. Needs UI navigation menu routing fix — this is a wiring task, not an architectural concern.
- **Multi-User Readiness**: All new database tables include `user_id` column from day one, even though authentication (Phase 3) comes later. Default user ID used until then.

### Cross-Cutting Concerns Identified

- **Multi-currency Logic**: Affects comparative calculations. Global default currency with per-comparison override. Conversion applies to monetary values only — ratios pass through unconverted.
- **Data Integrity Audit**: Centralized validation layer for batch-loaded data.
- **Portfolio Context Signal**: Active portfolio selection propagates across views — analysis, comparison, and watchlist surfaces respond to it. This is both a **data concern** (API endpoints accept portfolio context) and a **frontend reactive signal** (Leptos global state that multiple components subscribe to for progressive density rendering). View-to-component mapping defined in Frontend Architecture section.
- **Schema Migration Safety**: All schema changes must be additive and non-breaking, supporting seamless evolution through Phases 1-3. **Rollback strategy:** database backup before each migration run via automated pre-migration wrapper script (`scripts/migrate-safe.sh` — to be created; dumps MariaDB before running `cargo loco db migrate`). SeaORM migrations are forward-only; recovery relies on backup restore. Migration scripts stored in version control for audit trail.

## Starter Template Evaluation

> **Settled Decision (2026-02-04).** Technology stack was evaluated and selected before MVP development. The rationale remains valid for post-MVP evolution. This section is historical context, not an open decision.

### Primary Technology Domain

**Full-Stack Rust Application** with high productivity focus for 'naic' investment analysis and portfolio management platform.

### Starter Options Considered

- **Leptos**: Full-stack Rust using WASM/Isomorphic functions.
- **Axum + Vite**: Decoupled backend/frontend for granular control.
- **Loco**: "Convention over Configuration" framework built on Axum with SeaORM.

### Selected Starter: Loco

**Rationale for Selection:**
Loco provides a structured "productivity suite" that mirrors the Ruby on Rails philosophy but in Rust. For 'naic', this accelerates the implementation of complex multi-currency logic, automated data harvesting, and database-persisted analyses by providing built-in ORM management (SeaORM), migrations, and a production-grade CLI. The convention-over-configuration approach proved its value across 6 epics of MVP development.

**Pinned Versions (as of 2026-02-10, from workspace `Cargo.toml`):**

| Dependency | Version | Notes |
|-----------|---------|-------|
| `loco-rs` | 0.16 | Backend framework |
| `leptos` | 0.8 | Frontend CSR with WASM |
| `sea-orm` | 1.1 | `sqlx-mysql`, `runtime-tokio-rustls` |
| `charming` | 0.3 | `wasm` (frontend) + `ssr` (backend) features |

### Loco Capabilities vs. Custom Extensions

**What Loco provides (used in naic):**

- MVC structure (Controllers, Models, Views)
- SeaORM migrations via Sea-Query with MySQL compatibility
- Controller routing and middleware pipeline
- Docker scaffolding (multi-stage builds)
- CLI tooling (`cargo loco`)
- Async Tokio runtime for concurrent API calls

**What Loco provides (not currently used):**

- Background worker job queue (harvest pipeline uses controller/async pattern instead)
- Mailer system

**What naic extends beyond Loco (Phase 1-3):**

- **Authentication** (Phase 3): Loco has basic JWT support; naic will need session management, user registration, and personal workspaces.
- **Exchange Rate Service** (Phase 1): Custom service layer integrating with external rate providers — no Loco equivalent.
- **Static Chart Capture** (Phase 1): Client-side canvas export at thesis lock time — entirely custom.
- **Portfolio Calculation Engine** (Phase 2): Lives in `crates/naic-logic`, not in Loco's service layer. Risk rules, position sizing, exposure detection are domain logic, not framework concerns.

## Core Architectural Decisions

### Data Architecture (MariaDB + SeaORM)

**ARCHITECTURAL DECISION UPDATE (Post-Epic 1):** Migrated from PostgreSQL to MariaDB for compatibility with existing infrastructure.

- **Database**: **MariaDB** (changed from PostgreSQL after Epic 1)
- **Storage Pattern**: **Monolithic "Historicals" Table**. All historical financial data (Sales, EPS, Price) for all exchanges (SMI, DAX, US) stored in a flat, high-performance table indexed by `ticker` and `period_date`.
- **Validation Strategy**: **Strong Type Enforcement**. Leverage Rust's `serde` and `validator` crates at the ingestion boundary to ensure only valid, split-adjusted data enters the monolithic store.
- **Migration Strategy**: Handled via Loco's built-in migration system using **Sea-Query** with MySQL compatibility. Pre-migration backup via `scripts/migrate-safe.sh`.

**Post-MVP Schema Expansion (Phase 1-3):**

All new tables include a `user_id` column (FK to `users`) from day one. Until Phase 3 authentication, a default user ID (1) is used for all operations.

| Table | Phase | Purpose | Key Columns |
|-------|-------|---------|-------------|
| `analysis_snapshots` | 1 | Versioned SSG analyses | `user_id`, `ticker_id`, `snapshot_data` (JSON), `thesis_locked` (bool), `captured_at` |
| `comparison_sets` | 1 | Saved multi-ticker comparisons | `user_id`, `name`, `base_currency`, `created_at` |
| `comparison_set_items` | 1 | Analyses within a comparison | `comparison_set_id`, `analysis_snapshot_id` (FK to specific version), `sort_order` |
| `portfolios` | 2 | Portfolio definitions | `user_id`, `name`, `exposure_threshold_pct`, `rules_config` (JSON) |
| `portfolio_holdings` | 2 | Stock positions | `portfolio_id`, `ticker_id`, `quantity`, `purchase_price`, `purchase_date`, `stop_loss_pct` |
| `watchlist_items` | 2 | Stocks of interest | `user_id`, `ticker_id`, `target_buy_price`, `notes`, `analysis_snapshot_id` (nullable FK to specific version) |

**`analysis_snapshots` vs. existing `locked_analyses`:** The existing `locked_analyses` table stores locked thesis snapshots (immutable). The new `analysis_snapshots` table stores both locked and unlocked snapshots. Rather than renaming and altering `locked_analyses` in-place (5 migration operations), create a clean new `analysis_snapshots` table with a one-time data migration script that copies existing locked analyses into the new table (with `thesis_locked = true`, `user_id = 1`), then drops the old table. **Column mapping:** `analysis_data` → `snapshot_data`, `locked_at` → `captured_at`. Document this mapping in the migration script comments.

**Version-based snapshot model:** The `analysis_snapshots` table is **append-only** — each save creates a new row, never updates an existing one. This design simultaneously solves two problems:

1. **Comparison integrity**: Comparison set items reference a specific snapshot row by ID. Re-analyzing a stock creates a *new* row; existing comparisons continue to reference the original snapshot version.
2. **Thesis evolution** (FR4.2): Querying all snapshots for the same `ticker_id` + `user_id` ordered by `captured_at` naturally produces the thesis evolution timeline — no additional schema needed.

The "latest" snapshot for a ticker is simply the most recent `captured_at` row. Older versions are retained for history and comparison stability. Watchlist items also reference specific snapshot versions — if a user re-analyzes a stock, the watchlist retains the original reference; updating requires explicit re-linking.

**Snapshot retention (Phase 2+ concern):** The append-only model means snapshot count grows with each save. For Phase 1, this is negligible. A retention/archival policy (e.g., auto-archive unlocked snapshots older than N months, retain all locked snapshots indefinitely) should be designed in Phase 2 when usage patterns are understood.

**Immutability contract:** Rows with `thesis_locked = true` are immutable — the application layer must reject updates to locked snapshots. Unlocked snapshots are also never updated in-place (append-only model) but *can* be soft-deleted if the user explicitly discards a draft. This contract is enforced in the controller layer, not via database constraints.

**Ephemeral vs. persisted comparison:** The `comparison_sets` table stores *saved* comparisons for retrieval. Ad-hoc comparisons (user selects tickers, sees ranked grid immediately) are supported via query parameters to the comparison endpoint — no persistence required. The API supports both patterns.

**Current price data source:** Portfolio exposure and composition calculations require current stock prices. The default source is the **most recent historical price** in the `historicals` table (the latest `period_date` entry for each ticker). **The price source is pluggable**: initially last-known historical, with an architectural hook to add on-demand "last close" lookup in Phase 2 if accuracy demands it (lightweight single-ticker API call when user opens portfolio view — not real-time streaming). A "prices as of" timestamp is displayed in the portfolio view to set user expectations.

### Frontend Architecture (Leptos 0.8 / CSR)

- **Framework**: **Leptos 0.8** (Signal-based fine-grained reactivity, CSR mode).
- **Rendering Pattern**: **Client-Side Rendering (CSR)** with WASM. SSR was evaluated but CSR chosen for the interactive "app-like" experience required by kinetic charting.
- **Charting Engine**: `charming` 0.3 with WASM feature for ECharts-based logarithmic visualization.

**State Management (Post-MVP expansion):**

- **Existing**: Distributed signals for chart manipulation and projection shadowing (defined inline in components).
- **New — Global Signals**: Two new application-level signals required:
  - **Active Portfolio Signal**: `RwSignal<Option<PortfolioId>>` — selected portfolio propagates to analysis, comparison, and watchlist views for progressive density rendering.
  - **Currency Preference Signal**: `RwSignal<CurrencyCode>` — global default currency; overridable per comparison view.
- **Signal Architecture**: Global signals defined in a new `frontend/src/state/` module (to be created — does not exist today). Components subscribe via `use_context()`. No prop-drilling for cross-view state.

**Core Views (5 destinations, mapped from UX spec):**

| View | Route | Phase | Components |
|------|-------|-------|------------|
| Analysis | `/` (home) | MVP (exists) | SSG Chart, Quality Dashboard, Valuation Panel, Analyst HUD |
| Comparison | `/compare` | Phase 1 | Compact Analysis Cards, Ranked Grid, Currency Selector |
| Library | `/library` | Phase 1 | Analysis list, search/filter, History Timeline |
| Portfolio | `/portfolio` | Phase 2 | Holdings table, Exposure Bars, Position Sizer, Wisdom Card |
| Watchlist | `/watchlist` | Phase 2 | Watchlist table, linked analyses, Action Pending flags |

**Router expansion:** The current `leptos_router` handles `/`, `/system-monitor`, `/audit-log`, and 404. Phase 1 adds `/compare` and `/library`; Phase 2 adds `/portfolio` and `/watchlist`. The `frontend/src/pages/mod.rs` module requires restructuring to register new page components.

**Chart Rendering Performance:** Target < 500ms from data received to chart fully rendered for a standard 10-year, 6-series SSG chart (measured in browser DevTools). To be validated during Phase 1 frontend work.

### Security & Authentication

- **Phase 1-2 — Single-User System**: Designed for the individual analyst running 'naic' on a local network or personal server. No authentication required. Default `user_id = 1` used in all database operations.
- **Phase 3 — Multi-User**: Loco's JWT support extended with:
  - User registration and password hashing (bcrypt/argon2)
  - Session management (JWT tokens with configurable expiry)
  - Middleware guard on all `/api/v1/` endpoints extracting `user_id` from token
  - Personal workspace isolation: all queries scoped by `user_id`
- **Data Protection**: Encrypted HTTPS transit for external financial API feeds; local database credentials managed via environment variables (`.env`).
- **Access Control Migration**: The `user_id` column exists from Phase 1; Phase 3 adds the middleware that enforces it. No schema changes needed at authentication time.

### API & Communication

- **Pattern**: **RESTful API** via Loco controllers. Frontend communicates via `fetch` calls from WASM.
- **Payload Format**: JSON for all API exchanges. MessagePack evaluated but JSON chosen for debuggability and browser DevTools compatibility.

**Post-MVP API Expansion:**

| Endpoint Group | Phase | Methods | Purpose |
|---------------|-------|---------|---------|
| `/api/v1/snapshots` | 1 | GET, POST, DELETE | Analysis snapshot CRUD (POST = new version) |
| `/api/v1/snapshots/:id/history` | 1 | GET | Thesis evolution for same ticker (all versions) |
| `/api/v1/compare` | 1 | GET | Ephemeral comparison (ticker IDs + base currency as query params) |
| `/api/v1/comparisons` | 1 | GET, POST, PUT, DELETE | Persisted comparison set management |
| `/api/v1/exchange-rates` | 1 | GET | Current rates for currency conversion |
| `/api/v1/portfolios` | 2 | GET, POST, PUT, DELETE | Portfolio CRUD |
| `/api/v1/portfolios/:id/holdings` | 2 | GET, POST, PUT, DELETE | Holdings management |
| `/api/v1/portfolios/:id/exposure` | 2 | GET | Exposure analysis with composition percentages and alerts |
| `/api/v1/portfolios/:id/position-size` | 2 | GET | Stateless position sizing calculation (query params: ticker, amount) |
| `/api/v1/watchlist` | 2 | GET, POST, PUT, DELETE | Watchlist CRUD |
| `/api/v1/auth/*` | 3 | POST | Registration, login, refresh |

**FR5.6 stop loss prompting:** The `stop_loss_pct` column on `portfolio_holdings` stores the user's intended stop loss percentage. The "prompt at purchase time" behavior (FR5.6) is a **frontend UX concern**: the holdings creation form (triggered by `POST /holdings`) displays a stop loss input field with advisory text. No separate backend endpoint needed — the prompt is part of the holding creation UI flow.

**Test coverage implication:** ~29 new API routes across Phases 1-3, requiring approximately 90-120 new API tests (happy path + validation error + not-found per route, plus unauthorized tests for Phase 3). Existing E2E suite has 23 tests — test count will ~5x.

### Infrastructure & Deployment

- **Deployment Pattern**: **Containerized (Docker)**. Multi-stage Docker build: Rust compilation (static binary) + Leptos WASM bundle, served alongside MariaDB via `docker-compose`.
- **Container Composition**: `docker-compose.yml` orchestrates `naic-backend`, `naic-frontend`, and `mariadb` services.
- **Environment Configuration**: `.env` file with database credentials, API keys, and configurable parameters. `.env.example` maintained as template.

## Implementation Patterns & Consistency Rules

> **CARDINAL RULE:** All calculation logic — ROE, profit-on-sales, split adjustments, CAGR projections, position sizing, exposure detection — lives in `crates/naic-logic`. Never duplicated between frontend and backend. This is a trust and auditability guarantee, not a convenience. See Architectural Differentiators.

### Naming Patterns

- **Backend Controllers**: `snake_case`, resource-named: `analyses.rs`, `portfolios.rs`, `watchlist.rs`. No `_controller` suffix.
- **Backend Models**: `snake_case` files, singular `PascalCase` entities: `analysis_snapshot.rs` → `AnalysisSnapshot`.
- **Backend Services**: `snake_case` in `backend/src/services/`: `reporting.rs`, `exchange_rates.rs`. (New directory — to be created.)
- **Frontend Components**: `snake_case` files: `ssg_chart.rs`, `portfolio_dashboard.rs`. Component functions use `PascalCase`: `fn SsgChart()`, `fn PortfolioDashboard()`.
- **Frontend Pages**: `snake_case` files in `frontend/src/pages/`: `home.rs`, `comparison.rs`, `library.rs`, `portfolio.rs`, `watchlist.rs`.
- **Database Tables (MariaDB)**: Plural `snake_case`: `historicals`, `analysis_snapshots`, `portfolio_holdings`.
- **Routes**: `/api/v1/[resource]` for collections. `/api/v1/[resource]/:id/[sub-resource]` for nested resources.
- **Global Signals**: Defined in `frontend/src/state/mod.rs`. Named as `[domain]_signal`: `active_portfolio_signal`, `currency_preference_signal`.

### Structure Patterns

- **Frontend/Backend Separation**: Clear directory separation: `/frontend` (Leptos CSR app) and `/backend` (Loco API service).
- **Domain Logic (naic-logic crate)**: Business logic for NAIC calculations lives in `/crates/naic-logic`. Compiles to both native Rust and WASM. Currently a single `lib.rs` file; may evolve to multi-module structure (e.g., `calculations.rs`, `types.rs`, `portfolio.rs`) as Phase 2 adds position sizing and exposure logic. See Cardinal Rule above.
- **Model vs. Service boundary**: **Models** handle database access (SeaORM entities, queries, relations). **Services** handle business logic that orchestrates across models or calls external APIs (e.g., fetching exchange rates from external provider, composing PDF reports). `backend/src/services/` is a new directory to create in Phase 1.
- **Global State**: Application-level Leptos signals defined in `frontend/src/state/` (new module, to be created). Component-local signals remain inline.
- **Tests**:
  - **Unit tests**: Co-located with source files (`#[cfg(test)]` modules).
  - **Doctests**: Public functions in `naic-logic` include `///` doc examples that double as doctests (established practice from Epic 6, Story 6.6).
  - **Backend API tests**: In `backend/tests/` — one test file per controller (e.g., `snapshots_test.rs`). Cover happy path + validation error + not-found per route.
  - **E2E tests**: In top-level `tests/` directory. Browser-based tests via the existing E2E framework.

### Format & Communication

- **API Response**: Standardized JSON wrapper: `{ "status": "success", "data": ... }` or `{ "status": "error", "message": ... }`.
- **Error Handling**: Use the `thiserror` crate for defining domain-specific errors. Centralized error mapping in Loco middleware.
- **Date/Time**: ISO 8601 strings for all API exchanges; stored as UTC in MariaDB.
- **Currency Codes**: ISO 4217 three-letter codes (e.g., `CHF`, `EUR`, `USD`). Represented as a validated newtype `CurrencyCode(String)` in Rust (defined in `naic-logic` for shared use). Validation enforces 3 uppercase ASCII letters at API boundary on deserialization. Stored as `VARCHAR(3)` in database.
- **Snapshot Data**: Serialized as JSON (`serde_json::Value`) in `snapshot_data` columns. Schema validated at the application layer, not database level.

### Process Patterns

- **Graceful Failures**: The "One-Click" engine must return partial results with high-fidelity "Data Gap" flags rather than generic timeouts.
- **Validation Timing**: Database-level constraints are the last line of defense; Loco-level schema validation is the primary.
- **Append-Only Snapshots**: Analysis saves always create new rows — never update existing snapshot rows. Controller layer enforces this.
- **Immutability on Lock**: Locked analyses reject all modification requests at the controller layer. Unlocked snapshots can be soft-deleted but not modified in-place.
- **Pre-Migration Backup**: All schema migrations preceded by automated database backup (`scripts/migrate-safe.sh`).

## Project Structure & Boundaries

### Complete Project Directory Structure

```text
naic/
├── Cargo.toml                  # Workspace configuration
├── docker-compose.yml          # Orchestrates backend, frontend, MariaDB
├── .env.example                # Template for DB credentials and API keys
├── docs/                       # Project documentation
│   ├── definition-of-done.md
│   ├── deployment-verification-checklist.md
│   └── ...
├── scripts/                    # Helper scripts
│   └── migrate-safe.sh         # (To create) Pre-migration backup wrapper
├── crates/
│   └── naic-logic/             # SHARED: NAIC calculations (ROE, CAGR, splits, position sizing)
│       └── src/lib.rs          # Single file today; may split to modules in Phase 2
├── backend/                    # LOCO API SERVICE
│   ├── src/
│   │   ├── bin/                # Binary entry points
│   │   ├── controllers/        # API endpoints: analyses, auth, harvest, overrides, system, tickers
│   │   │                       # Phase 1 adds: snapshots, comparisons, exchange_rates
│   │   │                       # Phase 2 adds: portfolios, watchlist
│   │   ├── models/             # SeaORM entities: historicals, tickers, users, exchange_rates,
│   │   │   │                   #   locked_analyses, audit_logs, provider_rate_limits, historicals_overrides
│   │   │   └── _entities/      # Auto-generated SeaORM entity files
│   │   ├── services/           # Business logic: audit, exchange, harvest, provider_health, reporting
│   │   │                       # Phase 1 adds: snapshot_service, comparison_service
│   │   │                       # Phase 2 adds: portfolio_service, exposure_service
│   │   ├── tasks/              # Background/CLI tasks
│   │   ├── middlewares/         # Request middleware pipeline
│   │   ├── initializers/       # App startup hooks
│   │   ├── views/              # Response serialization
│   │   ├── workers/            # Loco worker jobs (not currently used)
│   │   ├── mailers/            # Email templates (Loco scaffold, not currently used)
│   │   ├── data/               # Static data files
│   │   └── fixtures/           # Test fixtures
│   ├── config/                 # Environment configs (development, production, test)
│   ├── migration/              # SeaORM migrations (11 existing + new per phase)
│   └── tests/                  # Backend API integration tests
├── frontend/                   # LEPTOS CSR APP
│   ├── src/
│   │   ├── components/         # UI components: ssg_chart, quality_dashboard, valuation_panel,
│   │   │                       #   analyst_hud, search_bar, command_strip, lock_thesis_modal,
│   │   │                       #   override_modal, snapshot_hud, footer
│   │   │                       # Phase 1 adds: compact_analysis_card, currency_selector, history_timeline
│   │   │                       # Phase 2 adds: portfolio_dashboard, exposure_bars, position_sizer,
│   │   │                       #   wisdom_card, watchlist_view
│   │   ├── pages/              # Route pages: home, system_monitor, audit_log, not_found
│   │   │                       # Phase 1 adds: comparison, library
│   │   │                       # Phase 2 adds: portfolio, watchlist
│   │   ├── state/              # (To create) Global Leptos signals
│   │   └── persistence.rs      # Browser file save/load — retained as secondary export option
│   │                           # alongside Phase 1 server persistence (useful for offline sharing)
│   ├── public/                 # Static assets (CSS, icons)
│   └── Cargo.toml              # Frontend-specific dependencies
└── tests/                      # E2E test suite
    └── e2e/                    # Browser-based E2E tests (23 existing)
```

### Architectural Boundaries

- **API Boundaries**: All backend endpoints under `/api/v1/`. Strict JSON schema enforcement via Loco models and service layer validation.
- **Component Boundaries**: Shared calculation logic isolated in `crates/naic-logic` (Cardinal Rule). No business logic in pure UI components — components consume signals and render, services compute.
- **Data Boundaries**: MariaDB as the system of record. Browser-based file persistence (`persistence.rs`) retained as secondary export option alongside server persistence — useful for offline sharing with club members.
- **Service Boundaries**: Controllers handle HTTP concerns (routing, request parsing, response formatting). Services handle business logic. Models handle data access. No direct database calls from controllers — always through models or services.

### Requirements to Structure Mapping

**MVP (delivered):**

| Requirement | Location |
|------------|----------|
| "One-Click" Ingestion | `backend/src/services/harvest.rs` + `backend/src/controllers/harvest.rs` |
| Logarithmic SSG Charts | `frontend/src/components/ssg_chart.rs` |
| Multi-currency/ROE Logic | `crates/naic-logic/src/lib.rs` |
| Data Integrity Audit | `backend/src/services/audit_service.rs` + `backend/src/models/audit_logs.rs` |
| PDF Export | `backend/src/services/reporting.rs` |
| API Health Monitoring | `backend/src/services/provider_health.rs` + `frontend/src/pages/system_monitor.rs` |

**Post-MVP (new):**

| Requirement | Location |
|------------|----------|
| Analysis Snapshots (FR4.1) | `backend/src/controllers/snapshots.rs` + `backend/src/services/snapshot_service.rs` |
| Thesis Evolution (FR4.2) | `backend/src/controllers/snapshots.rs` (history endpoint) |
| Multi-Ticker Comparison (FR4.3) | `backend/src/controllers/comparisons.rs` + `backend/src/services/comparison_service.rs` |
| Exchange Rate Conversion | `backend/src/services/exchange.rs` (exists) + `backend/src/controllers/exchange_rates.rs` |
| Portfolio Management (FR5.1-5.5) | `backend/src/controllers/portfolios.rs` + `backend/src/services/portfolio_service.rs` + `crates/naic-logic` (calculations) |
| Stop Loss Prompt (FR5.6) | `frontend/src/components/` (holding creation form UX) |
| Watchlists (FR6.1-6.2) | `backend/src/controllers/watchlist.rs` + `frontend/src/pages/watchlist.rs` |
| Authentication (FR7.1-7.2) | `backend/src/controllers/auth.rs` (exists) + `backend/src/middlewares/` |

## Architecture Validation & Readiness Assessment

### Coherence Validation

- **Decision Compatibility**: Rust 1.8x, Loco 0.16, Leptos 0.8, SeaORM 1.1, and `charming` 0.3 verified as compatible across 6 epics of MVP development. No version conflicts anticipated for post-MVP additions.
- **Pattern Consistency**: The append-only snapshot model, service layer boundary, and Cardinal Rule (naic-logic exclusivity) form a coherent set of patterns. Each reinforces the others — snapshots preserve calculation results from naic-logic, services orchestrate without duplicating logic, controllers stay thin.
- **Schema Coherence**: All new tables include `user_id` from Phase 1; Phase 3 authentication adds middleware enforcement without schema changes. Version-based snapshots serve both comparison integrity and thesis evolution (FR4.2) with a single design pattern.
- **UX-Architecture Alignment**: Five Core Views map to explicit routes and page components. Global signals (active portfolio, currency preference) bridge the UX progressive density pattern to Leptos reactive architecture. View-to-component mapping is complete.
- **Cross-Section Consistency**: 6 capability clusters (Project Context) → elaborated in Core Decisions → mapped to file locations in Structure. Cardinal Rule referenced in Architectural Differentiators, Implementation Patterns, and Component Boundaries. Append-only model in Core Decisions and Process Patterns. No contradictions found.

### Requirements Coverage Validation

| FR Group | Architectural Coverage | Status |
|----------|----------------------|--------|
| FR1 (Search & Population) | Delivered (MVP) | Complete |
| FR2 (Analysis & Visualization) | Delivered (MVP) | Complete |
| FR3 (Reporting & Operations) | Delivered; PDF UI routing fix needed | Complete (minor fix) |
| FR4 (Analysis Persistence) | Schema, API endpoints, service layer defined | Ready |
| FR5 (Portfolio Management) | Schema, API endpoints, naic-logic extension, UX components defined | Ready |
| FR6 (Watchlist) | Schema, API endpoints, page component defined | Ready |
| FR7 (Multi-User) | user_id columns, auth controller exists, middleware pattern defined | Ready |

| NFR | Architectural Support | Validation Method |
|-----|----------------------|-------------------|
| NFR1 (< 2s load) | CSR WASM bundle, Docker multi-stage | Browser DevTools measurement (automate with Lighthouse/web-vitals in CI as future improvement) |
| NFR2 (< 5s population) | Async Tokio, parallel API calls | API response time logging via Loco request middleware or `tracing` spans |
| NFR3 (99.9% API success) | Provider health monitoring, retry logic | `backend/src/services/provider_health.rs` metrics (delivered) |
| NFR4 (explicit data gaps) | "Data Gap" flags in harvest pipeline | Delivered (MVP); validated by E2E tests |
| NFR5 (HTTPS) | Loco middleware, external API config | Deployment verification checklist |
| NFR6 (multi-user migration) | user_id on all tables from Phase 1 | Schema inspection |
| NFR7 (< 1s portfolio ops) | MariaDB indexed queries, service layer | Timed assertions in API integration tests |
| NFR8 (< 2s snapshot, < 3s comparison) | Indexed queries, exchange rate caching | Timed assertions in API integration tests |

### Open Decisions

| Decision | Options | Recommended | Resolve By |
|----------|---------|-------------|------------|
| Static Chart Image Capture | A: Lock-time browser capture, B: Headless rendering, C: Rust pipeline | Option A | Phase 1 story planning |
| naic-logic module structure | Single lib.rs vs. multi-module | Defer until Phase 2 complexity demands it | Phase 2 |
| Snapshot retention policy | Time-based archival vs. unlimited retention | Defer until usage patterns observed | Phase 2 |
| On-demand price refresh | Last-known historical vs. fresh daily close | Last-known historical (Phase 1); evaluate in Phase 2 | Phase 2 |
| Performance test harness | A: Extend existing E2E framework, B: `criterion` benchmark suite, C: Timed assertions in integration tests | Option C for API NFRs; Option A for frontend NFRs | Phase 1 story planning |

### Architecture Completeness Checklist

- [x] Project context and fintech complexity analyzed (updated for Phase 1-4)
- [x] Technology stack fully specified with pinned versions
- [x] Post-MVP database schema designed with multi-user readiness
- [x] API endpoint expansion defined for Phases 1-3
- [x] Frontend architecture expanded (Core Views, global signals, router)
- [x] Security evolution path (single-user → multi-user) documented
- [x] Implementation patterns updated (Cardinal Rule, service boundaries, append-only snapshots)
- [x] Project structure reflects actual codebase with phase expansion annotations
- [x] Requirements-to-structure mapping covers all FR and NFR groups
- [x] Open decisions catalogued with recommended resolutions and timelines

### Architecture Readiness Assessment

- **Overall Status**: **READY FOR EPIC PLANNING**
- **Confidence Level**: **High**
- **Key Strengths**:
  - 100% type safety from DB to UI via Rust full-stack
  - Shared logic crate ensures calculation auditability and trust
  - Multi-user readiness baked into schema from Phase 1
  - Version-based snapshots elegantly solve comparison integrity and thesis evolution
  - Existing infrastructure (services, exchange rates, analyses controller) provides foundation for Phase 1
- **Key Risks**:
  - `naic-logic` crate is a single 30K-line file — will need modularization as Phase 2 adds portfolio calculations
  - Test count needs to ~5x from current 23 E2E tests to cover new API surface
  - Static chart capture and performance test harness decisions must be resolved before first Phase 1 story
  - `locked_analyses` → `analysis_snapshots` data migration involves column mapping and data transformation (`analysis_data` → `snapshot_data`, `locked_at` → `captured_at`, default `user_id = 1`). Low severity but non-zero risk — a botched migration could lose existing locked analyses. Mitigated by pre-migration backup.
- **Next Step**: Epic and story planning for Phase 1 (Fix & Foundation)
