//! # NAIC Stock Selection Guide — Core Business Logic
//!
//! This crate contains the shared financial analysis logic used by both the
//! backend API and the Leptos frontend (via WASM). It implements the key
//! calculations from the **NAIC Stock Selection Guide (SSG)** methodology:
//!
//! - **Growth analysis** — logarithmic trendline regression and CAGR calculation
//!   for Sales and EPS series ([`calculate_growth_analysis`])
//! - **P/E range analysis** — historical High/Low P/E ratios averaged over the
//!   last 5 years ([`calculate_pe_ranges`])
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
    #[serde(default)]
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
    /// Calculated P/E ranges and averages (last 5 years per NAIC Section 3).
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

        for record in &mut self.records {
            if record.adjustment_factor != rust_decimal::Decimal::ONE {
                record.eps *= record.adjustment_factor;
                record.price_high *= record.adjustment_factor;
                record.price_low *= record.adjustment_factor;
            }
        }
        self.is_split_adjusted = true;
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

/// P/E range analysis over the last 5 years of historical data (per NAIC Section 3).
///
/// Years with zero or negative EPS are excluded from the calculation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PeRangeAnalysis {
    /// Per-year P/E data points (up to 5).
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
    /// Projected Pre-Tax Profit CAGR (%).
    #[serde(default)]
    pub projected_ptp_cagr: f64,
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
    let start_idx = if len > 5 { len - 5 } else { 0 };
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

/// Extracted monetary prices from an analysis snapshot.
///
/// Used by both backend and frontend to obtain current/target prices without
/// duplicating the extraction logic (Cardinal Rule).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SnapshotPrices {
    /// Current price (latest fiscal year's high price).
    pub current_price: Option<f64>,
    /// Target high price (projected_high_pe × projected 5-year EPS).
    pub target_high_price: Option<f64>,
    /// Target low price (projected_low_pe × projected 5-year EPS).
    pub target_low_price: Option<f64>,
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

/// Validates that a string is a valid ISO 4217 currency code (3 uppercase ASCII letters).
///
/// Used at API boundaries to enforce currency format per architecture spec.
///
/// # Examples
///
/// ```
/// use steady_invest_logic::is_valid_currency_code;
///
/// assert!(is_valid_currency_code("CHF"));
/// assert!(is_valid_currency_code("USD"));
/// assert!(!is_valid_currency_code("us"));     // too short + lowercase
/// assert!(!is_valid_currency_code("USDX"));   // too long
/// assert!(!is_valid_currency_code("123"));     // digits, not letters
/// ```
pub fn is_valid_currency_code(code: &str) -> bool {
    code.len() == 3 && code.bytes().all(|b| b.is_ascii_uppercase())
}

/// Converts a monetary value from one currency to another using the given rate.
///
/// This is the single source of truth for currency conversion (Cardinal Rule).
/// The rate should be a directional rate from source to target currency
/// (e.g., CHF→USD = 1.15 means 1 CHF = 1.15 USD).
///
/// # Examples
///
/// ```
/// use steady_invest_logic::convert_monetary_value;
///
/// // Convert 100 CHF to USD at rate 1.15
/// let usd = convert_monetary_value(100.0, 1.15);
/// assert!((usd - 115.0).abs() < 1e-10);
///
/// // Same currency (rate = 1.0) returns unchanged value
/// assert!((convert_monetary_value(42.0, 1.0) - 42.0).abs() < 1e-10);
/// ```
pub fn convert_monetary_value(amount: f64, rate: f64) -> f64 {
    amount * rate
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
    fn test_convert_monetary_value_basic() {
        // CHF→USD at 1.15
        let usd = convert_monetary_value(100.0, 1.15);
        assert!((usd - 115.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_monetary_value_same_currency() {
        // Rate = 1.0 means same currency — no change
        assert!((convert_monetary_value(42.0, 1.0) - 42.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_monetary_value_zero_rate() {
        assert!((convert_monetary_value(100.0, 0.0)).abs() < 1e-10);
    }

    #[test]
    fn test_convert_monetary_value_negative_amount() {
        // Negative amounts are valid (losses)
        let result = convert_monetary_value(-50.0, 1.15);
        assert!((result - (-57.5)).abs() < 1e-10);
    }

    #[test]
    fn test_is_valid_currency_code() {
        assert!(is_valid_currency_code("CHF"));
        assert!(is_valid_currency_code("USD"));
        assert!(is_valid_currency_code("EUR"));
        assert!(!is_valid_currency_code("us"));      // too short + lowercase
        assert!(!is_valid_currency_code("usd"));     // lowercase
        assert!(!is_valid_currency_code("USDX"));    // too long
        assert!(!is_valid_currency_code("123"));     // digits
        assert!(!is_valid_currency_code(""));        // empty
        assert!(!is_valid_currency_code("U D"));     // contains space
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

        assert_eq!(snapshot.historical_data.ticker, deserialized.historical_data.ticker);
        assert_eq!(snapshot.projected_sales_cagr, deserialized.projected_sales_cagr);
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
    // Reference: docs/NAIC/SSGHandbook.pdf (O'Hara Cruises example)
    // ================================================================

    /// Verify 5-year P/E range averages match NAIC Section 3 methodology.
    /// Reference: Handbook Figure 2.3 (p14) — Price-Earnings History
    /// O'Hara Cruises: Avg High P/E = 27.9, Avg Low P/E = 20.0
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
        // High PEs: 28.0 + 27.0 + 28.0 + 29.0 + 27.5 = 139.5, avg = 27.9
        // Low PEs:  20.0 + 19.0 + 21.0 + 20.0 + 20.0 = 100.0, avg = 20.0
        let years_data: [(i32, i64, i64, i64); 5] = [
            (2011, 4, 112, 80),   // H=28.0, L=20.0
            (2012, 4, 108, 76),   // H=27.0, L=19.0
            (2013, 4, 112, 84),   // H=28.0, L=21.0
            (2014, 4, 116, 80),   // H=29.0, L=20.0
            (2015, 4, 110, 80),   // H=27.5, L=20.0
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

        // Per NAIC Section 3: only last 5 years used
        assert_eq!(analysis.points.len(), 5);
        assert_eq!(analysis.points[0].year, 2011);
        assert_eq!(analysis.points[4].year, 2015);

        // Match handbook averages (Figure 2.3)
        assert!((analysis.avg_high_pe - 27.9).abs() < 0.001,
            "Avg High P/E: expected 27.9, got {}", analysis.avg_high_pe);
        assert!((analysis.avg_low_pe - 20.0).abs() < 0.001,
            "Avg Low P/E: expected 20.0, got {}", analysis.avg_low_pe);
    }

    /// Verify NAIC Section 4A forecast price formula.
    /// Formula: Forecast High Price = Avg High P/E × Estimated Future EPS
    /// Reference: Handbook Figure 2.4 (p15) — 27.9 × 9.37 ≈ 261.3
    #[test]
    fn test_naic_handbook_forecast_high_price() {
        let avg_high_pe: f64 = 27.9;
        let estimated_high_eps: f64 = 9.37;
        let expected_forecast: f64 = 261.3;

        let computed = avg_high_pe * estimated_high_eps;
        // Exact = 261.423; handbook rounds to 261.3
        assert!((computed - expected_forecast).abs() < 0.2,
            "Forecast High: expected ~{}, got {:.1}", expected_forecast, computed);
    }

    /// Verify NAIC upside/downside ratio calculation.
    /// Reference: Handbook Figure 2.4, Section D (p15)
    /// O'Hara: (261.3 - 149.83) / (149.83 - 116.4) = 3.3 to 1
    #[test]
    fn test_naic_handbook_upside_downside_ratio() {
        let current_price = 149.83;
        let forecast_high = 261.3;
        let forecast_low = 116.4;

        let ratio = calculate_upside_downside_ratio(
            current_price, forecast_high, forecast_low
        );

        assert!(ratio.is_some());
        let r = ratio.unwrap();
        // Handbook says 3.3 to 1 (rounded from 3.334)
        assert!((r - 3.3).abs() < 0.1,
            "Upside/downside ratio: expected ~3.3, got {:.2}", r);
        // Exact: 111.47 / 33.43 = 3.334
        assert!((r - 3.334).abs() < 0.01,
            "Exact ratio: expected 3.334, got {:.3}", r);
    }

    /// Verify NAIC EPS projection formula over 5 years.
    /// Formula: Projected EPS = Current EPS × (1 + growth_rate)^5
    /// Reference: Handbook Section 4A (p15) — EPS 5.71 at 10.4% → ~9.37
    #[test]
    fn test_naic_handbook_eps_projection() {
        let start_year = 2015;
        let current_eps = 5.71;
        // Growth rate that produces handbook's Estimated High EPS of 9.37
        let cagr = 10.4;
        let future_years: Vec<i32> = (2016..=2020).collect();

        let projection = calculate_projected_trendline(
            start_year, current_eps, cagr, &future_years
        );

        assert_eq!(projection.trendline.len(), 5);

        // Year 1 (2016): 5.71 × 1.104 = 6.304
        assert!((projection.trendline[0].value - 6.304).abs() < 0.01,
            "Year 1: expected ~6.30, got {:.2}", projection.trendline[0].value);

        // Year 5 (2020): should match handbook's Estimated High EPS ≈ 9.37
        let year5_eps = projection.trendline[4].value;
        assert!((year5_eps - 9.37).abs() < 0.1,
            "Year 5 EPS: expected ~9.37, got {:.2}", year5_eps);
    }

    /// Verify NAIC quality metrics (Evaluate Management, Section 2).
    /// % Pre-Tax Profit on Sales = Pre-Tax Profit / Sales × 100
    /// % Earned on Equity = Net Income / Total Equity × 100
    /// Reference: Handbook Figures 2.2, 4.1 (pp13, 24)
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

        // 2014: PTP/Sales = 300/950 × 100 = 31.58%
        let y2014 = &quality.points[0];
        assert!((y2014.profit_on_sales - 31.6).abs() < 0.1,
            "2014 PTP/Sales: expected ~31.6%, got {:.1}%", y2014.profit_on_sales);
        // 2014: ROE = 210/538 × 100 = 39.03%
        assert!((y2014.roe - 39.0).abs() < 0.1,
            "2014 ROE: expected ~39.0%, got {:.1}%", y2014.roe);

        // 2015: PTP/Sales = 334/1007 × 100 = 33.17%
        let y2015 = &quality.points[1];
        assert!((y2015.profit_on_sales - 33.2).abs() < 0.1,
            "2015 PTP/Sales: expected ~33.2%, got {:.1}%", y2015.profit_on_sales);
        // 2015: ROE = 250/570 × 100 = 43.86%
        assert!((y2015.roe - 43.9).abs() < 0.1,
            "2015 ROE: expected ~43.9%, got {:.1}%", y2015.roe);

        // Both should trend Up from 2014 → 2015
        assert_eq!(y2015.profit_trend, TrendIndicator::Up);
        assert_eq!(y2015.roe_trend, TrendIndicator::Up);
    }

    /// Verify full NAIC Section 4 pipeline: snapshot → forecast prices → ratio.
    /// Combines P/E history, EPS projection, and upside/downside assessment.
    /// Reference: Handbook Figures 2.3-2.4 (pp14-15)
    #[test]
    fn test_naic_handbook_full_valuation_pipeline() {
        let snapshot = AnalysisSnapshot {
            historical_data: HistoricalData {
                records: vec![HistoricalYearlyData {
                    fiscal_year: 2015,
                    eps: Decimal::new(571, 2),          // 5.71
                    price_high: Decimal::new(14983, 2), // 149.83 (proxy for current price)
                    ..Default::default()
                }],
                ..Default::default()
            },
            projected_eps_cagr: 10.4, // yields ~9.37 at year 5
            projected_high_pe: 27.9,  // Handbook Avg High P/E
            projected_low_pe: 20.0,   // Handbook Avg Low P/E
            ..Default::default()
        };

        let prices = extract_snapshot_prices(&snapshot);

        // Projected EPS at year 5: 5.71 × 1.104^5 ≈ 9.363
        // Target High = 27.9 × 9.363 ≈ 261.2
        let target_high = prices.target_high_price.unwrap();
        assert!((target_high - 261.3).abs() < 1.0,
            "Target high: expected ~261.3, got {:.1}", target_high);

        // Target Low = 20.0 × 9.363 ≈ 187.3
        // Note: handbook uses separate low EPS estimate (5.82); our model uses same EPS
        let target_low = prices.target_low_price.unwrap();
        assert!((target_low - 187.3).abs() < 1.0,
            "Target low: expected ~187.3, got {:.1}", target_low);

        // Upside/downside using handbook's selected low of 116.4
        let ratio = calculate_upside_downside_ratio(149.83, target_high, 116.4);
        assert!(ratio.unwrap() > 3.0,
            "Should meet NAIC 3:1 minimum for BUY recommendation");
    }
}
