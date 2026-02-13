# SteadyInvest E2E Test Framework (Rust + ThirtyFour)

This directory contains the End-to-End (E2E) testing suite for the **SteadyInvest** project, built using [ThirtyFour](https://github.com/stevepryde/thirtyfour), a Selenium-driven WebDriver client for Rust.

## Prerequisites

- **Rust Toolchain**: 1.80+ (managed via workspace)
- **ChromeDriver**: Must be running for local execution.
- **Running Application**: Both backend and frontend must be running.

## Test Inventory

| File | Tests | Coverage |
|------|-------|----------|
| `lib.rs` | 5 | Ticker search, autocomplete, data retrieval, split indicators, error handling |
| `epic3_tests.rs` | 2 | Manual data override flow, kinetic chart dragging |
| `epic4_tests.rs` | 2 | Thesis locking flow, persistence UI controls |
| `epic5_tests.rs` | 3 | System monitor dashboard, audit log page, latency indicator |
| `epic6_tests.rs` | 11 | Full analyst workflow, slider independence, navigation, keyboard, modal dismiss |

**Total: 23 tests**

## Setup & Execution

### Local Development

1. Start ChromeDriver:
   ```bash
   chromedriver --port=9515
   ```

2. Start the backend (in project root):
   ```bash
   cargo run -p backend
   ```

3. Start the frontend (in `frontend/` directory):
   ```bash
   trunk serve
   ```

4. Run all E2E tests:
   ```bash
   cargo test -p e2e-tests
   ```

   Run tests sequentially (recommended to avoid port conflicts):
   ```bash
   cargo test -p e2e-tests -- --test-threads=1
   ```

### Headless Mode (CI)

Set the `HEADLESS` environment variable to run Chrome without a visible window:

```bash
HEADLESS=true cargo test -p e2e-tests -- --test-threads=1
```

### Configuration

Environment variables (can be set in a `.env` file in the project root):

| Variable | Default | Description |
|----------|---------|-------------|
| `BASE_URL` | `http://localhost:5173` | Frontend URL |
| `CHROME_DRIVER_URL` | `http://localhost:9515` | ChromeDriver URL |
| `HEADLESS` | (unset) | Set to `"true"` for headless Chrome |

## Architecture

```
tests/e2e/
├── Cargo.toml              # ThirtyFour v0.31, tokio, anyhow, rstest
├── README.md               # This file
└── src/
    ├── lib.rs              # Module declarations + search tests
    ├── epic3_tests.rs      # Override + chart drag tests
    ├── epic4_tests.rs      # Thesis locking + persistence tests
    ├── epic5_tests.rs      # System monitor + audit log tests
    ├── epic6_tests.rs      # Workflow, slider, navigation, keyboard tests
    └── common/
        └── mod.rs          # TestContext (WebDriver lifecycle, headless support)
```

### TestContext

The `TestContext` struct in `common/mod.rs` manages browser session lifecycle:
- `TestContext::new()` — Creates a WebDriver session (headless if `HEADLESS=true`)
- `ctx.navigate(path)` — Navigates to `BASE_URL + path`
- `ctx.cleanup()` — Quits the browser session

### Test Naming Convention

Tests follow the pattern: `test_<feature>_<expected_behavior>`

Examples:
- `test_complete_analyst_workflow`
- `test_sliders_independent_no_cross_contamination`
- `test_command_strip_navigation_all_pages`

## Best Practices

1. **Selector Strategy**: Use `By::ClassName` or stable CSS selectors. Avoid brittle XPath where possible.
2. **Isolation**: Each test creates its own `TestContext` and calls `cleanup()` at the end.
3. **Deterministic Waits**: Use `ctx.driver.query(...).wait(timeout, poll).first().await?` instead of `thread::sleep()`.
4. **Slider Manipulation**: Use JavaScript to set range input values and dispatch input events (see `set_slider_value` helper in `epic6_tests.rs`).
5. **Independence**: No test should depend on another test's state or execution order.

## CI Integration

E2E tests run in GitHub Actions via `.github/workflows/e2e.yaml`:
- ChromeDriver is started as a background process
- MariaDB runs as a service container
- Backend starts in test mode
- Frontend is built with `trunk` and served statically
- Tests run with `HEADLESS=true`
- Test failures block the pipeline
