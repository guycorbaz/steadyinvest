//! Methodology quality flags (FR7) and input plausibility rules (FR10) — the pinned **catalog**.
//!
//! This module defines the keys + severities that mirror `docs/method/ssg-method-spec-v1.md` §2–§3.
//! The engine (Story 1.8) *raises* these; here we only fix the catalog so the spec and code cannot
//! drift. Numeric thresholds live in [`crate::method`].

/// Severity of a methodology quality flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
}

impl Severity {
    /// Stable identifier for the method fingerprint (NOT derived from `Debug`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warn => "warn",
        }
    }
}

/// A methodology quality flag (FR7): a signal about the *business*, distinct from a data
/// plausibility warning and from the user's review tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualityFlag {
    pub key: &'static str,
    pub severity: Severity,
}

/// The v1 quality-flag catalog (spec §2 + §1 verdict thresholds).
pub const QUALITY_FLAGS: [QualityFlag; 10] = [
    QualityFlag {
        key: "ptp_trend_declining",
        severity: Severity::Warn,
    },
    QualityFlag {
        key: "roe_trend_declining",
        severity: Severity::Warn,
    },
    QualityFlag {
        key: "roe_low",
        severity: Severity::Info,
    },
    QualityFlag {
        key: "eps_lags_sales",
        severity: Severity::Info,
    },
    QualityFlag {
        key: "high_debt",
        severity: Severity::Info,
    },
    QualityFlag {
        key: "projected_high_pe_aggressive",
        severity: Severity::Warn,
    },
    QualityFlag {
        key: "projected_high_pe_implausible",
        severity: Severity::Warn,
    },
    QualityFlag {
        key: "ud_below_target",
        severity: Severity::Info,
    },
    QualityFlag {
        key: "ud_extreme",
        severity: Severity::Warn,
    },
    QualityFlag {
        key: "relative_value_high",
        severity: Severity::Info,
    },
];

/// The v1 plausibility-rule catalog (spec §3) — input-data warnings, never blocking.
pub const PLAUSIBILITY_RULES: [&str; 6] = [
    "split_series_break",
    "currency_mismatch",
    "fiscal_period_misalignment",
    "out_of_bounds_ratio",
    "negative_or_zero_denominator",
    "low_price_above_current",
];
