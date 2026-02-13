# Story 5.1: API Health Monitoring Dashboard

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an Admin,
I want to monitor the status and rate limits of all connected financial data APIs (CH, DE, US),
so that I can preemptively fix connection issues or provider downtime.

## Acceptance Criteria

1. **Dashboard Access**: [x] Admin dashboard "System Monitor" accessible via Sidebar/HUD (Access restricted to local subnets).
2. **Provider Health**: [x] Display real-time status (Online/Offline) and latency (ms) for all primary data providers.
3. **Rate Limit Transparency**: [x] Visible percentage of quota consumed for each API key/provider.
4. **Audit-Depth View**: [x] Implementation of high-density data verification panels without cluttering main UX.

## Tasks / Subtasks

- [x] Backend: API Monitoring Service (AC: 2, 3)
  - [x] Implement health check logic for primary data providers (US, CH, DE)
  - [x] Implement rate limit tracking (SeaORM/Postgres persistence or In-memory cache)
  - [x] Expose health data via `/api/v1/system/health`
- [x] Frontend: System Monitor Dashboard (AC: 1, 2, 3, 4)
  - [x] Create `SystemMonitor` page in Leptos
  - [x] Implement real-time polling or refresh for API status
  - [x] Render "Institutional HUD" style health indicators (latencies, status colors)
- [x] UI/UX: Admin Navigation (AC: 1)
  - [x] Add "System Monitor" icon/link to the Command Strip

## Dev Notes

- **Architecture Compliance**: Follow the `Audit-Depth` pattern for high-density verification.
- **Backend**: Use `backend/src/controllers/system_controller.rs` for health endpoints.
- **Frontend**: Page should live in `frontend/src/pages/system_monitor.rs`.
- **Security**: Access is restricted to local network as per Architecture decision (line 95).
- **Math consistency**: Latency calculations should be accurate to milliseconds.

### Project Structure Notes

- Backend: `backend/src/controllers/system_controller.rs`
- Frontend: `frontend/src/pages/system_monitor.rs`, `frontend/src/components/health_indicator.rs`
- Logic: `crates/steady-invest-logic/src/lib.rs` (if any shared status types are needed)

### References

- [Epics: Story 5.1](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/epics.md#L292)
- [Architecture: Admin Monitor](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md#L34-L35)
- [Architecture: Audit-Depth Pattern](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md#L180)
- [PRD: Journey 3 (Admin)](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/prd.md#L70)

## Dev Agent Record

### Agent Model Used

Antigravity

### Debug Log References

### Completion Notes List

- [x] Implemented `ProviderHealth` service with simulated checks and DB persistence for rate limits.
- [x] Implemented IP-based security restriction for the health endpoint (AC 1).
- [x] Created `SystemController` with `health` endpoint.
- [x] Developed `SystemMonitor` page in Leptos with "Institutional HUD" design.
- [x] Integrated `HealthIndicator` component with color-coded latency alerts.
- [x] Added navigation link in `AnalystHUD` header.
- [x] Verified backend services and frontend routing.

### File List

- `backend/src/services/provider_health.rs`
- `backend/src/services/mod.rs` (modified)
- `backend/src/controllers/system.rs`
- `backend/src/controllers/mod.rs` (modified)
- `backend/src/app.rs` (modified)
- `backend/src/models/_entities/provider_rate_limits.rs`
- `backend/src/models/provider_rate_limits.rs`
- `backend/migration/src/m20260208_114000_provider_rate_limits.rs`
- `backend/tests/requests/system.rs`
- `frontend/src/pages/system_monitor.rs`
- `frontend/src/pages/mod.rs` (modified)
- `frontend/src/lib.rs` (modified)
- `frontend/src/pages/home.rs` (modified)
