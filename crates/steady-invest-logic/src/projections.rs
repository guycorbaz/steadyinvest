use crate::types::{TrendAnalysis, TrendPoint};

/// Projects a value forward by `years` at the given CAGR (percentage).
///
/// Formula: `base * (1 + cagr_pct / 100)^years`
///
/// This is the single source of truth for forward-projection calculations
/// used in the Fundamental Company Data table's "5 Yr Est" column and
/// the `extract_snapshot_prices` target price computation.
///
/// # Arguments
///
/// * `base` — The starting value (e.g., last fiscal year's sales or EPS).
/// * `cagr_pct` — Compound Annual Growth Rate expressed as a percentage (e.g., 10.0 for 10%).
/// * `years` — Number of years to project forward.
///
/// # Examples
///
/// ```
/// use steady_invest_logic::project_forward;
///
/// // 100 at 10% for 5 years = 161.051
/// assert!((project_forward(100.0, 10.0, 5) - 161.051).abs() < 0.01);
/// // Zero base stays zero
/// assert!((project_forward(0.0, 10.0, 5)).abs() < 1e-10);
/// ```
pub fn project_forward(base: f64, cagr_pct: f64, years: u32) -> f64 {
    let growth_factor = 1.0 + cagr_pct / 100.0;
    if growth_factor < 0.0 {
        // CAGR below -100% produces a negative base which is undefined for powf
        return 0.0;
    }
    base * growth_factor.powi(years as i32)
}

/// Generates a projected trendline based on a starting point and a target CAGR.
///
/// Uses the formula: `value = start_value * (1 + cagr/100)^(year - start_year)`.
/// This is used by the Valuation Panel to compute future buy/sell zone prices.
///
/// # Arguments
///
/// * `start_year`  — The base year from which projection begins.
/// * `start_value` — The value at `start_year` (must be positive).
/// * `cagr`        — Target Compound Annual Growth Rate (as a percentage).
/// * `years`       — The future years to project values for.
///
/// # Returns
///
/// A [`TrendAnalysis`] with the given `cagr` and one [`TrendPoint`] per requested year.
/// Returns [`TrendAnalysis::default()`] if `years` is empty or `start_value` is non-positive.
///
/// # Examples
///
/// ```
/// use steady_invest_logic::calculate_projected_trendline;
///
/// let result = calculate_projected_trendline(2023, 100.0, 10.0, &[2024, 2025]);
/// assert!((result.trendline[0].value - 110.0).abs() < 0.01);
/// assert!((result.trendline[1].value - 121.0).abs() < 0.01);
/// ```
pub fn calculate_projected_trendline(
    start_year: i32,
    start_value: f64,
    cagr: f64,
    years: &[i32],
) -> TrendAnalysis {
    if years.is_empty() || start_value <= 0.0 {
        return TrendAnalysis::default();
    }

    let trendline = years
        .iter()
        .map(|&year| {
            let n = (year - start_year) as f64;
            let value = start_value * (1.0 + cagr / 100.0).powf(n);
            TrendPoint { year, value }
        })
        .collect();

    TrendAnalysis { cagr, trendline }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projected_trendline() {
        let years = vec![2024, 2025, 2026];
        let start_year = 2023;
        let start_value = 100.0;
        let cagr = 10.0;

        let analysis = calculate_projected_trendline(start_year, start_value, cagr, &years);

        assert_eq!(analysis.cagr, 10.0);
        assert_eq!(analysis.trendline.len(), 3);
        // 2024: 100 * 1.1 = 110
        assert!((analysis.trendline[0].value - 110.0).abs() < 0.001);
        // 2025: 110 * 1.1 = 121
        assert!((analysis.trendline[1].value - 121.0).abs() < 0.001);
        // 2026: 121 * 1.1 = 133.1
        assert!((analysis.trendline[2].value - 133.1).abs() < 0.001);
    }

    #[test]
    fn test_project_forward() {
        // 100 at 10% for 5 years = 161.051
        assert!((project_forward(100.0, 10.0, 5) - 161.051).abs() < 0.01);
        // Zero base stays zero
        assert!((project_forward(0.0, 10.0, 5)).abs() < 1e-10);
        // 0% growth returns base unchanged
        assert!((project_forward(50.0, 0.0, 5) - 50.0).abs() < 1e-10);
        // Negative CAGR decreases value
        assert!(project_forward(100.0, -10.0, 5) < 100.0);
        // Extreme negative CAGR (< -100%) returns 0.0 instead of NaN
        assert!((project_forward(100.0, -150.0, 3)).abs() < 1e-10);
    }

    /// Verify NAIC EPS projection formula over 5 years.
    /// Formula: Projected EPS = Current EPS × (1 + growth_rate)^5
    /// Reference: Handbook Section 4A (p15) — EPS 5.71 at 10.4% → ~9.37
    #[test]
    fn test_naic_handbook_eps_projection() {
        let start_year = 2015;
        let current_eps = 5.71;
        let cagr = 10.4;
        let future_years: Vec<i32> = (2016..=2020).collect();

        let projection =
            calculate_projected_trendline(start_year, current_eps, cagr, &future_years);

        assert_eq!(projection.trendline.len(), 5);

        // Year 1 (2016): 5.71 × 1.104 = 6.304
        assert!(
            (projection.trendline[0].value - 6.304).abs() < 0.01,
            "Year 1: expected ~6.30, got {:.2}",
            projection.trendline[0].value
        );

        // Year 5 (2020): should match handbook's Estimated High EPS ≈ 9.37
        let year5_eps = projection.trendline[4].value;
        assert!(
            (year5_eps - 9.37).abs() < 0.1,
            "Year 5 EPS: expected ~9.37, got {:.2}",
            year5_eps
        );
    }
}
