# Spike C — exact-decimal CAGR precision & cross-OS determinism

**Story:** 1.6 · **Date:** 2026-06-11 · **Type:** spike (go/no-go) — but unlike Spikes A/B the
artifact is **KEPT**: the test is the permanent CI determinism gate for fractional `powd` math.
**Question:** does `rust_decimal` 1.42 (`maths` feature, `powd`) give **exact, reproducible**
compound-growth results — integer-power projections exact, fractional-power CAGR within a tight
asserted bound, bit-identical across OS — so the real engine (Stories 1.7/1.8) can be built on the
no-float decision (exact decimal everywhere in the decision chain)?

## What was built

`core/tests/spike_c_cagr_precision.rs` (headless, sub-second; run with `just spike-c` to see the
measured deviations on stderr):

- **Exact-by-construction series** — reference CAGRs are exact with no hand-typed transcendental
  digits: end values built by exact integer multiplication in the test (`1.00 × 1.1⁵` over
  5 intervals ⇒ true CAGR = 0.10 exactly; `1.08⁹` over 9 intervals ⇒ 0.08 exactly).
- **Fractional path** (`(end/start).powd(1/n) − 1`, the `exp(y·ln x)` series): relative error
  asserted ≤ **1e-9** and the exact measured deviation printed.
- **Round-trip metamorphic check**: the fractional `powd` result re-raised to the integer power `n`
  compared back to `end/start` — pins the series error without trusting any reference digits.
- **Integer path**: `1000 × 1.15⁵` via `powd(5)` asserted **exactly equal** (no tolerance) to the
  same product by plain multiplication, and to `2011.3571875`.
- **Display interaction**: full-precision CAGR → `core::rounding::round_for_display(·, Percent)`
  (1 dp, half-up, §8) gives the hand-expected `10.0`; rounding stays display-only, never mid-chain.
- **Pinned determinism hash**: SHA-256 over the result vector (both CAGRs + projection, full
  precision), serialized exactly like `core::determinism_hash` (`normalize().to_string()` + `\n`
  per value — value-only, representation-independent), asserted against a pinned constant and wired
  into the CI "Determinism hash" step. The existing Story 1.1 probe/hash was **not** modified.

## Results — RUN 2026-06-11 (Linux, rustc 1.96, rust_decimal 1.42.0, debug & CI `--locked`)

| Metric | Value |
|--------|-------|
| Integer-power projection (`1000 × 1.15⁵`) | **exact** — `assert_eq!`, zero deviation (`2011.3571875`) |
| Fractional CAGR, n=5 (`1/n = 0.2` exact exponent) | `0.1000000000000000000000000001` → relative error **1e-27** |
| Fractional CAGR, n=9 (`1/9` truncated at 28 digits) | `0.0800000000000000000000000000` → relative error **0** (exact at 28 digits) |
| Round-trip `((x)^(1/n))^n` vs `x` | n=5: **4e-28** · n=9: **0** |
| Asserted bound (never flakes) | relative error ≤ **1e-9** — measured is ~18 orders of magnitude inside it |
| vs golden tolerance ±0.5% (method spec §7) | measured error ~**24 orders of magnitude** tighter |
| vs Percent display quantum 0.05 at 1 dp (§8) | unmeasurably far inside; half-up display value exact (`10.0`) |
| Pinned determinism hash (cross-OS contract) | `d9af555376f754c8543bb4a6c94267257c227612df4a18e7ee6cba02b57e5557` |
| Gates (`fmt`, `clippy -D warnings`, `test --all --locked`, `deny`) | ✅ green |

**Precision statement for the method spec:** with `rust_decimal` 1.42 (`maths`), integer-power
compound projections are **exact**; fractional-power CAGR (`exp(y·ln x)` series) carries a
deterministic series-truncation error in the last 1–2 of the 28–29 significant digits (measured
relative error ≤ ~1e-27 on realistic SSG magnitudes) — i.e. invisible at any §8 display scale and
~24 orders of magnitude inside the §7 golden tolerance. The error is identical on every platform
(pure-Rust series, no libm, no `f64`), which is exactly why a pinned hash works as the contract.
If a §8 computation-precision amendment is wanted, file it as a **GitHub issue** (spec edits couple
to a `METHOD_VERSION` bump — out of spike scope; issues are the single source of truth).

## Cross-OS caveat (Linux-only CI, decision 2026-06-09)

The pinned constant *is* the cross-OS assertion mechanism: it compares every runner to one recorded
truth, not two runners to each other. CI currently runs it on Linux only; determinism across
Windows/macOS holds **by construction** (pure-Rust `rust_decimal`, `maths` has no platform libm,
no `f64` in the decision chain) and becomes **mechanically verified on all three OS the day the
matrix is restored** — no test change needed, the same pinned hash simply runs everywhere.

## `powd`/`ln` panic vs `checked_*` behaviour (for Story 1.8) — observed empirically (1.42.0)

| Call | Behaviour |
|------|-----------|
| `ln(0)`, `ln(-1)`, `log10(0)` | **panics** |
| `powd` on overflow (e.g. `(1e19)^5`) | **panics** |
| `checked_ln(≤ 0)` | `None` (no panic) |
| `checked_powd` on overflow | `None` (no panic) |
| `powd(-2, 0.5)` (negative base, fractional exp) | ⚠️ **no panic — silently returns `-1.4142…`** (computes `sign(x)·|x|^y`; mathematically the result is complex) |
| `powd(0, 0)` | returns `1` (convention) |

Implications for Story 1.8 (restating the 1-1 review item in `deferred-work.md`):

- Use **`checked_powd` / `checked_ln`** (or guard inputs) in the engine — never the panicking paths.
- The **silent negative-base result is the sharpest edge**: `checked_powd` does NOT protect against
  it (`checked_powd(-2, 0.5) = Some(-1.4142…)`). The method-spec **§9 guard is load-bearing**:
  CAGR with base ≤ 0 / sign-crossing series must be ruled `unknown/insufficient` **before** any
  `powd`/`ln` call — the engine must never reach the math with a degenerate base.
- The `maths-nopanic` feature exists as an alternative, but explicit `checked_*` + §9 guards keep
  failure handling visible at the call site (preferred).

## Decision — **GO** (2026-06-11)

- [x] **GO** — `rust_decimal` (+`maths`) delivers exact integer-power projections, fractional CAGR
  ~18 orders of magnitude inside the asserted 1e-9 bound (~24 inside the ±0.5% golden tolerance),
  and a reproducible pinned hash. The exact-decimal no-float decision is **validated end to end**;
  Stories 1.7/1.8 build the real engine on `rust_decimal` as architected, using `checked_*` paths
  and the §9 degenerate-base guard.
- [ ] ~~NO-GO fallback~~ — not needed. (Per the architecture, the fallback would have been scaled
  integers or reformulating without transcendentals — **never `f64`** in the decision chain.)

Unlike Spikes A/B there is no on-display step: the evidence above is fully automated. The test
`core/tests/spike_c_cagr_precision.rs` stays as the permanent determinism gate (e.g. against a
future `rust_decimal` upgrade silently changing the series math).

**Post-spike QA hardening (same day):** a QA pass added 5 tests to the same file (6 → 11 — see
`_bmad-output/implementation-artifacts/tests/test-summary.md`), mechanically pinning the
behavioural claims in the table above (`checked_ln(≤ 0) = None`, `checked_powd` overflow → `None`,
the silent negative-base `sign(x)·|x|^y` result) plus a non-unit-start CAGR series and the
value-only serialization claim of the hash scheme. The pinned digest is unchanged.

**Second review pass (same day):** a follow-up review added 3 more tests (11 → 14), pinning the
remaining rows of the behaviour table that until then rested only on the throwaway `/tmp` probe:
bare `ln(0)` panics and bare `powd` overflow panics (`#[should_panic]` — the contrast that makes
the `checked_*` paths mandatory), and the `powd(0, 0) = 1` convention. Every row of the table is
now regression-gated. The pinned digest is unchanged.
