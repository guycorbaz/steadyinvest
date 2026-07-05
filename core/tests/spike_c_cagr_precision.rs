//! Spike C (Story 1.6) — exact-decimal CAGR precision & cross-OS determinism gate.
//!
//! Unlike Spikes A/B this artifact is KEPT permanently: it is the headless CI regression gate that
//! pins fractional `powd` behaviour (the `exp(y·ln x)` series) against e.g. a future `rust_decimal`
//! upgrade silently changing the math. Findings + GO/NO-GO decision:
//! `docs/spikes/spike-c-decimal-cagr-determinism.md`.
//!
//! The pinned digest below is the cross-OS contract: it does not compare two runners, it compares
//! every runner to one recorded truth, so any OS that ever runs this test must reproduce it. CI is
//! Linux-only for now (decision 2026-06-09); determinism holds by construction — pure-Rust
//! `rust_decimal` (`maths` is pure Rust, no platform libm), no `f64` in the decision chain.
//!
//! Run `just spike-c` (adds `--nocapture`) to read the measured deviations on stderr.

use rust_decimal::{Decimal, MathematicalOps};
use sha2::{Digest, Sha256};
use steadyinvest_core::rounding::{DisplayField, round_for_display};

/// Asserted upper bound on the fractional-`powd` relative error. Deliberately conservative so the
/// gate never flakes (the measured error is orders of magnitude smaller — see the findings note),
/// while still sitting ~6-7 orders of magnitude inside both the ±0.5% golden tolerance (method
/// spec §7) and the 0.05 Percent display quantum (§8).
fn max_relative_error() -> Decimal {
    Decimal::new(1, 9) // 1e-9
}

/// Exact integer power by repeated multiplication — the reference path. Every step is an exact
/// `Decimal` multiply at these magnitudes (well under 28 significant digits): no `powd`, no series,
/// no hand-typed long constants.
fn pow_int_exact(base: Decimal, n: u32) -> Decimal {
    let mut acc = Decimal::ONE;
    for _ in 0..n {
        acc *= base;
    }
    acc
}

/// CAGR over `n` intervals: `(end/start)^(1/n) − 1`. The `1/n` exponent is fractional, so `powd`
/// takes its transcendental `exp(y·ln x)` series path — the path under test.
fn cagr(start: Decimal, end: Decimal, n: u32) -> Decimal {
    (end / start).powd(Decimal::ONE / Decimal::from(n)) - Decimal::ONE
}

fn relative_error(measured: Decimal, reference: Decimal) -> Decimal {
    ((measured - reference) / reference).abs()
}

/// Series 1 — headline hand-match: EPS 1.00 → 1.1⁵ over 5 intervals ⇒ true CAGR = 0.10 exactly.
/// `1/5 = 0.2` terminates, so the exponent itself is exact and the measured deviation is purely
/// the `exp(y·ln x)` series-truncation error.
#[test]
fn fractional_cagr_matches_exact_reference_terminating_reciprocal() {
    let start = Decimal::new(100, 2); // 1.00
    let end = start * pow_int_exact(Decimal::new(11, 1), 5); // 1.61051, computed not hand-typed
    let true_cagr = Decimal::new(10, 2); // 0.10 exact by construction

    let measured = cagr(start, end, 5);
    let rel = relative_error(measured, true_cagr);
    eprintln!(
        "[spike-c] n=5 (1/n = 0.2, exact exponent): cagr = {measured}, relative error = {rel}"
    );
    assert!(
        rel <= max_relative_error(),
        "fractional powd CAGR drifted past the bound: cagr = {measured}, relative error = {rel}"
    );
}

/// Series 2 — second measured data point: 1.08⁹ over 9 intervals ⇒ true CAGR = 0.08. `1/9`
/// truncates at 28 digits (≈1e-28 exponent error — negligible against the 1e-9 bound), so this
/// exercises a slightly different error mix than the terminating-reciprocal series.
#[test]
fn fractional_cagr_second_series_nonterminating_reciprocal() {
    let start = Decimal::ONE;
    let end = start * pow_int_exact(Decimal::new(108, 2), 9); // 1.999004627104432128 exactly
    let true_cagr = Decimal::new(8, 2); // 0.08 exact by construction

    let measured = cagr(start, end, 9);
    let rel = relative_error(measured, true_cagr);
    eprintln!(
        "[spike-c] n=9 (1/n truncated at 28 digits): cagr = {measured}, relative error = {rel}"
    );
    assert!(
        rel <= max_relative_error(),
        "fractional powd CAGR drifted past the bound: cagr = {measured}, relative error = {rel}"
    );
}

/// Round-trip metamorphic check: `((end/start)^(1/n))^n` must come back to `end/start` (fractional
/// path down, exact integer path back up). Pins the series-approximation error without trusting
/// any typed-in reference digits at all.
#[test]
fn round_trip_metamorphic_pins_series_error_without_reference_digits() {
    for (rate, n) in [(Decimal::new(11, 1), 5u32), (Decimal::new(108, 2), 9)] {
        let ratio = pow_int_exact(rate, n); // end/start, exact
        let root = ratio.powd(Decimal::ONE / Decimal::from(n)); // fractional series path
        let back = pow_int_exact(root, n); // exact multiplication back up
        let rel = relative_error(back, ratio);
        eprintln!("[spike-c] round-trip n={n}: |(x^(1/n))^n − x| / x = {rel}");
        assert!(
            rel <= max_relative_error(),
            "round-trip drifted past the bound for n = {n}: relative error = {rel}"
        );
    }
}

/// Integer-power path (the 5-year projection `base × (1+g)^5`) multiplies exactly in `Decimal`:
/// zero deviation, `assert_eq!`, no tolerance. The expected constant is verified by exact
/// multiplication in the test too — not trusted from a hand-typed source.
#[test]
fn integer_power_projection_is_exact() {
    let base = Decimal::new(1000, 0); // 1000
    let factor = Decimal::new(115, 2); // 1.15

    let via_powd = base * factor.powd(Decimal::from(5u32));
    let via_exact_multiplication = base * pow_int_exact(factor, 5);

    assert_eq!(via_powd, via_exact_multiplication);
    assert_eq!(via_powd, Decimal::new(20_113_571_875, 7)); // 2011.3571875 exactly
}

/// Display-rounding interaction (§8): the full-precision CAGR is rounded **only at display**
/// (Percent: 1 dp, half-up) and lands on the hand-expected value — a true 10% growth shows 10.0.
/// Rounding never happens mid-chain; the hash below covers the unrounded values.
#[test]
fn full_precision_cagr_display_rounds_to_expected_percent() {
    let end = pow_int_exact(Decimal::new(11, 1), 5);
    let cagr_pct = cagr(Decimal::ONE, end, 5) * Decimal::ONE_HUNDRED;
    let displayed = round_for_display(cagr_pct, DisplayField::Percent);
    assert_eq!(displayed, Decimal::new(100, 1)); // 10.0
}

/// CAGR with a non-unit start: the `end/start` division path with a realistic per-share base
/// (EPS 2.50 → 2.50 × 1.1⁵ over 5 intervals ⇒ true CAGR = 0.10 exactly by construction).
#[test]
fn fractional_cagr_with_non_unit_start_matches_exact_reference() {
    let start = Decimal::new(250, 2); // 2.50
    let end = start * pow_int_exact(Decimal::new(11, 1), 5);
    let true_cagr = Decimal::new(10, 2); // 0.10 exact by construction

    let measured = cagr(start, end, 5);
    let rel = relative_error(measured, true_cagr);
    eprintln!("[spike-c] n=5, start=2.50: cagr = {measured}, relative error = {rel}");
    assert!(
        rel <= max_relative_error(),
        "fractional powd CAGR with non-unit start drifted past the bound: relative error = {rel}"
    );
}

/// Pins the `checked_*` panic-avoidance contract Story 1.8 relies on (findings note: `ln`/`log10`
/// panic on input ≤ 0; the checked variants are the non-panicking path). If a `rust_decimal`
/// upgrade changes these semantics, this gate must catch it before the engine does.
#[test]
fn checked_ln_returns_none_on_degenerate_input_instead_of_panicking() {
    assert_eq!(Decimal::ZERO.checked_ln(), None);
    assert_eq!(Decimal::NEGATIVE_ONE.checked_ln(), None);
    // Sanity: a valid input still computes.
    assert!(Decimal::TWO.checked_ln().is_some());
}

/// Pins `checked_powd` overflow → `None` (where bare `powd` panics) — the variant Story 1.8 must
/// use per `deferred-work.md`.
#[test]
fn checked_powd_returns_none_on_overflow() {
    assert_eq!(Decimal::MAX.checked_powd(Decimal::TWO), None);
    // Sanity: a non-overflowing power still computes.
    assert!(Decimal::TWO.checked_powd(Decimal::TWO).is_some());
}

/// ⚠️ Pins the silent negative-base behaviour found by the spike: `powd(-2, 0.5)` does NOT panic
/// and `checked_powd` does NOT return `None` — both return `sign(x)·|x|^y` (a mathematically wrong
/// "square root of a negative"). This is exactly why the method-spec §9 degenerate-base guard
/// (base ≤ 0 ⇒ `unknown/insufficient`, never computed) is load-bearing for Story 1.8. If an
/// upgrade changes this to a panic or `None`, the guard rationale must be revisited — fail here.
#[test]
fn negative_base_fractional_powd_is_silent_sign_magnitude_not_an_error() {
    let half = Decimal::new(5, 1); // 0.5
    let on_negative = (-Decimal::TWO).powd(half);
    let on_positive = Decimal::TWO.powd(half);

    assert_eq!(on_negative, -on_positive); // sign(x)·|x|^y semantics
    assert_eq!((-Decimal::TWO).checked_powd(half), Some(on_negative)); // checked does NOT guard it
}

/// Pins the panicking half of the `ln` row in the findings-note behaviour table: bare `ln` on a
/// degenerate input panics — the contrast that makes `checked_ln` mandatory for Story 1.8.
#[test]
#[should_panic]
fn bare_ln_panics_on_zero_unlike_checked_ln() {
    let _ = Decimal::ZERO.ln();
}

/// Pins the panicking half of the overflow row: bare `powd` panics where `checked_powd` returns
/// `None` — the contrast that makes `checked_powd` mandatory for Story 1.8.
#[test]
#[should_panic]
fn bare_powd_panics_on_overflow_unlike_checked_powd() {
    let _ = Decimal::MAX.powd(Decimal::TWO);
}

/// Pins the `powd(0, 0) = 1` convention recorded in the findings-note behaviour table.
#[test]
fn powd_zero_to_the_zero_is_one_by_convention() {
    assert_eq!(Decimal::ZERO.powd(Decimal::ZERO), Decimal::ONE);
}

/// Pins the "value-only, representation-independent" serialization claim of the hash scheme:
/// equal values with different internal scales must serialize to the same normalized string, so
/// the digest depends on numeric value alone.
#[test]
fn hash_serialization_is_value_only_representation_independent() {
    let pairs = [
        (Decimal::new(11, 1), Decimal::new(1100, 3)), // 1.1 vs 1.100
        (
            Decimal::new(20_113_571_875, 7), // 2011.3571875 (the projection value)
            Decimal::new(20_113_571_875_000, 10), // 2011.3571875000
        ),
    ];
    for (a, b) in pairs {
        assert_eq!(a, b);
        assert_eq!(a.normalize().to_string(), b.normalize().to_string());
    }
}

/// The spike result vector: both fractional-path CAGRs + the integer-path projection, at FULL
/// precision (unrounded — the digest pins the raw math; display rounding is checked separately).
fn spike_result_vector() -> Vec<Decimal> {
    let cagr_5 = cagr(Decimal::ONE, pow_int_exact(Decimal::new(11, 1), 5), 5);
    let cagr_9 = cagr(Decimal::ONE, pow_int_exact(Decimal::new(108, 2), 9), 9);
    let projection = Decimal::new(1000, 0) * Decimal::new(115, 2).powd(Decimal::from(5u32));
    vec![cagr_5, cagr_9, projection]
}

/// SHA-256 over the result vector, serialized exactly like `core::determinism_hash`: each value's
/// `normalize().to_string()` + `\n` — value-only, representation-independent. The pinned constant
/// is the cross-OS contract; if this fails anywhere, exact-decimal CAGR determinism broke and the
/// build must not pass.
#[test]
fn spike_determinism_hash_matches_pinned_cross_os_contract() {
    const EXPECTED: &str = "d9af555376f754c8543bb4a6c94267257c227612df4a18e7ee6cba02b57e5557";

    let mut hasher = Sha256::new();
    for d in spike_result_vector() {
        hasher.update(d.normalize().to_string().as_bytes());
        hasher.update(b"\n");
    }
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    assert_eq!(
        digest, EXPECTED,
        "spike-c determinism hash changed — the fractional powd series math drifted \
         (rust_decimal upgrade? toolchain change?). Investigate before re-pinning."
    );
}
