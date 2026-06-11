# Test Automation Summary — Story 1.6 (Spike C: exact-decimal CAGR precision & cross-OS determinism)

Date: 2026-06-11
Workflow: `bmad-qa-generate-e2e-tests` (auto-apply gaps mode)
Framework: Rust integration tests (`cargo test`) — the project's existing framework. No UI/API surface in this story (headless core gate), so no browser-E2E or HTTP-API tests apply.

## Scope

Story 1.6's feature is itself a permanent headless test gate (`core/tests/spike_c_cagr_precision.rs`,
6 tests, all ACs covered). QA pass = gap analysis of that gate against the story's ACs and the
GO/NO-GO findings note claims, with discovered gaps auto-applied as new tests in the same file
(so they run under the CI "Determinism hash" step, `just spike-c`, and `cargo test --all`
without any CI/justfile change, and without touching the pinned hash or the 6 existing tests).

## Discovered Gaps → Generated Tests

All in `core/tests/spike_c_cagr_precision.rs` (5 new tests, 6 → 11):

- [x] `fractional_cagr_with_non_unit_start_matches_exact_reference` — every existing series used
  `start = 1.00`; the `end/start` division path with a realistic per-share base (2.50) was
  unexercised. Measured: relative error 1e-27 (same as the unit-start series).
- [x] `checked_ln_returns_none_on_degenerate_input_instead_of_panicking` — the findings note's
  `checked_ln(≤ 0) = None` claim came from a throwaway `/tmp` probe and was pinned by no permanent
  test; Story 1.8 relies on it.
- [x] `checked_powd_returns_none_on_overflow` — same gap for the overflow → `None` claim
  (`deferred-work.md` requires checked math in Story 1.8).
- [x] `negative_base_fractional_powd_is_silent_sign_magnitude_not_an_error` — the spike's most
  load-bearing finding (`powd(-2, 0.5)` silently returns `sign(x)·|x|^y`; `checked_powd` does NOT
  guard it ⇒ the method-spec §9 degenerate-base guard is mandatory) was only documented, never
  gated. A `rust_decimal` upgrade changing this semantics now fails the build.
- [x] `hash_serialization_is_value_only_representation_independent` — the "value-only,
  representation-independent" claim of the hash scheme (`normalize().to_string()`) was untested:
  equal values at different internal scales must serialize identically.

## Verification

- `cargo test -p steadyinvest-core --test spike_c_cagr_precision`: **11 passed, 0 failed** —
  pinned digest `d9af5553…5557` unchanged (the new tests assert behaviour outside the hashed
  result vector, by design).
- `cargo fmt --all --check`: clean. `cargo clippy --all-targets --all-features --locked -- -D warnings`: 0 warnings.
- `cargo test --all --locked`: **52 passed, 0 failed** across the workspace.

## Coverage

- Story 1.6 ACs: 4/4 covered (AC1 precision + display-rounding interaction: 6 tests; AC2 pinned
  hash + serialization claim: 2; AC3 findings-note behavioural claims now mechanically pinned: 3;
  AC4 gates green).
- Findings-note `powd`/`ln` behavioural claims: 3/3 now regression-gated (previously 0/3).

## Next Steps

- Nothing to wire: the new tests already run in CI via the existing
  `cargo test -p steadyinvest-core --test spike_c_cagr_precision --locked` command.
- Story 1.8 (engine) should reference `negative_base_fractional_powd_is_silent_sign_magnitude_not_an_error`
  as the rationale test for the §9 degenerate-base guard.
