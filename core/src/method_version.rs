//! The calculation-semantics version — one of the three version axes (`schema_version` /
//! SQLite `user_version` / **`method_version`**). Bump on ANY change to the method constants or
//! formulas, or to the definition of a historical input (spec §0 — which period a figure covers,
//! which EPS; see `docs/method/ssg-method-spec-v1.md` "Change control"). By the Foundational
//! Invariant, a `method_version` change re-addresses (invalidates) every derived verdict.

/// Semver-like identifier of the SSG method this build implements. `ssg-1.2.0`: the historical
/// inputs are the company's fiscal years, their high/low the fiscal year's, their EPS the reported
/// diluted one (spec §0) — engine formulas unchanged.
pub const METHOD_VERSION: &str = "ssg-1.2.0";
