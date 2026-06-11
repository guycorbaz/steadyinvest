//! steadyinvest-core — pure SSG calculation engine.
//!
//! **Cardinal Rule:** this crate performs ALL calculation and has **no** I/O, UI, SQL, or network
//! dependencies. Money and ratios use [`rust_decimal::Decimal`] exact decimals — never `f32`/`f64`
//! in the decision chain — which makes results bit-identical across platforms.
//!
//! The versioned **method specification** (the authoritative oracle) is mirrored here as typed
//! constants: see [`method`], [`quality_flags`], [`rounding`] and [`method_version`], backed by
//! `docs/method/ssg-method-spec-v1.md`. The raw → canonical [`normalize`] layer (Story 1.7) lives
//! here too; the five-section SSG engine, plausibility checks, verdict and risk math that *consume*
//! it arrive in later Epic 1 stories (1.8–1.11). This file also provides the cross-platform
//! **determinism probe** that the CI determinism-hash gate relies on.

pub mod method;
pub mod method_version;
pub mod normalize;
pub mod quality_flags;
pub mod rounding;

pub use method_version::METHOD_VERSION;
pub use normalize::{normalize, CanonicalFinancials, RawFinancials};

use rust_decimal::{Decimal, MathematicalOps};
use sha2::{Digest, Sha256};

/// A tiny, fixed exact-decimal computation used to prove cross-platform numeric determinism.
///
/// It builds a compound-growth vector `1000 * (1 + 0.15)^n` using `rust_decimal`'s `maths` feature
/// (`powd`), rounded to 6 decimal places. The exponents deliberately include both **integer** powers
/// (`n = 0,1,2` — the exact integer-power path) **and a fractional power** (`n = 1.5`), so the
/// digest also pins `powd`'s transcendental `exp(y·ln x)` series — the path real fractional-CAGR math
/// will use and the true cross-OS / cross-version risk. This is **scaffolding**, not the SSG method.
pub fn determinism_probe() -> Vec<Decimal> {
    let base = Decimal::new(1000, 0); // 1000
    let rate = Decimal::new(15, 2); // 0.15
    let one = Decimal::ONE;
    // Integer exponents (exact path) + one fractional exponent (series path).
    let exponents = [
        Decimal::ZERO,
        Decimal::ONE,
        Decimal::TWO,
        Decimal::new(15, 1), // 1.5 — exercises the transcendental series
    ];
    exponents
        .iter()
        .map(|&exp| {
            let factor = (one + rate).powd(exp);
            (base * factor).round_dp(6)
        })
        .collect()
}

/// Canonical, platform-independent SHA-256 over [`determinism_probe`].
///
/// Each decimal is serialized to its normalized decimal **string** before hashing, so the digest
/// depends only on the numeric value — not on any in-memory representation. The CI matrix asserts
/// this digest is identical on Windows, macOS and Linux.
pub fn determinism_hash() -> String {
    let mut hasher = Sha256::new();
    for d in determinism_probe() {
        hasher.update(d.normalize().to_string().as_bytes());
        hasher.update(b"\n");
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determinism_probe_has_expected_shape() {
        let v = determinism_probe();
        assert_eq!(v.len(), 4);
        // exp = 0 → 1000 * 1.15^0 = 1000
        assert_eq!(v[0], Decimal::new(1000, 0));
    }

    #[test]
    fn determinism_hash_matches_cross_os_contract() {
        // If this fails on ANY OS, exact-decimal determinism broke — the build must not pass.
        const EXPECTED: &str = "eb45e761e031b0fa03c943e97a15aa41186f75c0088e33a69c426c2278fbd34f";
        assert_eq!(determinism_hash(), EXPECTED);
    }
}
