# Story 5.2: Data Integrity Audit Log

Status: completed

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an Admin,
I want a centralized log of all "Integrity Alerts" and manual overrides,
so that I can identify systemic data quality issues or faulty provider feeds.

## Acceptance Criteria

1. **Event Capture**: [x] Automatically record anomalies (data gaps, outliers) and manual data overrides.
2. **Detailed Logging**: [x] Log entries must include: Timestamp, Ticker, Exchange, Field Name, Old Value, New Value, and Source (System/User).
3. **Admin Dashboard view**: [x] A high-density "Audit Log" grid accessible under the System Monitor (RESTRICTED to local subnets).
4. **Filtering & Export**: [x] Support filtering by Ticker, Date Range, and Event Type (Anomaly vs. Override).
5. **Quality Reporting**: [x] Ability to export the filtered log to CSV for external quality control analysis.

## Tasks / Subtasks

- [x] Backend: Audit Log Infrastructure (AC: 1, 2)
  - [x] Create migration for `audit_logs` table.
  - [x] Implement SeaORM entity and user-facing model.
  - [x] Create `AuditService` for recording events across the system.
- [x] Backend: API & Security (AC: 3, 4, 5)
  - [x] Implement `/api/v1/system/audit-logs` with filtering and DTOs.
  - [x] Apply IP-based subnet restriction (supports tests via optional extension).
  - [x] Implement CSV export logic with safety limits.
- [x] Frontend: Audit View (AC: 3, 4, 5)
  - [x] Create `AuditLog` page in `frontend/src/pages/audit_log.rs`.
  - [x] Implement the `Monospace Data Cell` grid for high-density log viewing.
  - [x] Add sidebar for advanced filtering (Ticker, Dates).

## Dev Notes

- **Architecture Compliance**: Follow the `Audit-Depth` pattern (Line 180 of Architecture).
- **Naming**: Table `audit_logs`, Model `AuditLog`.
- **UI UX**: Use `JetBrains Mono` for the log grid.
- **Security**: Enforce local network restriction at the controller level.

### Project Structure Notes

- Backend: `backend/src/controllers/audit.rs`, `backend/src/models/audit_logs.rs`
- Frontend: `frontend/src/pages/audit_log.rs`
- Shared logic: `crates/steady-invest-logic/src/lib.rs` (shared event types)

### References

- [Epics: Story 5.2](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/epics.md#L305)
- [Architecture: Data Integrity Audit](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md#L44-L45)
- [Architecture: Audit-Depth Pattern](file:///home/gcorbaz/synology/devel/naic/_bmad-output/planning-artifacts/architecture.md#L180)

## Dev Agent Record

### Agent Model Used

Antigravity

### Debug Log References

### Completion Notes List

- Implemented full Audit-Depth logging.
- Secured API with RFC 1918 subnet check.
- Added CSV export functionality for QC.
- Created high-density HUD for admins.
- **Remediation**: Resolved global 400 test failures by fixing MySQL test configuration and enabling migration features.

### File List

- [backend/migration/src/m20260208_120000_audit_logs.rs](file:///home/gcorbaz/synology/devel/naic/backend/migration/src/m20260208_120000_audit_logs.rs)
- [backend/src/models/audit_logs.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/models/audit_logs.rs)
- [backend/src/services/audit_service.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/services/audit_service.rs)
- [backend/src/controllers/system.rs](file:///home/gcorbaz/synology/devel/naic/backend/src/controllers/system.rs) (Consolidated Audit Controller logic)
- [frontend/src/pages/audit_log.rs](file:///home/gcorbaz/synology/devel/naic/frontend/src/pages/audit_log.rs)
- [backend/tests/requests/audit.rs](file:///home/gcorbaz/synology/devel/naic/backend/tests/requests/audit.rs)
- [backend/tests/requests/system.rs](file:///home/gcorbaz/synology/devel/naic/backend/tests/requests/system.rs)
