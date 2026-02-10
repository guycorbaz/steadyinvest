# 🔥 CODE REVIEW FINDINGS: Story 4.2

**Story:** [4.2-professional-ssg-report-export-pdf-image.md](file:///home/gcorbaz/synology/devel/naic/_bmad-output/implementation-artifacts/4-2-professional-ssg-report-export-pdf-image.md)
**Git vs Story Discrepancies:** 2 Major found (Tasks incomplete, File List empty)
**Issues Found:** 3 High, 4 Medium, 3 Low

---

## 🔴 CRITICAL / HIGH ISSUES

### 1. Fake Completion Status (Artifact Integrity)

- **Finding**: Story 4.2 is marked as `done` (line 3), but all tasks in [Tasks / Subtasks](file:///home/gcorbaz/synology/devel/naic/_bmad-output/implementation-artifacts/4-2-professional-ssg-report-export-pdf-image.md#L23-35) are marked `[ ]`. The `File List` is completely empty.
- **Impact**: Zero traceability. If a developer joins the project, they have no idea what was changed or if it was truly verified.
- **AC Violated**: General Process Integrity.

### 2. Brittle Font Infrastructure

- **Finding**: [reporting.rs:L35-37](file:///home/gcorbaz/synology/devel/naic/backend/src/services/reporting.rs#L35-37) uses hardcoded Linux-specific paths (`/usr/share/fonts/...`).
- **Impact**: Will crash/fail on macOS, Windows, or non-standard Linux distros.
- **AC Violated**: 4 (Institutional Aesthetic).

### 3. Silent Failure Mode for Charts

- **Finding**: [reporting.rs:L81](file:///home/gcorbaz/synology/devel/naic/backend/src/services/reporting.rs#L81) handles rendering errors by pushing plain text into the PDF.
- **Impact**: The user expects a "Professional SSG Report" but gets a broken document. No error is bubbled back to the API or UI.
- **AC Violated**: 3 (High-Precision Layout), 6 (Chart Fidelity).

---

## 🟡 MEDIUM ISSUES

### 4. Poor Async Citizenship (Blocking)

- **Finding**: `resvg::render` and `doc.render` are heavy synchronous operations called directly in the async handler [reporting.rs:L134](file:///home/gcorbaz/synology/devel/naic/backend/src/services/reporting.rs#L134).
- **Impact**: Blocks the tokio task worker, starving other requests (like ticker search).
- **Fix**: Wrap in `tokio::task::spawn_blocking`.

### 5. Weak Test Assertions

- **Finding**: [reporting_test.rs:L52](file:///home/gcorbaz/synology/devel/naic/backend/src/services/reporting_test.rs#L52) only checks for `%PDF-` header.
- **Impact**: A PDF with 20 blank pages would pass this test. It doesn't verify the chart or quality grid logic.

### 6. Public Exposure of Heavy Export Endpoint

- **Finding**: [analyses.rs:L100](file:///home/gcorbaz/synology/devel/naic/backend/src/controllers/analyses.rs#L100) is a public GET route.
- **Impact**: DOS risk. Low hurdle for an automated tool to hammer this endpoint and consume all server resources.

---

## 🟢 LOW ISSUES / POLISH

### 7. Missing Precision in Quality Grid

- **Finding**: [reporting.rs:L114](file:///home/gcorbaz/synology/devel/naic/backend/src/services/reporting.rs#L114) uses raw `to_string()` for financial values.
- **Impact**: Potential rounding inconsistencies between UI and PDF.

### 8. Magic Numbers

- **Finding**: Hardcoded margins (10) and image dimensions (800x600).
- **Impact**: Harder to refactor for different page sizes (e.g., A4 vs Letter).

### 9. Lint: Unused Imports

- **Finding**: [reporting.rs:L10](file:///home/gcorbaz/synology/devel/naic/backend/src/services/reporting.rs#L10) still contains `HistoricalData` after I supposedly fixed it (Wait, I did remove it in step 715/716, let me re-verify).

---

## Next Steps

- [ ] Fix Artifact documentation (Tasks + File List)
- [ ] Implement `spawn_blocking` for PDF generation
- [ ] Add robust font discovery or embed basic fonts
- [ ] Improve test assertions
