# Story 1.1: project-initialization-loco-starter

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a Developer,
I want to initialize the project using the Loco starter template and configure the base environment (Rust, Postgres),
so that I can begin implementing the SteadyInvest analysis engine on a production-grade foundation.

## Acceptance Criteria

1. **Loco Initializaton**: The project must be initialized using the `loco-cli` with the command `loco generate app --name steadyinvest --db postgres`.
2. **Postgres Integration**: SeaORM must be configured and connected to a PostgreSQL database as defined in the `docker-compose.yml`.
3. **Shared Logic Crate**: A dedicated `crates/steady-invest-logic` crate must be initialized to house shared business logic (math for ROE, Splits, etc.).
4. **Project Structure Alignment**: The project must follow the defined directory structure:
    - `/backend`: Loco API service.
    - `/frontend`: Leptos CSR app.
    - `/crates/steady-invest-logic`: Shared domain logic.
    - Root `Cargo.toml`: Workspace configuration.
5. **Build & Start**: The backend must build and start successfully using `cargo loco run` or similar.
6. **Docker Scaffolding**: Multi-stage Docker builds for both backend and frontend must be present and functional via `docker-compose.yml`.

## Tasks / Subtasks

- [x] Initialize Loco backend (AC: 1, 4)
  - [x] Install `loco-cli` if not present.
  - [x] Run `loco generate app --name steadyinvest --db postgres` in a temporary or nested way to move it to `/backend`.
- [x] Set up Workspace (AC: 4)
  - [x] Create root `Cargo.toml` with workspace members `["backend", "frontend", "crates/steady-invest-logic"]`.
- [x] Initialize Shared Logic (AC: 3, 4)
  - [x] Create `crates/steady-invest-logic` with standard `src/lib.rs`.
- [x] Initialize Frontend (AC: 4)
  - [x] Initialize a Leptos CSR project in `/frontend`.
- [x] Configure Environment & Docker (AC: 2, 6)
  - [x] Customize `docker-compose.yml` to orchestrate Backend, Frontend, and Postgres.
  - [x] Ensure `.env.example` is updated.
- [x] Verification (AC: 5)
  - [x] Run `cargo build` at workspace root.
  - [x] Verify backend starts and connects to DB.
- [x] Create project Workspace <!-- id: 0 -->
- [x] Initialize Loco starter app in `/backend` <!-- id: 1 -->
- [x] Initialize Leptos CSR starter app in `/frontend` <!-- id: 2 -->
- [x] Initialize `steady-invest-logic` crate in `/crates` <!-- id: 3 -->
- [x] Configure root `Cargo.toml` and shared workspace dependencies <!-- id: 4 -->
- [x] Create initial `docker-compose.yml` and `.env.example` <!-- id: 5 -->
- [x] Verify full workspace build (backend, frontend, logic) <!-- id: 6 -->

## Dev Notes

- **Architecture Patterns**: Follow the "Convention over Configuration" approach from Loco.
- **Naming Conventions**: Backend uses `snake_case`, Frontend uses `PascalCase` for components.
- **Shared Logic**: Do not implement business logic for NAIC in `backend/` or `frontend/` directly; use `crates/steady-invest-logic`.
- **Reference Architecture**: [Architecture Decision Document](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md)

### Project Structure Notes

- **Unified Structure**: Strict separation of `/backend`, `/frontend`, and `/crates`.
- **SeaORM**: Use `singular PascalCase` for models (e.g., `Historical`) as per architecture.

### References

- [Architecture: Starter Selection](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md#L58-75)
- [Architecture: Directory Structure](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md#L137-160)
- [Epics: Story 1.1 Requirements](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/epics.md#L83-95)

## Dev Agent Record

### Agent Model Used

- **Agent**: Antigravity
- **Status**: Completed
- **Date**: 2026-02-04
- **Walkthrough**: [walkthrough.md](file:///home/gcorbaz/.gemini/antigravity/brain/c7e99b77-7a8c-4a2d-be59-e9ac962c51fe/walkthrough.md)

### Debug Log References

### Completion Notes List

### File List
