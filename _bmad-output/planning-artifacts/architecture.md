---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8]
workflowType: 'architecture'
lastStep: 8
status: 'complete'
completedAt: '2026-02-04'
---

# Architecture Decision Document

_This document builds collaboratively through step-by-step discovery. Sections are appended as we work through each architectural decision together._

## Project Context Analysis

### Requirements Overview

**Functional Requirements:**
Architecture must support three core capability clusters:

1. **High-Speed Data Ingestion**: Automated retrieval and normalization of 10-year historicals from SMI, DAX, and US exchanges.
2. **Specialized Visualization**: Rendering of logarithmic trend lines for Sales, Earnings, and Price, with high interactivity for projection charting.
3. **Audit & Verification**: Logic for manual data overrides and "Data Integrity" flagging to ensure user trust.

**Non-Functional Requirements:**

- **Performance**: < 2s initial load and < 5s for the "One-Click" data population (critical for the user "Aha!" moment).
- **Reliability**: 99.9% API integration success rate; zero math errors in quality ratios.
- **Security**: Encrypted transit (HTTPS) for all financial data feeds.

**Scale & Complexity:**

- Primary domain: Fintech / Data Visualization
- Complexity level: High
- Estimated architectural components: 5 (Frontend SPA, Data Engine Service, API Gateway, Cache/Store, Admin Monitor).

### Technical Constraints & Dependencies

- **IFRS/GAAP Normalization**: Logical dependency on robust accounting translation rules.
- **Financial API Stability**: External dependency on 3rd-party data providers.

### Cross-Cutting Concerns Identified

- **Multi-currency Logic**: Affects every comparative calculation.
- **Data Integrity Audit**: A centralized validation layer for batch-loaded data.

## Starter Template Evaluation

### Primary Technology Domain

**Full-Stack Rust Application** with high productivity focus for 'naic' investment tools.

### Starter Options Considered

- **Leptos**: Full-stack Rust using WASM/Isomorphic functions.
- **Axum + Vite**: Decoupled backend/frontend for granular control.
- **Loco**: "Convention over Configuration" framework built on Axum with SeaORM.

### Selected Starter: Loco

**Rationale for Selection:**
Loco provides a structured "productivity suite" that mirrors the Ruby on Rails philosophy but in Rust. For 'naic', this accelerates the implementation of complex multi-currency logic and automated data harvesting by providing built-in ORM management (SeaORM), migrations, and a production-grade CLI.

**Initialization Command:**

```bash
cargo install loco-cli
loco generate app --name naic --db postgres
```

**Architectural Decisions Provided by Starter:**

- **Language & Runtime**: Rust (Tokio-driven async runtime).
- **Database**: PostgreSQL with **SeaORM** for type-safe relational mapping.
- **Deployment**: Built-in Docker scaffolding including multi-stage builds.
- **Code Organization**: Structured MVC approach (Controllers, Models, Tasks).
- **Integrations**: Ready-to-use background jobs and scheduling for batch data harvesting.

## Core Architectural Decisions

### Data Architecture (MariaDB + SeaORM)

**ARCHITECTURAL DECISION UPDATE (Post-Epic 1):** Migrated from PostgreSQL to MariaDB for compatibility with existing infrastructure.

- **Database**: **MariaDB** (changed from PostgreSQL after Epic 1)
- **Storage Pattern**: **Monolithic "Historicals" Table**. All historical financial data (Sales, EPS, Price) for all exchanges (SMI, DAX, US) will be stored in a flat, high-performance table indexed by `ticker` and `period_date`.
- **Validation Strategy**: **Strong Type Enforcement**. Leverage Rust's `serde` and `validator` crates at the ingestion boundary to ensure only valid, split-adjusted data enters the monolithic store.
- **Migration Strategy**: Handled via Loco's built-in migration system using **Sea-Query** with MySQL compatibility.

### Frontend Architecture (Leptos / 100% Rust)

- **Framework**: **Leptos** (Signal-based fine-grained reactivity).
- **Rendering Pattern**: **Client-Side Rendering (CSR)** with WASM for a high-performance local "app" feel, or **SSR with Hydration** depending on final component complexity.
- **State Management**: Distributed signals for real-time chart manipulation and projection shadowing.
- **Charting Engine**: Integration with Rust-based visualization libraries or high-fidelity WASM bindings for maximum consistency with backend logic.

### Security & Authentication

- **User Model**: **Single-User System**. Designed for the individual analyst running 'naic' on a local network.
- **Access Control**: Restricted to `localhost` or local subnets by default. Complex OAuth/JWT is omitted in favor of local-first productivity.
- **Data Protection**: Encrypted HTTPS transit for external financial API feeds; local database credentials managed via standard environment variables.

### API & Communication

- **Pattern**: **Restful API / Server Functions**. Loco provides the backend service layer, while Leptos manages the UI. Data transfer optimized via JSON or MessagePack for large historical batch payloads.

### Infrastructure & Deployment

- **Deployment Pattern**: **Containerized (Docker)**. A multi-stage Docker build handles the Rust compilation (producing a static binary) and the Leptos WASM bundle, served alongside the Postgres database via `docker-compose`.

## Implementation Patterns & Consistency Rules

### Naming Patterns

- **Backend (Rust/Loco)**: Standard `snake_case` for modules, functions, and variables. Controllers follow `[feature]_controller.rs` naming.
- **Frontend (Leptos)**: `PascalCase` for Components (e.g., `SsgChart.rs`). Reactive signals use semantic naming (e.g., `sales_signal`).
- **Database (Postgres)**: Plural `snake_case` for tables (e.g., `historicals`). SeaORM models named in singular `PascalCase` (e.g., `Historical`).
- **Routes**: `/api/v1/[resource]/[action]` (e.g., `/api/v1/tickers/harvest`).

### Structure Patterns

- **Frontend/Backend Separation**: Even in 100% Rust, we will use a clear directory separation: `/frontend` (Leptos CSR app) and `/backend` (Loco API service).
- **Domain Logic**: Business logic for NAIC calculations (ROE, split-adjustments) lives in a shared crate: `/crates/naic-logic` to ensure identical math on both client and server.
- **Tests**: Unit tests co-located; Integration tests in `/tests`.

### Format & Communication

- **API Response**: Standardized JSON wrapper: `{ "status": "success", "data": ... }` or `{ "status": "error", "message": ... }`.
- **Error Handling**: Use the `thiserror` crate for defining domain-specific errors. Centralized error mapping in Loco middleware.
- **Date/Time**: ISO 8601 strings for all API exchanges; stored as UTC in Postgres.

### Process Patterns

- **Graceful Failures**: The "One-Click" engine must return partial results with high-fidelity "Data Gap" flags rather than generic timeouts.
- **Validation Timing**: Database-level constraints are the last line of defense; Loco-level schema validation is the primary.

## Project Structure & Boundaries

### Complete Project Directory Structure

```text
naic/
├── Cargo.toml              # Workspace configuration
├── docker-compose.yml      # Orchestrates Loco, Postgres, and Leptos
├── .env.example            # Template for DB and API keys
├── crates/
│   └── naic-logic/         # SHARED: Math for ROE, Splits, and Multi-currency
├── backend/                # LOCO API SERVICE
│   ├── src/
│   │   ├── controllers/    # API Endpoints (harvest, auth, ssg)
│   │   ├── models/         # SeaORM Entities (Historicals, Users)
│   │   ├── tasks/          # Background Jobs (Batch Harvesters)
│   │   └── mailers/        # (Optional) Notification logic
│   ├── config/             # Backend environments
│   └── tests/              # Integration tests
├── frontend/               # LEPTOS CSR APP
│   ├── src/
│   │   ├── components/     # UI: ChartView, DataGrid, SearchBar
│   │   ├── pages/          # AnalysisPage, Dashboard, Settings
│   │   └── state/          # Leptos Signals (Global Application State)
│   ├── public/             # Static assets (icons, styles)
│   └── Cargo.toml          # Leptos specific dependencies
└── scripts/                # Helper scripts for dev/seeding
```

### Architectural Boundaries

- **API Boundaries**: Restricted to `/api/v1/` for external/mobile parity; strict JSON schema enforcement via Loco models.
- **Component Boundaries**: Shared logic isolated in `crates/naic-logic`. No business logic allowed in pure UI components.
- **Data Boundaries**: Postgres as the system of record. File-based caching managed via Loco tasks if needed for large JSON historicals.

### Requirements to Structure Mapping

- **"One-Click" Ingestion**: Lives in `backend/src/tasks/` (for batch) and `backend/src/controllers/harvest.rs`.
- **Logarithmic SSG Charts**: Lives in `frontend/src/components/SsgChart.rs`.
- **Multi-currency/ROE Logic**: Lives in `crates/naic-logic/` (Used by both).
- **Data Integrity Audit**: Lives in `backend/src/models/historicals.rs` (via custom validations).

## Architecture Validation & Advanced Elicitation Results

### Coherence Validation ✅

- **Decision Compatibility**: Rust 1.8x, Loco 0.16+, and Leptos 0.6+ are verified as compatible. The selection of **`charming`** for charting ensures high-performance WASM rendering while maintaining type safety.
- **Pattern Consistency**: The "Audit-Depth" pattern for Admin features ensures that high-density data verification doesn't compromise the "Zen" UX principles.
- **Structure Alignment**: The addition of a localized `backend/src/services/reporting.rs` module provides a clean boundary for PDF generation.

### Requirements Coverage Validation ✅

- **Non-Functional Requirements**: Performance targets (<5s population) are architecturally supported by async batch processing in Loco.

### Implementation Readiness Validation ✅

- **Decision Completeness**: All critical choices (DB: Postgres, Stack: Rust/Loco/Leptos) are documented with Rationale.
- **Confidence Level**: **High**. The "Convention over Configuration" of Loco minimizes architectural ambiguity for implementation agents.

### Architecture Completeness Checklist

- [x] Project context and fintech complexity analyzed
- [x] Technology stack (Rust Full-Stack) fully specified
- [x] Implementation patterns (Naming/Structure) established
- [x] Project structure and workspace boundaries defined

### Architecture Readiness Assessment

- **Overall Status**: **READY FOR IMPLEMENTATION**
- **Confidence Level**: **High**
- **Key Strengths**: 100% Type safety from DB to UI; Built-in Docker orchestration; Centralized NAIC logic.
- **Areas for Future Enhancement**: Real-time ticker streaming (Phase 2), AI-powered PDF extraction logic.
