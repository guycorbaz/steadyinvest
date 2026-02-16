//! # NAIC Stock Selection Guide — Core Business Logic
//!
//! This crate contains the shared financial analysis logic used by both the
//! backend API and the Leptos frontend (via WASM). It implements the key
//! calculations from the **NAIC Stock Selection Guide (SSG)** methodology:
//!
//! - **Growth analysis** — logarithmic trendline regression and CAGR calculation
//!   for Sales and EPS series ([`calculate_growth_analysis`])
//! - **P/E range analysis** — historical High/Low P/E ratios averaged over the
//!   last 10 years ([`calculate_pe_ranges`])
//! - **Quality metrics** — ROE and Profit-on-Sales with year-over-year trend
//!   indicators ([`calculate_quality_analysis`])
//! - **Projections** — CAGR-based future trendlines for valuation zone
//!   calculations ([`calculate_projected_trendline`])
//!
//! ## Key Types
//!
//! - [`HistoricalData`] — aggregated financial records with adjustment and
//!   normalization methods
//! - [`AnalysisSnapshot`] — point-in-time capture of an analyst's full thesis
//! - [`TrendAnalysis`] — CAGR value plus best-fit trendline points
//! - [`PeRangeAnalysis`] — per-year High/Low P/E with computed averages
//!
//! ## Design Principles
//!
//! All business logic lives in this crate — UI components consume results only.
//! Financial values use [`rust_decimal::Decimal`] for precision; intermediate
//! math (trendlines, CAGR) uses `f64` where acceptable.

use serde::{Deserialize, Serialize};
use rust_decimal::prelude::ToPrimitive;

/// Basic identity information for a security ticker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TickerInfo {
    /// The trading symbol (e.g., "AAPL").
    pub ticker: String,
    /// The full descriptive name (e.g., "Apple Inc.").
    pub name: String,
    /// The exchange where the security is listed (e.g., "NASDAQ").
    pub exchange: String,
    /// The native currency of the security.
    pub currency: String,
}

/// A manual data override for a specific field in a [`HistoricalYearlyData`] record.
///
/// Overrides allow an analyst to replace API-sourced values with corrected figures
/// (e.g., fixing a misreported EPS) while preserving an audit trail.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ManualOverride {
    /// The name of the field being overridden (e.g., `"eps"`, `"sales"`).
    pub field_name: String,
    /// The replacement value.
    pub value: rust_decimal::Decimal,
    /// Optional analyst note explaining why the override was applied.
    pub note: Option<String>,
}

/// Financial results and pricing for a single fiscal year.
///
/// Each record represents one row in the SSG historical data table. Fields
/// marked `Option` may be unavailable from certain data providers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct HistoricalYearlyData {
    /// The calendar year of the fiscal period (e.g., 2023).
    pub fiscal_year: i32,
    /// Total revenue / net sales.
    pub sales: rust_decimal::Decimal,
    /// Earnings per share (diluted).
    pub eps: rust_decimal::Decimal,
    /// Highest stock price during the fiscal year.
    pub price_high: rust_decimal::Decimal,
    /// Lowest stock price during the fiscal year.
    pub price_low: rust_decimal::Decimal,
    /// Net income after tax (used for ROE calculation).
    pub net_income: Option<rust_decimal::Decimal>,
    /// Pre-tax income (used for Profit-on-Sales calculation).
    pub pretax_income: Option<rust_decimal::Decimal>,
    /// Total shareholders' equity (used for ROE calculation).
    pub total_equity: Option<rust_decimal::Decimal>,
    /// Multiplier to adjust historical values for splits/dividends.
    pub adjustment_factor: rust_decimal::Decimal,
    /// Rate to convert native currency to user's display currency.
    pub exchange_rate: Option<rust_decimal::Decimal>,
    /// List of manual overrides for this year.
    pub overrides: Vec<ManualOverride>,
}

/// A collection of historical financial records for a ticker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct HistoricalData {
    /// The trading symbol this data belongs to (e.g., `"AAPL"`).
    pub ticker: String,
    /// Native currency of the historical data.
    pub currency: String,
    /// Current display currency (if normalized).
    pub display_currency: Option<String>,
    /// Chronological list of yearly records.
    pub records: Vec<HistoricalYearlyData>,
    /// Flag indicating if data retrieval was successful/complete.
    pub is_complete: bool,
    /// Flag indicating if split/dividend adjustments have been applied.
    pub is_split_adjusted: bool,
    /// Calculated P/E ranges and averages (last 10 years).
    pub pe_range_analysis: Option<PeRangeAnalysis>,
}

impl HistoricalData {
    /// Applies split and dividend adjustments to EPS and price fields.
    ///
    /// Multiplies `eps`, `price_high`, and `price_low` by each record's
    /// `adjustment_factor`. Records with a factor of `1` are left unchanged.
    /// This method is idempotent — calling it again after adjustment is a no-op.
    pub fn apply_adjustments(&mut self) {
        if self.is_split_adjusted {
            return;
        }

        let mut adjusted = false;
        for record in &mut self.records {
            if record.adjustment_factor != rust_decimal::Decimal::ONE {
                record.eps *= record.adjustment_factor;
                record.price_high *= record.adjustment_factor;
                record.price_low *= record.adjustment_factor;
                adjusted = true;
            }
        }
        if adjusted {
            self.is_split_adjusted = true;
        }
    }

    /// Normalizes all monetary fields to `target_currency` using per-record exchange rates.
    ///
    /// Converts `sales`, `eps`, `price_high`, `price_low`, `net_income`,
    /// `pretax_income`, and `total_equity`. Records without an `exchange_rate`
    /// are left unchanged. This method is idempotent for the same target currency.
    pub fn apply_normalization(&mut self, target_currency: &str) {
        if self.display_currency.as_deref() == Some(target_currency) {
            return;
        }

        for record in &mut self.records {
            if let Some(rate) = record.exchange_rate {
                record.sales *= rate;
                record.eps *= rate;
                record.price_high *= rate;
                record.price_low *= rate;
                if let Some(ref mut val) = record.net_income { *val *= rate; }
                if let Some(ref mut val) = record.pretax_income { *val *= rate; }
                if let Some(ref mut val) = record.total_equity { *val *= rate; }
            }
        }
        self.display_currency = Some(target_currency.to_string());
    }
}

/// A single data point on a calculated trendline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TrendPoint {
    /// Fiscal year of this data point.
    pub year: i32,
    /// The calculated value on the best-fit line.
    pub value: f64,
}

/// The result of a growth analysis operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TrendAnalysis {
    /// Compound Annual Growth Rate (expressed as a percentage, e.g., 10.5).
    pub cagr: f64,
    /// Points defining the best-fit linear regression line in log space.
    pub trendline: Vec<TrendPoint>,
}

/// Year-over-year direction of a quality metric (ROE or Profit-on-Sales).
///
/// A threshold of ±0.1 percentage points is used to filter noise.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum TrendIndicator {
    /// The metric changed by less than 0.1 pp from the prior year.
    #[default]
    Stable,
    /// The metric increased by at least 0.1 pp from the prior year.
    Up,
    /// The metric decreased by at least 0.1 pp from the prior year.
    Down,
}

/// A single year's quality metrics as displayed on the SSG Quality Dashboard.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct QualityPoint {
    /// Fiscal year of the data point.
    pub year: i32,
    /// Return on Equity (%), calculated as `net_income / total_equity * 100`.
    pub roe: f64,
    /// Pre-tax Profit on Sales (%), calculated as `pretax_income / sales * 100`.
    pub profit_on_sales: f64,
    /// Year-over-year trend direction for ROE.
    pub roe_trend: TrendIndicator,
    /// Year-over-year trend direction for Profit-on-Sales.
    pub profit_trend: TrendIndicator,
}

/// Chronological series of quality metrics for the SSG Quality Dashboard.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct QualityAnalysis {
    /// Quality data points sorted oldest-to-newest.
    pub points: Vec<QualityPoint>,
}

/// A single year's High and Low P/E ratios.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PeRangePoint {
    /// Fiscal year of the data point.
    pub year: i32,
    /// High P/E for the year (`price_high / eps`).
    pub high_pe: f64,
    /// Low P/E for the year (`price_low / eps`).
    pub low_pe: f64,
}

/// P/E range analysis over the last 10 years of historical data.
///
/// Years with zero or negative EPS are excluded from the calculation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PeRangeAnalysis {
    /// Per-year P/E data points (up to 10).
    pub points: Vec<PeRangePoint>,
    /// Arithmetic mean of the yearly high P/E values.
    pub avg_high_pe: f64,
    /// Arithmetic mean of the yearly low P/E values.
    pub avg_low_pe: f64,
}

/// A complete snapshot of an analysis at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AnalysisSnapshot {
    /// The core historical data and calculations.
    pub historical_data: HistoricalData,
    /// Projected sales CAGR (%).
    pub projected_sales_cagr: f64,
    /// Projected EPS CAGR (%).
    pub projected_eps_cagr: f64,
    /// Future average high P/E projected by the user.
    pub projected_high_pe: f64,
    /// Future average low P/E projected by the user.
    pub projected_low_pe: f64,
    /// Optional analyst notes or thesis description.
    pub analyst_note: String,
    /// The timestamp when the snapshot was captured (UTC).
    pub captured_at: chrono::DateTime<chrono::Utc>,
}

/// Computes historical High/Low P/E ratios and their averages.
///
/// Strictly limited to the **last 10 completed years** of data. Years with
/// zero or negative EPS are skipped (they produce meaningless P/E ratios).
///
/// # Arguments
///
/// * `data` — Historical financial data; only the `records` field is read.
///
/// # Returns
///
/// A [`PeRangeAnalysis`] with up to 10 per-year P/E points and their averages.
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
    
    // Take the last 10 records
    let len = eligible_records.len();
    let start_idx = if len > 10 { len - 10 } else { 0 };
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
                (net / equity * rust_decimal::Decimal::from(100)).to_f64().unwrap_or(0.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        let profit_on_sales = if !record.sales.is_zero() {
            if let Some(pretax) = record.pretax_income {
                (pretax / record.sales * rust_decimal::Decimal::from(100)).to_f64().unwrap_or(0.0)
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

/// Computes the NAIC upside/downside ratio directly from an [`AnalysisSnapshot`].
///
/// Extracts the latest historical record (by fiscal year), uses its EPS and
/// high price as "current" values, projects 5-year EPS using the snapshot's
/// `projected_eps_cagr`, and computes target high/low prices from projected P/E
/// ratios. Delegates the final ratio to [`calculate_upside_downside_ratio`].
///
/// Returns `None` if historical records are empty, EPS/price are non-positive,
/// or the current price is at or below the projected low target.
pub fn compute_upside_downside_from_snapshot(snapshot: &AnalysisSnapshot) -> Option<f64> {
    use rust_decimal::prelude::ToPrimitive;

    let latest = snapshot
        .historical_data
        .records
        .iter()
        .max_by_key(|r| r.fiscal_year)?;
    let current_eps = latest.eps.to_f64()?;
    let current_price = latest.price_high.to_f64()?;
    if current_eps <= 0.0 || current_price <= 0.0 {
        return None;
    }
    let projected_eps_5yr = current_eps * (1.0 + snapshot.projected_eps_cagr / 100.0).powf(5.0);
    let target_high = snapshot.projected_high_pe * projected_eps_5yr;
    let target_low = snapshot.projected_low_pe * projected_eps_5yr;
    calculate_upside_downside_ratio(current_price, target_high, target_low)
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
        return TrendAnalysis { cagr, trendline: Vec::new() };
    }

    let denominator = n_pts * sum_xx - sum_x * sum_x;
    let trendline = if denominator != 0.0 {
        let m = (n_pts * sum_xy - sum_x * sum_y) / denominator;
        let b = (sum_y - m * sum_x) / n_pts;

        years.iter().map(|&year| {
            TrendPoint {
                year,
                value: (m * year as f64 + b).exp(),
            }
        }).collect()
    } else {
        Vec::new()
    };

    TrendAnalysis { cagr, trendline }
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
    use rust_decimal::Decimal;

    #[test]
    fn test_currency_normalization() {
        let mut data = HistoricalData {
            ticker: "NESN.SW".to_string(),
            currency: "CHF".to_string(),
            display_currency: None,
            is_complete: true,
            is_split_adjusted: true,
            records: vec![
                HistoricalYearlyData {
                    fiscal_year: 2021,
                    sales: Decimal::from(100),
                    eps: Decimal::from(10),
                    price_high: Decimal::from(1000),
                    price_low: Decimal::from(800),
                    adjustment_factor: Decimal::ONE,
                    exchange_rate: Some(Decimal::new(11, 1)), // 1.1
                    net_income: None,
                    pretax_income: None,
                    total_equity: None,
                    overrides: vec![],
                },
            ],
            pe_range_analysis: None,
        };

        data.apply_normalization("USD");

        assert_eq!(data.display_currency, Some("USD".to_string()));
        assert_eq!(data.records[0].sales, Decimal::from(110)); // 100 * 1.1
        assert_eq!(data.records[0].eps, Decimal::from(11)); // 10 * 1.1
    }

    #[test]
    fn test_split_adjustment() {
        let mut data = HistoricalData {
            ticker: "AAPL".to_string(),
            currency: "USD".to_string(),
            display_currency: None,
            is_complete: true,
            is_split_adjusted: false,
            records: vec![
                HistoricalYearlyData {
                    fiscal_year: 2021,
                    eps: Decimal::from(10),
                    price_high: Decimal::from(100),
                    price_low: Decimal::from(80),
                    adjustment_factor: Decimal::ONE,
                    exchange_rate: None,
                    ..Default::default()
                },
                HistoricalYearlyData {
                    fiscal_year: 2019,
                    eps: Decimal::from(5),
                    price_high: Decimal::from(50),
                    price_low: Decimal::from(40),
                    adjustment_factor: Decimal::from(2), // 2:1 split
                    exchange_rate: None,
                    ..Default::default()
                },
            ],
            pe_range_analysis: None,
        };

        data.apply_adjustments();

        assert!(data.is_split_adjusted);
        // 2021 should remain unchanged
        assert_eq!(data.records[0].eps, Decimal::from(10));
        // 2019 should be doubled
        assert_eq!(data.records[1].eps, Decimal::from(10));
        assert_eq!(data.records[1].price_high, Decimal::from(100));
        assert_eq!(data.records[1].price_low, Decimal::from(80));
    }

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
    fn test_pe_ranges_10year_limit() {
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
        assert_eq!(analysis.points.len(), 10);
        // Should only include years 2002-2011 (last 10)
        assert_eq!(analysis.points[0].year, 2002);
        assert_eq!(analysis.points[9].year, 2011);
        
        // Avg of 12..21 = (12+21)/2 = 16.5
        assert!((analysis.avg_high_pe - 16.5).abs() < 0.001);
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
            projected_high_pe: 25.0,
            projected_low_pe: 15.0,
            analyst_note: "Test note".to_string(),
            captured_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: AnalysisSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(snapshot.historical_data.ticker, deserialized.historical_data.ticker);
        assert_eq!(snapshot.projected_sales_cagr, deserialized.projected_sales_cagr);
        assert_eq!(snapshot.projected_high_pe, deserialized.projected_high_pe);
        assert_eq!(snapshot.analyst_note, deserialized.analyst_note);
    }
}
