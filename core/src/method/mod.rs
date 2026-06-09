//! Machine-readable mirror of `docs/method/ssg-method-spec-v1.md`.
//!
//! Every numeric threshold / list here corresponds to a line in the spec. **Changing any constant
//! in this module (or in [`crate::quality_flags`], [`crate::rounding`], or
//! [`crate::method_version`]) requires bumping [`crate::method_version::METHOD_VERSION`]** — the
//! `method_fingerprint` change-detection test enforces it. Numerics are exact `Decimal`, never float.

use crate::method_version::METHOD_VERSION;
use crate::quality_flags::{PLAUSIBILITY_RULES, QUALITY_FLAGS};
use crate::rounding::DisplayField;
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

// ── Golden tolerance (FR9 / NFR-C2) ──

/// Relative tolerance on derived numerics (0.005 = ±0.5%). Zoning + categorical verdict match EXACTLY.
pub fn golden_relative_tolerance() -> Decimal {
    Decimal::new(5, 3)
}

// ── Plausibility bounds (spec §3) ──

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

/// Banned imperative action/recommendation verbs in system signals (English, lowercase whole-word).
pub const BANNED_VERBS_EN: [&str; 15] = [
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

/// All display field groups, in a fixed order (for the fingerprint).
const DISPLAY_FIELDS: [DisplayField; 6] = [
    DisplayField::Price,
    DisplayField::PerShare,
    DisplayField::PeRatio,
    DisplayField::Percent,
    DisplayField::Ratio,
    DisplayField::LargeMonetary,
];

/// Canonical SHA-256 over the entire method definition (version + every constant, flag, rule, verb,
/// and display scale). Changing any of those changes this fingerprint, so the change-detection test
/// fails until `METHOD_VERSION` is bumped and the snapshot regenerated. This realizes "you cannot
/// change the method silently".
pub fn method_fingerprint() -> String {
    let mut p: Vec<String> = Vec::new();
    p.push(format!("method_version={METHOD_VERSION}"));
    p.push(format!("usable_years_floor={USABLE_YEARS_FLOOR}"));
    p.push(format!("forecast_horizon_years={FORECAST_HORIZON_YEARS}"));
    p.push(format!("zone_count={ZONE_COUNT}"));
    p.push(format!("load_bearing_year={LOAD_BEARING_YEAR_FIELDS:?}"));
    p.push(format!(
        "load_bearing_judgment={LOAD_BEARING_JUDGMENT_INPUTS:?}"
    ));
    p.push(format!("ud_target={}", ud_target()));
    p.push(format!("ud_extreme={}", ud_extreme()));
    p.push(format!(
        "relative_value_ceiling_pct={}",
        relative_value_ceiling_pct()
    ));
    p.push(format!("high_pe_aggressive={}", high_pe_aggressive()));
    p.push(format!("high_pe_implausible={}", high_pe_implausible()));
    p.push(format!("roe_low_pct={}", roe_low_pct()));
    p.push(format!("trend_even_band_pp={}", trend_even_band_pp()));
    p.push(format!(
        "golden_relative_tolerance={}",
        golden_relative_tolerance()
    ));
    p.push(format!("split_jump_high={}", split_jump_high()));
    p.push(format!("split_jump_low={}", split_jump_low()));
    p.push(format!("pe_axis_max={}", pe_axis_max()));
    for f in QUALITY_FLAGS {
        p.push(format!("flag:{}={:?}", f.key, f.severity));
    }
    p.push(format!("plausibility={PLAUSIBILITY_RULES:?}"));
    p.push(format!("banned_en={BANNED_VERBS_EN:?}"));
    p.push(format!("banned_fr={BANNED_VERBS_FR:?}"));
    for field in DISPLAY_FIELDS {
        p.push(format!("scale:{field:?}={}", field.scale()));
    }

    let mut hasher = Sha256::new();
    hasher.update(p.join("\n").as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Change-detection gate: editing ANY method constant changes this fingerprint. When that is an
    /// intentional method change, bump `METHOD_VERSION` and regenerate `EXPECTED` below (the failing
    /// assertion prints the new value).
    #[test]
    fn method_fingerprint_is_pinned_to_version() {
        const EXPECTED: &str = "78bfa4f044933320f2ad5df56aa91c4dfbbd3c7d614df225acaad7e35d12bb54";
        assert_eq!(
            method_fingerprint(),
            EXPECTED,
            "method definition changed — bump METHOD_VERSION and regenerate this snapshot"
        );
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
