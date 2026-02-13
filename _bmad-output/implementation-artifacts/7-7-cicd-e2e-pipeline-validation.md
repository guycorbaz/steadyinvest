# Story 7.7: CI/CD E2E Pipeline Validation

Status: in-progress

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an **admin**,
I want the CI/CD E2E test pipeline validated in real GitHub Actions,
So that code changes are reliably tested before deployment.

## Acceptance Criteria

1. **Given** the `.github/workflows/e2e.yaml` file exists in the repository
   **When** a push or pull request triggers the workflow
   **Then** the pipeline runs all 23+ existing E2E tests to completion
   **And** the pipeline reports pass/fail status accurately

2. **Given** the E2E pipeline runs in GitHub Actions
   **When** the pipeline requires MariaDB and the backend service
   **Then** the workflow correctly provisions database services and backend startup
   **And** tests can reach the running application

3. **Given** any test failures occur in the pipeline
   **When** the pipeline completes
   **Then** failure output includes sufficient detail (test name, assertion message, screenshot if applicable) to diagnose the issue without local reproduction

## Critical Path

Tasks 1, 3, and 7 are **pass/fail gates** — if any of these fail, zero E2E tests will run. Prioritize them first.

## Tasks / Subtasks

- [x] Task 1: **[CRITICAL]** Fix WASM build target setup (AC: #1, #2)
  - [x] 1.1 Add `targets: wasm32-unknown-unknown` to the `dtolnay/rust-toolchain@stable` `with:` block in the E2E job — this is the idiomatic way to install WASM target. Without it, `trunk build` WILL fail immediately
  - [x] 1.2 Verify the unit-tests job does NOT need the WASM target (it only runs `cargo test --workspace --exclude e2e-tests` which skips frontend compilation)

- [x] Task 2: Fix trunk installation speed (AC: #2)
  - [x] 2.1 Replace `cargo install trunk` (compiles from source, ~5-10 min) with `cargo-bins/cargo-binstall@main` action followed by `cargo binstall trunk --no-confirm` (~10 seconds, downloads prebuilt binary)
  - [x] 2.2 Verify trunk version compatibility with Leptos 0.8 and existing `Trunk.toml` configuration

- [x] Task 3: **[CRITICAL]** Fix SPA routing for frontend serving (AC: #1, #2)
  - [x] 3.1 `python3 -m http.server` does NOT support SPA fallback routing — client-side routes like `/library`, `/system-monitor`, `/audit-log` will return 404. At least 14 of 23 E2E tests navigate to non-root routes and WILL fail
  - [x] 3.2 Replace with an inline Python SPA-aware server after `trunk build`. Use a one-liner Python script that serves `dist/` with index.html fallback for missing paths:
    ```bash
    python3 -c "
    import http.server, functools, pathlib
    class H(http.server.SimpleHTTPRequestHandler):
        def do_GET(self):
            if not pathlib.Path(self.translate_path(self.path)).exists():
                self.path = '/index.html'
            super().do_GET()
    http.server.HTTPServer(('', 5173), functools.partial(H, directory='dist')).serve_forever()
    " &
    ```
  - [x] 3.3 Do NOT use `trunk serve` — it recompiles on the fly and adds file watchers; we already have `trunk build` output in `dist/`. The Python SPA server is zero-dependency and serves prebuilt artifacts
  - [x] 3.4 Add a frontend readiness check after starting the server (curl `http://localhost:5173` until 200)

- [x] Task 4: Add backend health check instead of blind sleep (AC: #2)
  - [x] 4.1 Replace `sleep 5` after backend startup with a health check loop. Using `GET /api/v1/system/health` from `system.rs` controller (confirmed exists at `backend/src/controllers/system.rs:47`)
  - [x] 4.2 Using `/api/v1/system/health` endpoint — confirmed in routes at `system.rs:100-106`
  - [x] 4.3 Set a hard timeout — if the backend isn't up after 60 seconds (30 iterations × 2s), fail the job explicitly with `cat backend.log` for diagnostics

- [x] Task 5: Ensure database configuration for CI (AC: #2)
  - [x] 5.1 Loco 0.16 respects the `DATABASE_URL` environment variable, which overrides `database.uri` in config files. No `ci.yaml` needed — set `DATABASE_URL` as a **job-level** env var (not step-level) to ensure consistency across all steps in the E2E job
  - [x] 5.2 The backend config has `auto_migrate: true` in `development.yaml` which will run migrations on startup. This is sufficient for CI — migrations run automatically when the backend starts
  - [x] 5.3 `DATABASE_URL` promoted to job level for both unit-tests and e2e jobs

- [x] Task 6: Add failure diagnostic artifacts (AC: #3)
  - [x] 6.1 Modify `tests/e2e/src/common/mod.rs` to add screenshot-on-failure capability to `TestContext`. Added `save_screenshot(test_name: &str)` method that calls `self.driver.screenshot()` and saves to `./screenshots/{test_name}.png`
  - [x] 6.2 Create a `screenshots/` directory in the workflow before tests run: `mkdir -p screenshots`
  - [x] 6.3 Redirect backend startup output to a log file for diagnostics: `cargo run -p backend > backend.log 2>&1 &`
  - [x] 6.4 Add `actions/upload-artifact@v4` step with `if: always()` to upload screenshots/ and backend.log with 7-day retention
  - [x] 6.5 Run E2E tests with `-- --nocapture` flag for verbose test output: `cargo test -p e2e-tests -- --test-threads=1 --nocapture`

- [x] Task 7: **[CRITICAL]** Fix ChromeDriver version alignment (AC: #1, #2)
  - [x] 7.1 Use `browser-actions/setup-chrome@v1` with `install-chromedriver: true` to ensure Chrome and ChromeDriver versions match exactly. Version mismatch causes a cryptic "session not created" error that fails ALL E2E tests
  - [x] 7.2 Verify ChromeDriver is on PATH after setup and the `chromedriver --port=9515 &` command works — confirmed via web research that `install-chromedriver: true` adds chromedriver to PATH
  - [x] 7.3 `install-chromedriver: true` is confirmed supported by `browser-actions/setup-chrome@v1` — no fallback to `nanasess/setup-chromedriver` needed

- [ ] Task 8: Push and validate pipeline (AC: #1, #2, #3)
  - [x] 8.1 After all fixes, `cargo check` (full workspace) passes locally
  - [ ] 8.2 Commit and push to trigger the GitHub Actions workflow
  - [ ] 8.3 Monitor the pipeline execution, verify all 23 E2E tests pass
  - [ ] 8.4 If tests fail, use uploaded artifacts (screenshots, backend.log) to diagnose and iterate
  - [ ] 8.5 Verify the unit-tests job also passes (57 backend tests)

- [ ] Task 9: Verification (AC: all)
  - [ ] 9.1 Confirm the pipeline runs all 23+ E2E tests to completion
  - [ ] 9.2 Confirm pass/fail status is accurately reported
  - [ ] 9.3 Confirm MariaDB and backend are correctly provisioned
  - [ ] 9.4 Confirm failure output has sufficient detail for diagnosis (screenshots uploaded, backend.log available)
  - [x] 9.5 Confirm `cargo check` (full workspace) passes locally after changes

## Dev Notes

### Critical Architecture Constraints

**Cardinal Rule:** All calculation logic lives in `crates/steady-invest-logic`. This story does NOT involve calculation logic — it's infrastructure/CI. No steady-invest-logic changes needed.

**No Feature Code Changes:** This story is purely about CI/CD pipeline validation. No application code should be changed (unless a small fix is needed to make tests pass in CI, like environment-specific configuration).

**Test Infrastructure vs. Test Logic:** Modifying `tests/e2e/src/common/mod.rs` for CI diagnostic capabilities (screenshot-on-failure) is IN SCOPE. Modifying individual E2E test assertions or flows is NOT.

**Pipeline Duration Expectation:** The E2E job recompiles the full workspace (~15-20 min) because GitHub Actions cannot share build caches between jobs within the same workflow run. `Swatinem/rust-cache@v2` helps across runs but not across jobs. This is expected and not a blocker.

### Existing Infrastructure (MUST BUILD ON)

**GitHub Actions Workflow** (`.github/workflows/e2e.yaml`) — already exists with two jobs:

```
Job 1: unit-tests
  ├── MariaDB 11 service container
  ├── Rust stable toolchain + cache
  └── cargo test --workspace --exclude e2e-tests

Job 2: e2e (depends on unit-tests)
  ├── MariaDB 11 service container
  ├── Rust stable toolchain + cache
  ├── Chrome + ChromeDriver
  ├── trunk install + build
  ├── Backend server startup
  ├── Frontend build + serve
  └── cargo test -p e2e-tests -- --test-threads=1
```

**E2E Test Suite** (`tests/e2e/`) — ThirtyFour v0.31 WebDriver framework:
- `src/lib.rs` — 5 search tests
- `src/epic3_tests.rs` — 2 tests (override, kinetic)
- `src/epic4_tests.rs` — 2 tests (locking, persistence)
- `src/epic5_tests.rs` — 3 tests (system monitor, audit, latency)
- `src/epic6_tests.rs` — 11 tests (workflow, sliders, navigation, keyboard)
- `src/common/mod.rs` — TestContext (WebDriver lifecycle, headless config)
- **Total: 23 E2E tests**

**Backend Tests** — 57 tests across 8 modules in `backend/tests/`:
- `snapshots.rs` (17), `auth.rs` (12), `users.rs` (10), `harvest.rs` (6), `exchange_rates.rs` (4), `audit.rs` (2), `system.rs` (2), `tickers.rs` (2), `analyses.rs` (1)

**TestContext Configuration** (`tests/e2e/src/common/mod.rs`):
- Reads `CHROME_DRIVER_URL` (default: `http://localhost:9515`)
- Reads `BASE_URL` (default: `http://localhost:5173`)
- Reads `HEADLESS` env var for headless Chrome mode
- Chrome capabilities: `--headless`, `--no-sandbox`, `--disable-dev-shm-usage`

### Known Issues in Current Workflow (MUST FIX)

**Issue 1 — No WASM target:**
`trunk build` requires `wasm32-unknown-unknown` target. The `dtolnay/rust-toolchain@stable` action doesn't install it by default. The build WILL fail without it.

**Issue 2 — Slow trunk installation:**
`cargo install trunk` compiles from source (~5-10 minutes). Use `cargo-binstall` or download prebuilt binary.

**Issue 3 — SPA routing broken:**
`python3 -m http.server 5173 --directory dist` is a basic static file server. It does NOT support SPA fallback routing. Any E2E test that navigates to `/library`, `/system-monitor`, `/audit-log`, or any non-root route will get a 404 instead of the SPA index.html. The tests in `epic5_tests.rs` navigate to `/system-monitor` and `/audit-log` — these WILL fail.

**Issue 4 — Fragile backend startup:**
`sleep 5` is unreliable in CI (cold caches, container startup delays). A health check loop is more robust.

**Issue 5 — No failure artifacts:**
When tests fail in CI, there's no way to diagnose without local reproduction. Need screenshot/log upload on failure.

**Issue 6 — ChromeDriver alignment:**
`browser-actions/setup-chrome@v1` installs Chrome stable but the matching ChromeDriver setup needs verification. Version mismatch = all E2E tests fail with session creation error.

**Issue 7 — Database configuration for CI:**
The backend's `config/development.yaml` has `database.uri: mysql://steadyinvest:password@127.0.0.1:3306/steadyinvest` which doesn't match the CI service container credentials (`steadyinvest:steadyinvest_test@localhost:3306/steadyinvest_test`). The `DATABASE_URL` env var must override this. Verify Loco 0.16 respects `DATABASE_URL` over config file.

### Backend Configuration Files

**`backend/config/development.yaml`** (default for `cargo run`):
```yaml
database:
  uri: mysql://steadyinvest:password@127.0.0.1:3306/steadyinvest
  auto_migrate: true
```

**`backend/config/test.yaml`** (used when `LOCO_ENV=test`):
```yaml
database:
  uri: mysql://steadyinvest:1000cpsvqrE$@192.168.1.5:3306/steadyinvest_test
  dangerously_truncate: true
  dangerously_recreate: true
```

Neither config matches CI credentials. Loco 0.16 respects the `DATABASE_URL` environment variable which overrides `database.uri` in config. Set `DATABASE_URL` as a **job-level** env var — no `ci.yaml` needed.

### Frontend Build Details

**`frontend/Cargo.toml`** depends on `leptos 0.8` with `csr` feature. Built with `trunk` which:
1. Compiles the Rust code to WASM via `wasm32-unknown-unknown` target
2. Processes `frontend/index.html` (or `Trunk.toml` config) for asset bundling
3. Outputs to `frontend/dist/`

Check for `Trunk.toml` or `frontend/Trunk.toml` for configuration that might affect the build.

### Previous Story Learnings (from Story 7.6)

- `request::<App, _, _>` (NOT `request::<App, Migrator, _>`) for Loco 0.16 test pattern
- `gloo-net` is the frontend HTTP client (NOT `reqwest`)
- `LocalResource` for data fetching in Leptos 0.8
- Library page at `/library` uses client-side routing — this confirms SPA routing is critical for E2E tests
- E2E test for Command Strip navigation (`test_command_strip_navigation_all_pages` in `epic6_tests.rs`) navigates to ALL pages including `/library` — will fail if SPA routing doesn't work

### Git Intelligence

Recent commits (all Story 7.x):
```
ceec3ca feat: complete Story 7.6 — Library view & analysis browsing
c82c299 feat: complete Story 7.5 — exchange rate service with DB fallback
ebbbe2f feat: complete Story 7.4 — static chart image capture at lock time
e75a5f6 feat: complete Story 7.3 — analysis snapshot API with code review fixes
957108a feat: complete Story 7.2 — pre-migration backup & snapshot schema
e0bcf71 fix: code review fixes for Story 7.1 (PDF export, chart, legend)
ab453f0 feat: complete Story 7.1 — PDF export, chart height & legend
```

### What NOT To Do

- Do NOT modify application source code (backend controllers, frontend components, steady-invest-logic)
- Do NOT change database schemas or migrations
- Do NOT modify individual E2E test assertions or flows — only `common/mod.rs` test infrastructure for CI diagnostics is in scope
- Do NOT add new E2E tests — this story is about validating the pipeline, not adding test coverage
- Do NOT change `backend/config/development.yaml` or `backend/config/test.yaml`
- Do NOT create `backend/config/ci.yaml` — use `DATABASE_URL` env var override instead
- Do NOT push force to main/master
- Do NOT remove or modify existing workflow triggers (push to main/master, pull_request)

### Project Structure Notes

Files to MODIFY:
- `.github/workflows/e2e.yaml` — Fix all identified issues (primary deliverable)
- `tests/e2e/src/common/mod.rs` — Add screenshot-on-failure capability for CI diagnostics

Files NOT to modify:
- `backend/src/` — No application code changes
- `frontend/src/` — No frontend code changes
- `crates/steady-invest-logic/` — No calculation logic
- `tests/e2e/src/lib.rs`, `epic3_tests.rs`, `epic4_tests.rs`, `epic5_tests.rs`, `epic6_tests.rs` — No test logic changes
- `backend/config/development.yaml` — Don't change local dev config
- `backend/config/test.yaml` — Don't change unit test config

Files NOT to create:
- `backend/config/ci.yaml` — Not needed; `DATABASE_URL` env var overrides config

### Definition of Done

- [ ] `.github/workflows/e2e.yaml` updated with all fixes
- [ ] WASM target installed in CI
- [ ] Trunk installed efficiently (binstall or prebuilt)
- [ ] SPA routing works for all frontend routes in CI
- [ ] Backend starts reliably with health check verification
- [ ] Database provisioned and migrated correctly in CI
- [ ] Chrome + ChromeDriver versions aligned
- [ ] Failure artifacts (screenshots, logs) uploaded on test failure
- [ ] Pipeline pushed and triggered in GitHub Actions
- [ ] All 23 E2E tests pass in CI
- [ ] All 57 backend tests pass in CI (unit-tests job)
- [ ] `cargo check` (full workspace) passes locally

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 7, Story 7.7]
- [Source: _bmad-output/planning-artifacts/architecture.md — Infrastructure & Deployment, Testing Standards]
- [Source: .github/workflows/e2e.yaml — Existing CI/CD pipeline]
- [Source: tests/e2e/src/common/mod.rs — TestContext WebDriver setup]
- [Source: tests/e2e/Cargo.toml — E2E test dependencies (ThirtyFour 0.31)]
- [Source: backend/config/development.yaml — Backend development config]
- [Source: backend/config/test.yaml — Backend test config]
- [Source: _bmad-output/implementation-artifacts/7-6-library-view-analysis-browsing.md — Previous story learnings]
- [Source: _bmad-output/planning-artifacts/implementation-readiness-report-2026-02-11.md — Epic 6 retro action items]

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

### File List
