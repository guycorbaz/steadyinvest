# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

SteadyInvest automates the NAIC Stock Selection Guide (SSG) methodology for international stock markets (CH, DE, US). It fetches 10 years of financial data, adjusts for splits/dividends, normalizes currencies, and renders SSG charts.

## Architecture

**Rust workspace with 5 crates:**

| Crate | Role |
|-------|------|
| `backend` | Loco 0.16 REST API (Axum 0.8), SeaORM 1.1 + MariaDB |
| `backend/migration` | SeaORM schema migrations (tickers are seeded in migrations, not fixtures) |
| `frontend` | Leptos 0.8 CSR SPA compiled to WASM via Trunk |
| `crates/steady-invest-logic` | Shared NAIC calculations — compiled to both native and WASM |
| `tests/e2e` | Browser E2E tests using ThirtyFour (ChromeDriver) |

**Cardinal Rule:** All financial calculation logic MUST live in `steady-invest-logic`. Never duplicate between frontend and backend.

**Data flow:** Browser (Leptos/WASM) --REST JSON--> Loco/Axum --SeaORM--> MariaDB. The `steady-invest-logic` crate is used by both sides for growth analysis, P/E ranges, quality metrics, and projections.

## Build & Run Commands

```bash
# Build everything
cargo build --workspace

# Run backend (port 5150, requires MariaDB + DATABASE_URL in .env)
cargo loco start

# Run frontend dev server (port 3000, requires trunk)
cd frontend && trunk serve

# Build frontend for production
cd frontend && trunk build --release
```

## Testing

```bash
# All unit + integration tests (requires MariaDB for backend tests)
cargo test --workspace --exclude e2e-tests

# Single crate
cargo test -p steady-invest-logic
cargo test -p backend

# Single test by name
cargo test -p backend test_name_here

# E2E tests (requires running backend + frontend + ChromeDriver)
cargo test -p e2e-tests -- --test-threads=1 --nocapture
```

**Backend test requirements:**
- All backend integration tests MUST use `#[serial]` (from `serial_test` crate) — `dangerously_recreate` causes DB race conditions in parallel
- `boot_test` and `request` do NOT auto-seed fixture data; only `db::converge()` runs (recreate+migrate)
- Tickers are seeded in migration `m20260204_185151_tickers.rs`, not via fixtures
- Loco's `db::seed()` works for inserts but `reset_autoincrement` is unimplemented for MySQL — the error is caught in `App::seed()`

**E2E test environment variables:** `HEADLESS=true`, `BASE_URL=http://localhost:5173`, `CHROME_DRIVER_URL=http://localhost:9515`

## Linting

```bash
cargo fmt --all -- --check
cargo clippy --workspace --exclude e2e-tests -- -D warnings
```

**Crate-level lint suppressions exist:** Frontend allows Leptos macro lints (`dead_code`, `clone_on_copy`, etc.). Backend allows `result_large_err` and `too_many_arguments` (Loco framework patterns).

## CI Pipeline

Single workflow `.github/workflows/e2e.yaml` with 4 sequential jobs: **fmt -> clippy -> unit-tests -> e2e**. Uses MariaDB 11 service container. Rust toolchain pinned to 1.93.0 via `rust-toolchain.toml` at workspace root.

## Database

- MariaDB (or MySQL 8+) with SeaORM 1.1
- Config: `backend/config/{development,test,production}.yaml` — uses Tera templates for env var interpolation
- `test.yaml` defaults to `localhost:3306` with generic creds; `DATABASE_URL` env var overrides
- Test mode uses `dangerously_recreate: true` (drops and recreates schema each run)

## Key Patterns

**Leptos 0.8 CSR specifics:**
- `use_navigate()` must be called at component init scope, NOT inside closures/callbacks
- Frontend DTOs must match backend response field names exactly (silent deserialization failure otherwise)
- Slider inputs: JS `dispatchEvent(new Event('input'))` does NOT trigger Leptos reactive signals in headless Chrome

**SeaORM / Loco:**
- Snapshot dates use `DATEZ` format on MySQL (timezone-aware). Use `cleanup_user_model_compat()` for cross-DB snapshot tests.
- Fixture YAML must include `id` fields — `from_json` requires all non-optional fields even though MySQL auto-generates IDs

**Frontend API calls** use `gloo-net` (not reqwest). Backend uses `reqwest` for external APIs (Yahoo Finance, exchange rates).

## Project Management (BMAD)

Story files live in `_bmad-output/implementation-artifacts/`. Sprint status tracked in `sprint-status.yaml`. Planning docs in `_bmad-output/planning-artifacts/`. The `_bmad/` directory contains workflow tooling (not application code — exclude from reviews).

## Docker

```bash
docker compose up          # backend:5150 + frontend:8080
docker compose build       # Dockerfiles pinned to rust:1.93.0
```
