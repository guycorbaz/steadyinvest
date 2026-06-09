//! steadyinvest-core — pure SSG calculation engine.
//!
//! **Cardinal Rule:** this crate performs ALL calculation and has **no** I/O, UI, SQL, or network
//! dependencies. Money and ratios use [`rust_decimal::Decimal`] exact decimals — never `f32`/`f64`
//! in the decision chain — which makes results bit-identical across platforms.
//!
//! The real five-section SSG engine, quality flags, plausibility checks, verdict and risk math
//! arrive in later Epic 1 stories (1.7–1.11). This file currently provides only the
//! cross-platform **determinism probe** that the CI determinism-hash gate relies on.

use rust_decimal::{Decimal, MathematicalOps};
use sha2::{Digest, Sha256};

/// A tiny, fixed exact-decimal computation used to prove cross-platform numeric determinism.
///
/// It builds a 3-element compound-growth vector `1000 * (1 + 0.15)^n` for `n in 0..3` using
/// `rust_decimal`'s `maths` feature (`powd`), rounded to 6 decimal places. This is **scaffolding**,
/// not part of the SSG method.
pub fn determinism_probe() -> Vec<Decimal> {
    let base = Decimal::new(1000, 0); // 1000
    let rate = Decimal::new(15, 2); // 0.15
    let one = Decimal::ONE;
    (0u32..3)
        .map(|n| {
            let factor = (one + rate).powd(Decimal::from(n));
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
        assert_eq!(v.len(), 3);
        // n = 0 → 1000 * 1.15^0 = 1000
        assert_eq!(v[0], Decimal::new(1000, 0));
    }

    #[test]
    fn determinism_hash_matches_cross_os_contract() {
        // If this fails on ANY OS, exact-decimal determinism broke — the build must not pass.
        const EXPECTED: &str = "6ccd4cac2820867018eeabf755fb7371b8dcbf14b88201a276591b4772247fd3";
        assert_eq!(determinism_hash(), EXPECTED);
    }
}
