//! The calculation-semantics version — one of the three version axes (`schema_version` /
//! SQLite `user_version` / **`method_version`**). Bump on ANY change to the method constants or
//! formulas (see `docs/method/ssg-method-spec-v1.md` "Change control"). By the Foundational
//! Invariant, a `method_version` change re-addresses (invalidates) every derived verdict.

/// Semver-like identifier of the SSG method this build implements.
pub const METHOD_VERSION: &str = "ssg-1.1.0";
