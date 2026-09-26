//! The calculation-semantics version — one of the three version axes (`schema_version` /
//! SQLite `user_version` / **`method_version`**). Bump on ANY change to the method constants or
//! formulas, or to the definition of a historical input (spec §0 — which period a figure covers,
//! which EPS; see `docs/method/ssg-method-spec-v1.md` "Change control"). By the Foundational
//! Invariant, a `method_version` change re-addresses (invalidates) every derived verdict.

/// Semver-like identifier of the SSG method this build implements. `ssg-1.2.0`: the historical
/// inputs are the company's fiscal years, their high/low the fiscal year's, their EPS the reported
/// diluted one (spec §0) — engine formulas unchanged.
pub const METHOD_VERSION: &str = "ssg-1.2.0";

/// The method version that last changed what a historical input IS (spec §0 — the period a figure
/// covers, which EPS). A provider figure fetched under an EARLIER version may differ from what a
/// fetch gives today although the provider's data did not change (issue #252). Moves only with a
/// §0 change, never with a formula-only bump.
pub const INPUTS_DEFINED_AT: &str = "ssg-1.2.0";

/// Whether a figure fetched under method `stamp` predates [`INPUTS_DEFINED_AT`] — `None` (a figure
/// fetched before stamps existed, i.e. before `ssg-1.2.0`) and an unreadable stamp both do: they
/// cannot be shown to follow today's definition. PURE.
pub fn predates_inputs_definition(stamp: Option<&str>) -> bool {
    match (stamp.and_then(parse), parse(INPUTS_DEFINED_AT)) {
        (Some(fetched), Some(defined)) => fetched < defined,
        _ => true,
    }
}

/// `"ssg-X.Y.Z"` → `(X, Y, Z)`; anything else → `None`.
fn parse(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.strip_prefix("ssg-")?.split('.');
    let triple = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(triple)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stamp_predates_the_inputs_definition_only_when_older_or_missing() {
        assert!(
            predates_inputs_definition(None),
            "no stamp: fetched before 1.2.0"
        );
        assert!(predates_inputs_definition(Some("ssg-1.1.0")));
        assert!(predates_inputs_definition(Some("garbage")));
        assert!(!predates_inputs_definition(Some("ssg-1.2.0")));
        assert!(
            !predates_inputs_definition(Some("ssg-1.10.0")),
            "numeric, not lexicographic"
        );
        assert!(!predates_inputs_definition(Some(METHOD_VERSION)));
    }
}
