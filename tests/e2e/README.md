# NAIC E2E Test Framework (Rust + ThirtyFour)

This directory contains the End-to-End (E2E) testing suite for the **naic** project, built using [ThirtyFour](https://github.com/stevepryde/thirtyfour), a Selenium-driven WebDriver client for Rust.

## Prerequisites

- **Rust Toolchain**: 1.80+ (managed via workspace)
- **ChromeDriver**: Must be running for local execution.

  ```bash
  chromedriver --port=9515 --url-base=/wd/hub
  ```

## Setup & Execution

### Local Development

To run all E2E tests:

```bash
cargo test -p e2e-tests
```

### Configuration

Environment variables can be set in a `.env` file in the project root:

- `BASE_URL`: The URL of the frontend (default: `http://localhost:5173`)
- `CHROME_DRIVER_URL`: The URL for the ChromeDriver (default: `http://localhost:9515`)

## Architecture

- **`tests/e2e/src/common/mod.rs`**: Contains the `TestContext`, which manages the `WebDriver` lifecycle, navigation helpers, and cleanup hooks.
- **`tests/e2e/src/lib.rs`**: (This file) Contains the actual test implementations organized by feature modules.
- **Fixtures**: Leveraging Rust's `TestContext` pattern for composable setup.

## Best Practices

1. **Selector Strategy**: Use `By::ClassName` or stable IDs. Avoid brittle XPath where possible.
2. **Isolation**: Use the `TestContext` to ensure each test has a fresh browser session or proper cleanup.
3. **Deterministic Waits**: Use `ctx.driver.query(...).first().await?` which includes built-in polling for stability.

## CI Integration

In CI environments, ensure ChromeDriver is started as a service before running the test suite.
