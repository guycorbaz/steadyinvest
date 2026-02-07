# Test Automation Summary: Historical Data Adjustment

## Generated Tests

### API Tests

- [x] `backend/tests/requests/harvest.rs`
  - `verify_split_adjustment_metadata`: Confirms `is_split_adjusted` flag is `true` for split-affected tickers (AAPL).
  - `verify_no_split_adjustment_for_standard_ticker`: Confirms flag is `false` for standard tickers (MSFT).
  - Also verified existing `can_harvest_ticker` and `cannot_harvest_empty_ticker`.

### E2E Tests

- [x] `tests/e2e/src/lib.rs`
  - `test_split_adjustment_indicator`: Verifies "Split-Adjusted" badge is visible for AAPL.
  - `test_no_split_adjustment_indicator`: Verifies badge is HIDDEN for MSFT.

## Coverage

- **API logic**: 100% coverage of split-adjustment flag states via mock integration.
- **UI visibility**: 100% coverage of badge logic for adjusted vs non-adjusted data sets.

## Verification Results

- **Backend Tests**: PASSED (Verified via `cargo test -p backend harvest` with dedicated test DB).
- **Unit Tests**: PASSED (Logic layer math verified).

## Next Steps

- Continue with **Story 1.5: Multi-Currency Normalization**.
- Consider adding real-world split data tests once Yahoo Finance integration is live.
