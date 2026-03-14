use rust_decimal::prelude::ToPrimitive;

use crate::projections::project_forward;
use crate::types::*;

/// Computes historical High/Low P/E ratios and their averages.
///
/// Strictly limited to the **last 5 completed years** of data. Years with
/// zero or negative EPS are skipped (they produce meaningless P/E ratios).
///
/// # Arguments
///
/// * `data` — Historical financial data; only the `records` field is read.
///
/// # Returns
///
/// A [`PeRangeAnalysis`] with up to 5 per-year P/E points and their averages
/// (per NAIC SSG Section 3: 5-year P/E history).
///
/// # Examples
///
/// ```
/// use steady_invest_logic::{HistoricalData, HistoricalYearlyData, calculate_pe_ranges};
/// use rust_decimal::Decimal;
///
/// let data = HistoricalData {
///     records: vec![HistoricalYearlyData {
///         fiscal_year: 2023,
///         eps: Decimal::from(10),
///         price_high: Decimal::from(200),
///         price_low: Decimal::from(100),
///         ..Default::default()
///     }],
///     ..Default::default()
/// };
/// let result = calculate_pe_ranges(&data);
/// assert_eq!(result.points.len(), 1);
/// assert!((result.avg_high_pe - 20.0).abs() < 0.01);
/// assert!((result.avg_low_pe - 10.0).abs() < 0.01);
/// ```
pub fn calculate_pe_ranges(data: &HistoricalData) -> PeRangeAnalysis {
    let mut eligible_records = data.records.clone();

    // Sort chronologically to identify the "last" years reliably
    eligible_records.sort_by_key(|r| r.fiscal_year);

    // Take the last 5 records per NAIC SSG Section 3 (5-year P/E history)
    let len = eligible_records.len();
    let start_idx = len.saturating_sub(5);
    let recent_records = &eligible_records[start_idx..];

    let mut points = Vec::new();
    let mut high_pes = Vec::new();
    let mut low_pes = Vec::new();

    for record in recent_records {
        if !record.eps.is_zero() && record.eps.is_sign_positive() {
            let high_pe = (record.price_high / record.eps).to_f64().unwrap_or(0.0);
            let low_pe = (record.price_low / record.eps).to_f64().unwrap_or(0.0);

            if high_pe > 0.0 && low_pe > 0.0 {
                points.push(PeRangePoint {
                    year: record.fiscal_year,
                    high_pe,
                    low_pe,
                });
                high_pes.push(high_pe);
                low_pes.push(low_pe);
            }
        }
    }

    let avg_high_pe = if !high_pes.is_empty() {
        high_pes.iter().sum::<f64>() / high_pes.len() as f64
    } else {
        0.0
    };

    let avg_low_pe = if !low_pes.is_empty() {
        low_pes.iter().sum::<f64>() / low_pes.len() as f64
    } else {
        0.0
    };

    PeRangeAnalysis {
        points,
        avg_high_pe,
        avg_low_pe,
    }
}

/// Computes ROE and Profit-on-Sales ratios with year-over-year trend indicators.
///
/// Records are processed oldest-to-newest so that each year's trend is
/// relative to the immediately preceding year. A ±0.1 pp dead-band prevents
/// minor fluctuations from toggling the indicator.
///
/// # Arguments
///
/// * `data` — Historical data; uses `net_income`, `total_equity`,
///   `pretax_income`, and `sales` from each record.
///
/// # Returns
///
/// A [`QualityAnalysis`] with one [`QualityPoint`] per record, sorted
/// chronologically.
///
/// # Examples
///
/// ```
/// use steady_invest_logic::{HistoricalData, HistoricalYearlyData, calculate_quality_analysis};
/// use rust_decimal::Decimal;
///
/// let data = HistoricalData {
///     records: vec![HistoricalYearlyData {
///         fiscal_year: 2023,
///         sales: Decimal::from(1000),
///         net_income: Some(Decimal::from(150)),
///         pretax_income: Some(Decimal::from(200)),
///         total_equity: Some(Decimal::from(1000)),
///         ..Default::default()
///     }],
///     ..Default::default()
/// };
/// let result = calculate_quality_analysis(&data);
/// assert_eq!(result.points.len(), 1);
/// assert!((result.points[0].roe - 15.0).abs() < 0.01);
/// assert!((result.points[0].profit_on_sales - 20.0).abs() < 0.01);
/// ```
pub fn calculate_quality_analysis(data: &HistoricalData) -> QualityAnalysis {
    let mut points = Vec::new();
    let mut last_roe: Option<f64> = None;
    let mut last_profit: Option<f64> = None;

    // Process from oldest to newest to determine trends
    let mut sorted_records = data.records.clone();
    sorted_records.sort_by_key(|r| r.fiscal_year);

    for record in sorted_records {
        let roe = if let (Some(net), Some(equity)) = (record.net_income, record.total_equity) {
            if !equity.is_zero() {
                (net / equity * rust_decimal::Decimal::from(100))
                    .to_f64()
                    .unwrap_or(0.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        let profit_on_sales = if !record.sales.is_zero() {
            if let Some(pretax) = record.pretax_income {
                (pretax / record.sales * rust_decimal::Decimal::from(100))
                    .to_f64()
                    .unwrap_or(0.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        let roe_trend = match last_roe {
            Some(last) if roe > last + 0.1 => TrendIndicator::Up,
            Some(last) if roe < last - 0.1 => TrendIndicator::Down,
            _ => TrendIndicator::Stable,
        };

        let profit_trend = match last_profit {
            Some(last) if profit_on_sales > last + 0.1 => TrendIndicator::Up,
            Some(last) if profit_on_sales < last - 0.1 => TrendIndicator::Down,
            _ => TrendIndicator::Stable,
        };

        points.push(QualityPoint {
            year: record.fiscal_year,
            roe,
            profit_on_sales,
            roe_trend,
            profit_trend,
        });

        last_roe = Some(roe);
        last_profit = Some(profit_on_sales);
    }

    // Return in chronological order
    QualityAnalysis { points }
}

/// Calculates the NAIC upside/downside ratio for investment decision-making.
///
/// The ratio measures how much potential gain (upside) exists relative to
/// potential loss (downside) based on projected target prices. NAIC recommends
/// investing only in companies where this ratio is **at least 3:1**.
///
/// # Formula
///
/// ```text
/// upside   = projected_high_price - current_price
/// downside = current_price - projected_low_price
/// ratio    = upside / downside
/// ```
///
/// # Arguments
///
/// * `current_price` — The current market price (or latest high price from historical data).
/// * `projected_high_price` — The target high price (projected_high_pe × projected_eps_5yr).
/// * `projected_low_price` — The target low price (projected_low_pe × projected_eps_5yr).
///
/// # Returns
///
/// `Some(ratio)` if the downside is positive (current price above the low target),
/// or `None` if the downside is zero or negative (current price at or below the
/// low target, meaning there is no measurable downside risk).
///
/// # Examples
///
/// ```
/// use steady_invest_logic::calculate_upside_downside_ratio;
///
/// // NAIC example: stock at $50, target high $100, target low $35
/// // Upside = $50, Downside = $15, Ratio = 3.33 (meets 3:1 rule)
/// let ratio = calculate_upside_downside_ratio(50.0, 100.0, 35.0);
/// assert!((ratio.unwrap() - 3.333).abs() < 0.01);
///
/// // Current price at or below low target → no measurable downside
/// let ratio = calculate_upside_downside_ratio(30.0, 100.0, 35.0);
/// assert!(ratio.is_none());
/// ```
pub fn calculate_upside_downside_ratio(
    current_price: f64,
    projected_high_price: f64,
    projected_low_price: f64,
) -> Option<f64> {
    let downside = current_price - projected_low_price;
    if downside <= 0.0 {
        return None;
    }
    let upside = projected_high_price - current_price;
    Some(upside / downside)
}

/// Extracts current and projected target prices from an [`AnalysisSnapshot`].
///
/// Uses the latest historical record's high price as "current price" and
/// projects 5-year EPS growth to compute target high/low prices.
/// Returns all `None` if no records exist or EPS/price are non-positive.
///
/// # Examples
///
/// ```
/// use steady_invest_logic::{AnalysisSnapshot, HistoricalData, HistoricalYearlyData, extract_snapshot_prices};
/// use rust_decimal::Decimal;
///
/// let snapshot = AnalysisSnapshot {
///     historical_data: HistoricalData {
///         records: vec![HistoricalYearlyData {
///             fiscal_year: 2023, eps: Decimal::from(10),
///             price_high: Decimal::from(50), ..Default::default()
///         }],
///         ..Default::default()
///     },
///     projected_eps_cagr: 10.0,
///     projected_high_pe: 20.0,
///     projected_low_pe: 10.0,
///     ..Default::default()
/// };
/// let prices = extract_snapshot_prices(&snapshot);
/// assert!((prices.current_price.unwrap() - 50.0).abs() < 0.01);
/// assert!(prices.target_high_price.unwrap() > 0.0);
/// ```
pub fn extract_snapshot_prices(snapshot: &AnalysisSnapshot) -> SnapshotPrices {
    use rust_decimal::prelude::ToPrimitive;

    let latest = match snapshot
        .historical_data
        .records
        .iter()
        .max_by_key(|r| r.fiscal_year)
    {
        Some(r) => r,
        None => return SnapshotPrices::default(),
    };

    let current_price = latest.price_high.to_f64();
    let current_eps = latest.eps.to_f64();

    let (target_high, target_low) = match (current_eps, current_price) {
        (Some(eps), Some(price)) if eps > 0.0 && price > 0.0 => {
            let projected_eps_5yr = project_forward(eps, snapshot.projected_eps_cagr, 5);
            (
                Some(snapshot.projected_high_pe * projected_eps_5yr),
                Some(snapshot.projected_low_pe * projected_eps_5yr),
            )
        }
        _ => (None, None),
    };

    SnapshotPrices {
        current_price,
        target_high_price: target_high,
        target_low_price: target_low,
    }
}

/// Computes the NAIC upside/downside ratio directly from an [`AnalysisSnapshot`].
///
/// Delegates price extraction to [`extract_snapshot_prices`] and the final
/// ratio to [`calculate_upside_downside_ratio`].
///
/// Returns `None` if historical records are empty, EPS/price are non-positive,
/// or the current price is at or below the projected low target.
pub fn compute_upside_downside_from_snapshot(snapshot: &AnalysisSnapshot) -> Option<f64> {
    let prices = extract_snapshot_prices(snapshot);
    let current = prices.current_price.filter(|&p| p > 0.0)?;
    let high = prices.target_high_price?;
    let low = prices.target_low_price?;
    calculate_upside_downside_ratio(current, high, low)
}

/// Computes CAGR and a best-fit linear regression trendline for a series of values.
///
/// Regression is performed in log-space (`ln(y) = mx + b`) to produce a straight
/// line on logarithmic charts, which represents constant percentage growth.
/// Non-positive values are excluded from the regression (but the CAGR is still
/// computed from the first and last values in the input).
///
/// # Arguments
///
/// * `years`  — Fiscal years corresponding to each value (must be same length as `values`).
/// * `values` — Observed values (e.g., Sales or EPS) for each year.
///
/// # Returns
///
/// A [`TrendAnalysis`] containing the CAGR (as a percentage) and the trendline points.
/// Returns [`TrendAnalysis::default()`] if fewer than 2 data points are provided.
///
/// # Examples
///
/// ```
/// use steady_invest_logic::calculate_growth_analysis;
///
/// let years  = vec![2020, 2021, 2022, 2023];
/// let values = vec![100.0, 110.0, 121.0, 133.1];
/// let result = calculate_growth_analysis(&years, &values);
/// assert!((result.cagr - 10.0).abs() < 0.1);
/// ```
pub fn calculate_growth_analysis(years: &[i32], values: &[f64]) -> TrendAnalysis {
    if years.len() < 2 || years.len() != values.len() {
        return TrendAnalysis::default();
    }

    // 1. CAGR Calculation
    // (End / Start) ^ (1/n) - 1
    let n = (years.len() - 1) as f64;
    let start_val = values[0];
    let end_val = *values.last().unwrap();

    let cagr = if start_val > 0.0 && end_val > 0.0 {
        ((end_val / start_val).powf(1.0 / n) - 1.0) * 100.0
    } else {
        0.0
    };

    // 2. Best-fit Linear Regression in Log Space
    // y = exp(mx + b)  => ln(y) = mx + b
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xx = 0.0;
    let mut sum_xy = 0.0;
    let mut n_pts = 0.0;

    for (i, &v) in values.iter().enumerate() {
        if v > 0.0 {
            let x = years[i] as f64;
            let y = v.ln();
            if y.is_finite() {
                sum_x += x;
                sum_y += y;
                sum_xx += x * x;
                sum_xy += x * y;
                n_pts += 1.0;
            }
        }
    }

    if n_pts < 2.0 {
        return TrendAnalysis {
            cagr,
            trendline: Vec::new(),
        };
    }

    let denominator = n_pts * sum_xx - sum_x * sum_x;
    let trendline = if denominator != 0.0 {
        let m = (n_pts * sum_xy - sum_x * sum_y) / denominator;
        let b = (sum_y - m * sum_x) / n_pts;

        years
            .iter()
            .map(|&year| TrendPoint {
                year,
                value: (m * year as f64 + b).exp(),
            })
            .collect()
    } else {
        Vec::new()
    };

    TrendAnalysis { cagr, trendline }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_growth_analysis() {
        let years = vec![2010, 2011, 2012, 2013, 2014];
        // Perfect 10% growth: 100, 110, 121, 133.1, 146.41
        let values = vec![100.0, 110.0, 121.0, 133.1, 146.41];

        let analysis = calculate_growth_analysis(&years, &values);

        // CAGR should be exactly 10%
        assert!((analysis.cagr - 10.0).abs() < 0.001);

        // Trendline should match original values for a perfect geometric series
        assert!((analysis.trendline[0].value - 100.0).abs() < 0.001);
        assert!((analysis.trendline[4].value - 146.41).abs() < 0.001);
    }

    #[test]
    fn test_growth_analysis_robustness() {
        let years = vec![2010, 2011, 2012, 2013];
        let values = vec![0.0, -10.0, 100.0, 110.0];

        let analysis = calculate_growth_analysis(&years, &values);

        // CAGR should be 0 because start_val is 0.0
        assert_eq!(analysis.cagr, 0.0);

        // Trendline should exist because we have 2 positive points (100, 110)
        assert_eq!(analysis.trendline.len(), 4);
        assert!(analysis.trendline[0].value > 0.0);
    }

    #[test]
    fn test_quality_analysis() {
        let mut data = HistoricalData::default();
        data.records = vec![
            HistoricalYearlyData {
                fiscal_year: 2020,
                sales: Decimal::from(100),
                net_income: Some(Decimal::from(10)),
                pretax_income: Some(Decimal::from(15)),
                total_equity: Some(Decimal::from(100)),
                overrides: vec![],
                ..Default::default()
            },
            HistoricalYearlyData {
                fiscal_year: 2021,
                sales: Decimal::from(200),
                net_income: Some(Decimal::from(30)),
                pretax_income: Some(Decimal::from(40)),
                total_equity: Some(Decimal::from(200)),
                overrides: vec![],
                ..Default::default()
            },
        ];

        let analysis = calculate_quality_analysis(&data);

        // 2020: ROE = 10/100 = 10%, Profit = 15/100 = 15%
        // 2021: ROE = 30/200 = 15%, Profit = 40/200 = 20%
        assert_eq!(analysis.points.len(), 2);
        assert!((analysis.points[0].roe - 10.0).abs() < 0.001);
        assert!((analysis.points[1].roe - 15.0).abs() < 0.001);
        assert_eq!(analysis.points[1].roe_trend, TrendIndicator::Up);
        assert_eq!(analysis.points[1].profit_trend, TrendIndicator::Up);
    }

    #[test]
    fn test_pe_ranges_math() {
        let mut data = HistoricalData::default();
        data.records = vec![
            HistoricalYearlyData {
                fiscal_year: 2020,
                eps: Decimal::from(10),
                price_high: Decimal::from(150),
                price_low: Decimal::from(100),
                overrides: vec![],
                ..Default::default()
            },
            HistoricalYearlyData {
                fiscal_year: 2021,
                eps: Decimal::from(20),
                price_high: Decimal::from(400),
                price_low: Decimal::from(300),
                overrides: vec![],
                ..Default::default()
            },
            // Edge case: Negative EPS should be ignored
            HistoricalYearlyData {
                fiscal_year: 2022,
                eps: Decimal::from(-5),
                price_high: Decimal::from(100),
                price_low: Decimal::from(50),
                overrides: vec![],
                ..Default::default()
            },
        ];

        let analysis = calculate_pe_ranges(&data);
        assert_eq!(analysis.points.len(), 2);
        assert!((analysis.avg_high_pe - 17.5).abs() < 0.001);
        assert!((analysis.avg_low_pe - 12.5).abs() < 0.001);
    }

    #[test]
    fn test_pe_ranges_5year_limit() {
        let mut data = HistoricalData::default();
        // 12 years of data
        for i in 0..12 {
            data.records.push(HistoricalYearlyData {
                fiscal_year: 2000 + i,
                eps: Decimal::from(1),
                price_high: Decimal::from(10 + i), // High PE = 10..21
                price_low: Decimal::from(5 + i),   // Low PE = 5..16
                ..Default::default()
            });
        }

        let analysis = calculate_pe_ranges(&data);
        // NAIC SSG Section 3: 5-year P/E history
        assert_eq!(analysis.points.len(), 5);
        // Should only include years 2007-2011 (last 5)
        assert_eq!(analysis.points[0].year, 2007);
        assert_eq!(analysis.points[4].year, 2011);

        // Avg High PE of years 2007-2011: high prices = 17,18,19,20,21 → avg = 19.0
        assert!((analysis.avg_high_pe - 19.0).abs() < 0.001);
        // Avg Low PE of years 2007-2011: low prices = 12,13,14,15,16 → avg = 14.0
        assert!((analysis.avg_low_pe - 14.0).abs() < 0.001);
    }

    #[test]
    fn test_upside_downside_ratio_meets_naic_rule() {
        // Stock at $50, target high $100, target low $35
        // Upside = $50, Downside = $15 → Ratio = 3.33
        let ratio = calculate_upside_downside_ratio(50.0, 100.0, 35.0);
        assert!((ratio.unwrap() - 3.333).abs() < 0.01);
    }

    #[test]
    fn test_upside_downside_ratio_below_threshold() {
        // Stock at $80, target high $100, target low $60
        // Upside = $20, Downside = $20 → Ratio = 1.0
        let ratio = calculate_upside_downside_ratio(80.0, 100.0, 60.0);
        assert!((ratio.unwrap() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_upside_downside_ratio_no_downside() {
        // Current price at low target → None
        let ratio = calculate_upside_downside_ratio(35.0, 100.0, 35.0);
        assert!(ratio.is_none());

        // Current price below low target → None
        let ratio = calculate_upside_downside_ratio(30.0, 100.0, 35.0);
        assert!(ratio.is_none());
    }

    #[test]
    fn test_extract_snapshot_prices() {
        let snapshot = AnalysisSnapshot {
            historical_data: HistoricalData {
                records: vec![HistoricalYearlyData {
                    fiscal_year: 2023,
                    eps: Decimal::from(10),
                    price_high: Decimal::from(50),
                    ..Default::default()
                }],
                ..Default::default()
            },
            projected_eps_cagr: 10.0,
            projected_high_pe: 20.0,
            projected_low_pe: 10.0,
            ..Default::default()
        };
        let prices = extract_snapshot_prices(&snapshot);
        assert!((prices.current_price.unwrap() - 50.0).abs() < 0.01);
        // projected_eps_5yr = 10 * 1.1^5 ≈ 16.1051
        // target_high = 20 * 16.1051 ≈ 322.10
        // target_low = 10 * 16.1051 ≈ 161.05
        assert!((prices.target_high_price.unwrap() - 322.10).abs() < 0.1);
        assert!((prices.target_low_price.unwrap() - 161.05).abs() < 0.1);
    }

    #[test]
    fn test_extract_snapshot_prices_empty_records() {
        let snapshot = AnalysisSnapshot::default();
        let prices = extract_snapshot_prices(&snapshot);
        assert!(prices.current_price.is_none());
        assert!(prices.target_high_price.is_none());
        assert!(prices.target_low_price.is_none());
    }

    #[test]
    fn test_snapshot_serialization() {
        let snapshot = AnalysisSnapshot {
            historical_data: HistoricalData {
                ticker: "TEST".to_string(),
                currency: "USD".to_string(),
                records: vec![HistoricalYearlyData {
                    fiscal_year: 2023,
                    sales: Decimal::from(100),
                    eps: Decimal::from(10),
                    ..Default::default()
                }],
                ..Default::default()
            },
            projected_sales_cagr: 10.5,
            projected_eps_cagr: 12.0,
            projected_ptp_cagr: 8.0,
            projected_high_pe: 25.0,
            projected_low_pe: 15.0,
            analyst_note: "Test note".to_string(),
            captured_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: AnalysisSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(
            snapshot.historical_data.ticker,
            deserialized.historical_data.ticker
        );
        assert_eq!(
            snapshot.projected_sales_cagr,
            deserialized.projected_sales_cagr
        );
        assert_eq!(snapshot.projected_eps_cagr, deserialized.projected_eps_cagr);
        assert_eq!(snapshot.projected_ptp_cagr, deserialized.projected_ptp_cagr);
        assert_eq!(snapshot.projected_high_pe, deserialized.projected_high_pe);
        assert_eq!(snapshot.projected_low_pe, deserialized.projected_low_pe);
        assert_eq!(snapshot.analyst_note, deserialized.analyst_note);

        // Verify backward compatibility: JSON without projected_ptp_cagr deserializes to 0.0
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value.as_object_mut().unwrap().remove("projected_ptp_cagr");
        let from_old: AnalysisSnapshot = serde_json::from_value(value).unwrap();
        assert_eq!(from_old.projected_ptp_cagr, 0.0);
    }

    // ================================================================
    // NAIC SSG Handbook Golden Tests
    // ================================================================

    /// Verify 5-year P/E range averages match NAIC Section 3 methodology.
    #[test]
    fn test_naic_handbook_pe_5year_average() {
        let mut data = HistoricalData::default();
        // 5 early years (should be EXCLUDED by 5-year limit)
        for year in 2006..=2010 {
            data.records.push(HistoricalYearlyData {
                fiscal_year: year,
                eps: Decimal::from(5),
                price_high: Decimal::from(200), // PE = 40
                price_low: Decimal::from(150),  // PE = 30
                ..Default::default()
            });
        }
        // Last 5 years (2011-2015): crafted to produce handbook averages
        let years_data: [(i32, i64, i64, i64); 5] = [
            (2011, 4, 112, 80), // H=28.0, L=20.0
            (2012, 4, 108, 76), // H=27.0, L=19.0
            (2013, 4, 112, 84), // H=28.0, L=21.0
            (2014, 4, 116, 80), // H=29.0, L=20.0
            (2015, 4, 110, 80), // H=27.5, L=20.0
        ];
        for &(year, eps, high, low) in &years_data {
            data.records.push(HistoricalYearlyData {
                fiscal_year: year,
                eps: Decimal::from(eps),
                price_high: Decimal::from(high),
                price_low: Decimal::from(low),
                ..Default::default()
            });
        }

        let analysis = calculate_pe_ranges(&data);

        assert_eq!(analysis.points.len(), 5);
        assert_eq!(analysis.points[0].year, 2011);
        assert_eq!(analysis.points[4].year, 2015);

        assert!(
            (analysis.avg_high_pe - 27.9).abs() < 0.001,
            "Avg High P/E: expected 27.9, got {}",
            analysis.avg_high_pe
        );
        assert!(
            (analysis.avg_low_pe - 20.0).abs() < 0.001,
            "Avg Low P/E: expected 20.0, got {}",
            analysis.avg_low_pe
        );
    }

    /// Verify NAIC Section 4A forecast price formula.
    #[test]
    fn test_naic_handbook_forecast_high_price() {
        let avg_high_pe: f64 = 27.9;
        let estimated_high_eps: f64 = 9.37;
        let expected_forecast: f64 = 261.3;

        let computed = avg_high_pe * estimated_high_eps;
        assert!(
            (computed - expected_forecast).abs() < 0.2,
            "Forecast High: expected ~{}, got {:.1}",
            expected_forecast,
            computed
        );
    }

    /// Verify NAIC upside/downside ratio calculation.
    #[test]
    fn test_naic_handbook_upside_downside_ratio() {
        let current_price = 149.83;
        let forecast_high = 261.3;
        let forecast_low = 116.4;

        let ratio = calculate_upside_downside_ratio(current_price, forecast_high, forecast_low);

        assert!(ratio.is_some());
        let r = ratio.unwrap();
        assert!(
            (r - 3.3).abs() < 0.1,
            "Upside/downside ratio: expected ~3.3, got {:.2}",
            r
        );
        assert!(
            (r - 3.334).abs() < 0.01,
            "Exact ratio: expected 3.334, got {:.3}",
            r
        );
    }

    /// Verify NAIC quality metrics (Evaluate Management, Section 2).
    #[test]
    fn test_naic_handbook_quality_metrics() {
        let data = HistoricalData {
            records: vec![
                HistoricalYearlyData {
                    fiscal_year: 2014,
                    sales: Decimal::from(950),
                    pretax_income: Some(Decimal::from(300)),
                    net_income: Some(Decimal::from(210)),
                    total_equity: Some(Decimal::from(538)),
                    ..Default::default()
                },
                HistoricalYearlyData {
                    fiscal_year: 2015,
                    sales: Decimal::from(1007),
                    pretax_income: Some(Decimal::from(334)),
                    net_income: Some(Decimal::from(250)),
                    total_equity: Some(Decimal::from(570)),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let quality = calculate_quality_analysis(&data);

        assert_eq!(quality.points.len(), 2);

        let y2014 = &quality.points[0];
        assert!(
            (y2014.profit_on_sales - 31.6).abs() < 0.1,
            "2014 PTP/Sales: expected ~31.6%, got {:.1}%",
            y2014.profit_on_sales
        );
        assert!(
            (y2014.roe - 39.0).abs() < 0.1,
            "2014 ROE: expected ~39.0%, got {:.1}%",
            y2014.roe
        );

        let y2015 = &quality.points[1];
        assert!(
            (y2015.profit_on_sales - 33.2).abs() < 0.1,
            "2015 PTP/Sales: expected ~33.2%, got {:.1}%",
            y2015.profit_on_sales
        );
        assert!(
            (y2015.roe - 43.9).abs() < 0.1,
            "2015 ROE: expected ~43.9%, got {:.1}%",
            y2015.roe
        );

        assert_eq!(y2015.profit_trend, TrendIndicator::Up);
        assert_eq!(y2015.roe_trend, TrendIndicator::Up);
    }
}
