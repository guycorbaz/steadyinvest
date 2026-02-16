# SteadyInvest

A web application that automates the [NAIC Stock Selection Guide (SSG)](https://www.betterinvesting.org/) methodology for international markets (Switzerland, Germany, United States).

SteadyInvest replaces manual data entry with "One-Click History" — fetching 10 years of financial data, adjusting for splits and dividends, normalizing currencies, and rendering the familiar SSG charts automatically.

## Features

**Data Engine**
- Automated 10-year historical data retrieval (Sales, EPS, prices, equity)
- Split and dividend adjustment with full audit trail
- Multi-currency normalization (CHF, EUR, USD) with live exchange rates

**Analysis & Visualization**
- Logarithmic SSG chart with best-fit trendlines and CAGR calculation
- High/Low P/E range analysis over the last 10 years
- Quality dashboard (ROE, Profit-on-Sales with trend indicators)
- Interactive projection sliders for EPS CAGR and future P/E estimates
- Upside/downside ratio calculation (NAIC 3-to-1 rule)
- Manual data override system with analyst notes

**Persistence & Comparison**
- Thesis locking with point-in-time snapshots
- Analysis library with search and filtering
- Side-by-side comparison grid with sortable columns
- Client-side currency conversion for cross-market comparisons
- Saved comparison sets

**Reporting & Operations**
- Professional PDF/image export of SSG reports
- API health monitoring dashboard
- Data integrity audit log

## Tech Stack

| Layer | Technology |
|-------|------------|
| Backend | [Loco](https://loco.rs) 0.16 (Rust) + [Axum](https://github.com/tokio-rs/axum) 0.8 |
| Frontend | [Leptos](https://leptos.dev) 0.8 CSR (WebAssembly via [Trunk](https://trunkrs.dev)) |
| Shared Logic | `steady-invest-logic` crate (compiled to both native and WASM) |
| Database | MariaDB with [SeaORM](https://www.sea-ql.org/SeaORM/) 1.1 |
| Charts | [Charming](https://github.com/yuankunzhang/charming) 0.3 (ECharts bindings) |
| CI/CD | GitHub Actions with MariaDB service container + E2E browser tests |

## Architecture

```
┌─────────────────────────────┐
│     Leptos 0.8 CSR (WASM)   │  Browser
│  charming charts · gloo-net  │
└──────────────┬──────────────┘
               │ REST JSON
┌──────────────▼──────────────┐
│   Loco 0.16 / Axum 0.8     │  Server
│   SeaORM 1.1 · MariaDB     │
└──────────────┬──────────────┘
               │
┌──────────────▼──────────────┐
│   steady-invest-logic       │  Shared (native + WASM)
│   NAIC calculations         │
│   Currency conversion       │
└─────────────────────────────┘
```

**Cardinal Rule:** All financial calculation logic lives in the `steady-invest-logic` crate — never duplicated between frontend and backend.

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Trunk](https://trunkrs.dev/) — `cargo install trunk`
- [Loco CLI](https://loco.rs/) — `cargo install loco`
- MariaDB 11+ (or MySQL 8+)
- WASM target — `rustup target add wasm32-unknown-unknown`

### Database Setup

Create the database and user:

```sql
CREATE DATABASE steadyinvest;
CREATE USER 'steadyinvest'@'%' IDENTIFIED BY 'your_password';
GRANT ALL PRIVILEGES ON steadyinvest.* TO 'steadyinvest'@'%';
```

### Configuration

Copy the environment template and adjust:

```bash
cp .env.example .env
# Edit DATABASE_URL and other settings
```

### Running

**Backend** (port 5150):

```bash
cargo loco start
```

**Frontend** (port 8080, proxies API to backend):

```bash
cd frontend
trunk serve
```

Then open `http://localhost:8080`.

### Docker

```bash
docker compose up
```

## Project Structure

```
steadyinvest/
├── backend/                    # Loco REST API
│   ├── src/
│   │   ├── controllers/        # Route handlers (10+ controllers)
│   │   ├── models/             # SeaORM entities
│   │   ├── services/           # Business logic (harvest, reporting, exchange rates)
│   │   └── app.rs              # Loco bootstrap
│   ├── migration/              # SeaORM migrations
│   ├── tests/                  # Request-level integration tests
│   └── config/                 # Environment configs (development, test, production)
├── frontend/                   # Leptos CSR application
│   ├── src/
│   │   ├── components/         # Reusable UI (charts, cards, modals, panels)
│   │   ├── pages/              # Route pages (home, library, compare, system)
│   │   ├── state/              # Global Leptos signals
│   │   └── lib.rs              # App root and router
│   └── public/                 # Static assets and styles
├── crates/
│   └── steady-invest-logic/    # Shared NAIC calculations (native + WASM)
├── tests/e2e/                  # Browser E2E tests (ChromeDriver)
└── docs/                       # Project documentation
```

## Development

### Running Tests

```bash
# All workspace tests (excluding E2E)
cargo test --workspace --exclude e2e-tests

# Shared logic crate only
cargo test -p steady-invest-logic

# Backend integration tests (requires MariaDB)
cargo test -p backend

# E2E browser tests (requires running backend + frontend)
cargo test -p e2e-tests
```

### Key API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/tickers?q=AAPL` | Search tickers |
| POST | `/api/v1/harvest` | Fetch 10-year historical data |
| GET | `/api/v1/snapshots` | List saved analysis snapshots |
| GET | `/api/v1/compare?ticker_ids=1,2,3` | Ad-hoc multi-ticker comparison |
| GET | `/api/v1/comparisons` | List saved comparison sets |
| GET | `/api/v1/exchange-rates` | Current exchange rates (CHF/EUR/USD) |
| GET | `/api/v1/system/health` | API provider health status |
| GET | `/api/v1/system/audit-log` | Data integrity audit trail |

## Roadmap

- **Phase 1** (in progress): Analysis persistence, comparison views, thesis evolution
- **Phase 2**: Portfolio management, watchlists, position sizing, risk discipline
- **Phase 3**: Multi-user authentication, personal workspaces
- **Phase 4**: Collaboration features

## License

This project does not yet have a license. All rights reserved.
