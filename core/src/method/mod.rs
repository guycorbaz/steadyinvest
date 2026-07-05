//! Machine-readable mirror of `docs/method/ssg-method-spec-v1.md`.
//!
//! Every numeric threshold / list here corresponds to a line in the spec. **Changing any constant
//! in this module (or in [`crate::quality_flags`], [`crate::rounding`], or
//! [`crate::method_version`]) requires bumping [`crate::method_version::METHOD_VERSION`]** — the
//! `method_fingerprint` change-detection test enforces it. Numerics are exact `Decimal`, never float.

use crate::method_version::METHOD_VERSION;
use crate::quality_flags::{PLAUSIBILITY_RULES, QUALITY_FLAGS};
use crate::rounding::{DisplayField, strategy_behavior_probe};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};

// ── Confidence / structure (spec §4, §5/§6) ──

/// A study needs at least this many usable years to be full-confidence (FR8).
pub const USABLE_YEARS_FLOOR: u32 = 5;

/// Default forecast horizon, in years (spec §1/§4/§5).
pub const FORECAST_HORIZON_YEARS: u32 = 5;

/// Number of price zones (Buy / Neutral / Sell): equal thirds of the forecast range (spec §4).
pub const ZONE_COUNT: u32 = 3;

/// Per-year load-bearing fields — a year is "usable" iff all are present (FR8/FR12, spec §4/§5).
pub const LOAD_BEARING_YEAR_FIELDS: [&str; 4] = ["sales", "eps", "high_price", "low_price"];

/// Judgment inputs that gate the verdict (FR12, spec §5 "load-bearing input").
pub const LOAD_BEARING_JUDGMENT_INPUTS: [&str; 5] = [
    "estimated_high_eps",
    "estimated_low_eps",
    "judged_avg_high_pe",
    "judged_avg_low_pe",
    "current_price",
];

// ── Verdict / quality thresholds (spec §1 verdict, §2 flags). Decimal::new(mantissa, scale). ──

/// Recommended minimum upside/downside ratio (3.0).
pub fn ud_target() -> Decimal {
    Decimal::new(30, 1)
}
/// Upside/downside ratio above which the high/low choices should be reconsidered (15.0).
pub fn ud_extreme() -> Decimal {
    Decimal::new(150, 1)
}
/// Relative-value ceiling, percent (100.0): at/above this the current P/E is not below its average.
pub fn relative_value_ceiling_pct() -> Decimal {
    Decimal::new(1000, 1)
}
/// Judged future high P/E above which the projection is "aggressive" (20).
pub fn high_pe_aggressive() -> Decimal {
    Decimal::new(20, 0)
}
/// Judged future high P/E above which to re-evaluate the choice (25).
pub fn high_pe_implausible() -> Decimal {
    Decimal::new(25, 0)
}
/// Latest ROE (%) below which a low-ROE info flag is raised (10).
pub fn roe_low_pct() -> Decimal {
    Decimal::new(10, 0)
}
/// Trend "even" band (percentage points): |recent − 5yr avg| ≤ this ⇒ trend is even (0.5).
pub fn trend_even_band_pp() -> Decimal {
    Decimal::new(5, 1)
}

/// Verdict appreciation target (%): present price → forecast high should imply roughly doubling over
/// the forecast horizon (≈ 15%/yr over 5 years). Spec §1 verdict.
pub fn verdict_double_appreciation_pct() -> Decimal {
    Decimal::new(1000, 1) // 100.0% total appreciation over FORECAST_HORIZON_YEARS
}

// ── Golden tolerance (FR9 / NFR-C2) ──

/// Method default relative tolerance on derived numerics (0.005 = ±0.5%). Zoning + categorical
/// verdict match EXACTLY. (Fixed method constant — tests may compare with a tighter local epsilon,
/// but the method's canonical tolerance is this value.)
pub fn golden_relative_tolerance() -> Decimal {
    Decimal::new(5, 3)
}

// ── Plausibility bounds (spec §3) ──

/// Magnitude bound (percentage points) for PTP and ROE plausibility: a value outside
/// `[-this, +this]` is out-of-bounds (100 ⇒ [-100%, +100%]). Spec §3 `out_of_bounds_ratio`.
pub fn ptp_roe_bound_pct() -> Decimal {
    Decimal::new(100, 0)
}
/// Lower bound of the P/E plausibility range; P/E < this is out-of-bounds (0). Spec §3.
pub fn pe_axis_min() -> Decimal {
    Decimal::ZERO
}

/// Year-over-year jump factor at/above which a split/series break is suspected (1.5).
pub fn split_jump_high() -> Decimal {
    Decimal::new(15, 1)
}
/// Year-over-year jump factor at/below which a split/series break is suspected (0.67).
pub fn split_jump_low() -> Decimal {
    Decimal::new(67, 2)
}
/// Upper bound of the P/E chart axis; P/E outside [0, this] is out-of-bounds (200).
pub fn pe_axis_max() -> Decimal {
    Decimal::new(200, 0)
}

// ── Neutral-posture banned verbs (FR13, spec §6) — scope: system-generated signals only ──

/// Banned imperative action/recommendation verbs in **system-generated** signals (English,
/// case-insensitive). Entries may be multi-word phrases (e.g. "ought to"); the future posture
/// checker (Story 2.14) matches them case-insensitively against system strings only — never user
/// free-text — and zone-label nouns ("Buy"/"Neutral"/"Sell" zone) are exempt (they name the defined
/// price bands, they are not imperatives).
pub const BANNED_VERBS_EN: [&str; 16] = [
    "buy",
    "sell",
    "hold",
    "purchase",
    "acquire",
    "dump",
    "exit",
    "enter",
    "trade",
    "invest",
    "divest",
    "recommend",
    "suggest",
    "should",
    "must",
    "ought to",
];

/// Banned imperative verbs in the French-first UI (lowercase whole-word).
pub const BANNED_VERBS_FR: [&str; 10] = [
    "acheter",
    "vendre",
    "conserver",
    "garder",
    "acquérir",
    "investir",
    "recommander",
    "suggérer",
    "devrait",
    "il faut",
];

/// Case-insensitive **whole-word** match of `needle` in `haystack` — the matcher of the FR13
/// posture gates (banned-verb checks over emitted strings, here and in the downstream crates'
/// neutrality tests). A word boundary is any non-alphanumeric character (or the string edge), so
/// `_` IS a boundary: `"buy_top"` contains the word `"buy"` — which is why the spec-§6-exempt
/// zone-derived nouns must be excluded from an inventory before matching, never mixed in.
/// Multi-word needles (e.g. `"ought to"`) match verbatim with the same boundary rule.
pub fn contains_word(haystack: &str, needle: &str) -> bool {
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();
    h.match_indices(&n).any(|(i, _)| {
        let before_ok = i == 0
            || !h[..i]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric());
        let after = i + n.len();
        let after_ok = after == h.len()
            || !h[after..]
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric());
        before_ok && after_ok
    })
}

/// All display field groups, in a fixed order (for the fingerprint).
const DISPLAY_FIELDS: [DisplayField; 6] = [
    DisplayField::Price,
    DisplayField::PerShare,
    DisplayField::PeRatio,
    DisplayField::Percent,
    DisplayField::Ratio,
    DisplayField::LargeMonetary,
];

/// Serialize a `Decimal` canonically by **value** (not representation): `normalize()` collapses
/// trailing-zero scale so `3.0` and `3` hash identically.
fn d(value: Decimal) -> String {
    value.normalize().to_string()
}

/// Canonical SHA-256 over the **entire** method definition: version, every numeric constant, the
/// load-bearing field lists, the quality-flag catalog, the plausibility catalog, both banned-verb
/// lists, every display-field scale, AND the rounding strategy's behavior. Serialization is explicit
/// and value-based (no `Debug`, which is not a stability contract; decimals normalized) so the hash
/// is stable across toolchains. Changing any of these changes the fingerprint, so the
/// change-detection test fails until `METHOD_VERSION` is bumped and the snapshot regenerated —
/// realizing "you cannot change the method silently".
pub fn method_fingerprint() -> String {
    let mut p: Vec<String> = Vec::new();
    p.push(format!("method_version={METHOD_VERSION}"));
    p.push(format!("usable_years_floor={USABLE_YEARS_FLOOR}"));
    p.push(format!("forecast_horizon_years={FORECAST_HORIZON_YEARS}"));
    p.push(format!("zone_count={ZONE_COUNT}"));
    p.push(format!(
        "load_bearing_year={}",
        LOAD_BEARING_YEAR_FIELDS.join("|")
    ));
    p.push(format!(
        "load_bearing_judgment={}",
        LOAD_BEARING_JUDGMENT_INPUTS.join("|")
    ));
    p.push(format!("ud_target={}", d(ud_target())));
    p.push(format!("ud_extreme={}", d(ud_extreme())));
    p.push(format!(
        "relative_value_ceiling_pct={}",
        d(relative_value_ceiling_pct())
    ));
    p.push(format!("high_pe_aggressive={}", d(high_pe_aggressive())));
    p.push(format!("high_pe_implausible={}", d(high_pe_implausible())));
    p.push(format!("roe_low_pct={}", d(roe_low_pct())));
    p.push(format!("trend_even_band_pp={}", d(trend_even_band_pp())));
    p.push(format!(
        "verdict_double_appreciation_pct={}",
        d(verdict_double_appreciation_pct())
    ));
    p.push(format!(
        "golden_relative_tolerance={}",
        d(golden_relative_tolerance())
    ));
    p.push(format!("ptp_roe_bound_pct={}", d(ptp_roe_bound_pct())));
    p.push(format!("pe_axis_min={}", d(pe_axis_min())));
    p.push(format!("pe_axis_max={}", d(pe_axis_max())));
    p.push(format!("split_jump_high={}", d(split_jump_high())));
    p.push(format!("split_jump_low={}", d(split_jump_low())));
    for f in QUALITY_FLAGS {
        p.push(format!("flag:{}={}", f.key, f.severity.as_str()));
    }
    p.push(format!("plausibility={}", PLAUSIBILITY_RULES.join("|")));
    p.push(format!("banned_en={}", BANNED_VERBS_EN.join("|")));
    p.push(format!("banned_fr={}", BANNED_VERBS_FR.join("|")));
    for field in DISPLAY_FIELDS {
        p.push(format!("scale:{}={}", field.as_str(), field.scale()));
    }
    // Behavioral fingerprint of the rounding strategy (closes the silent rounding-mode-change hole).
    p.push(format!("rounding_behavior={}", strategy_behavior_probe()));

    let mut hasher = Sha256::new();
    hasher.update(p.join("\n").as_bytes());
    crate::hex_sha256(hasher)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Change-detection gate: editing ANY method constant changes this fingerprint. When that is an
    /// intentional method change, bump `METHOD_VERSION` and regenerate `EXPECTED` below (the failing
    /// assertion prints the new value).
    #[test]
    fn method_fingerprint_is_pinned_to_version() {
        const EXPECTED: &str = "f79e3c11227094ac8543376224cf2421d7f4d95082507cc6bf34d9395cd61d1d";
        assert_eq!(
            method_fingerprint(),
            EXPECTED,
            "method definition changed — bump METHOD_VERSION and regenerate this snapshot"
        );
    }

    /// Fingerprint-exhaustiveness tie: the `match` below is exhaustive with NO wildcard, so
    /// adding a [`DisplayField`] variant is a compile error here until `DISPLAY_FIELDS` (and
    /// hence the fingerprint input) is revisited. Same intent as the exhaustive destructuring
    /// in `verdict::inputs_digest`.
    #[test]
    fn display_fields_list_covers_every_variant() {
        // Exhaustive, wildcard-free match — the compile-time tie.
        for field in DISPLAY_FIELDS {
            match field {
                DisplayField::Price
                | DisplayField::PerShare
                | DisplayField::PeRatio
                | DisplayField::Percent
                | DisplayField::Ratio
                | DisplayField::LargeMonetary => {}
            }
        }
        // 6 entries, all distinct ⇒ all 6 variants are present exactly once.
        for (i, a) in DISPLAY_FIELDS.iter().enumerate() {
            assert!(
                !DISPLAY_FIELDS[i + 1..].contains(a),
                "duplicate DISPLAY_FIELDS entry: {a:?}"
            );
        }
        assert_eq!(
            DISPLAY_FIELDS.len(),
            6,
            "one entry per DisplayField variant"
        );
    }

    #[test]
    fn load_bearing_lists_are_coherent() {
        // Non-empty, no duplicates, and the two sets are disjoint. (The vs-year-struct subset
        // check lives in `crate::normalize::types` tests — Story 1.7 closed the 1.2 deferral.)
        assert!(!LOAD_BEARING_YEAR_FIELDS.is_empty());
        assert!(!LOAD_BEARING_JUDGMENT_INPUTS.is_empty());
        for (i, a) in LOAD_BEARING_YEAR_FIELDS.iter().enumerate() {
            assert!(
                !LOAD_BEARING_YEAR_FIELDS[i + 1..].contains(a),
                "duplicate load-bearing year field: {a}"
            );
            assert!(
                !LOAD_BEARING_JUDGMENT_INPUTS.contains(a),
                "field appears in both load-bearing sets: {a}"
            );
        }
        for (i, b) in LOAD_BEARING_JUDGMENT_INPUTS.iter().enumerate() {
            assert!(
                !LOAD_BEARING_JUDGMENT_INPUTS[i + 1..].contains(b),
                "duplicate load-bearing judgment input: {b}"
            );
        }
    }

    #[test]
    fn method_version_is_declared() {
        assert!(!METHOD_VERSION.is_empty());
        assert!(METHOD_VERSION.starts_with("ssg-"));
    }

    #[test]
    fn usable_years_floor_matches_spec() {
        assert_eq!(USABLE_YEARS_FLOOR, 5);
    }

    #[test]
    fn zone_count_is_thirds() {
        assert_eq!(ZONE_COUNT, 3);
    }

    #[test]
    fn golden_tolerance_is_positive_half_percent() {
        assert_eq!(golden_relative_tolerance(), Decimal::new(5, 3));
        assert!(golden_relative_tolerance() > Decimal::ZERO);
    }
}
