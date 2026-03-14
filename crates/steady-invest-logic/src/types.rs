use serde::{Deserialize, Serialize};

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
    /// Annual dividend per share (used for NAIC yield and payout ratio calculations).
    pub dividend_per_share: Option<rust_decimal::Decimal>,
    /// Total shares outstanding (used for NAIC total return and per-share metrics).
    pub shares_outstanding: Option<rust_decimal::Decimal>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_historical_yearly_data_defaults_dividend_fields() {
        let data = HistoricalYearlyData::default();
        assert_eq!(data.dividend_per_share, None);
        assert_eq!(data.shares_outstanding, None);
    }

    #[test]
    fn test_deserialization_without_dividend_fields() {
        // Legacy JSON without dividend fields should deserialize with None values
        let json = r#"{
            "fiscal_year": 2023,
            "sales": "1000",
            "eps": "5.50",
            "price_high": "200",
            "price_low": "150",
            "net_income": "100",
            "pretax_income": "120",
            "total_equity": "1000",
            "adjustment_factor": "1",
            "exchange_rate": null,
            "overrides": []
        }"#;
        let data: HistoricalYearlyData = serde_json::from_str(json).unwrap();
        assert_eq!(data.fiscal_year, 2023);
        assert_eq!(data.dividend_per_share, None);
        assert_eq!(data.shares_outstanding, None);
    }

    #[test]
    fn test_serialization_with_dividend_data() {
        let data = HistoricalYearlyData {
            fiscal_year: 2023,
            sales: rust_decimal::Decimal::from(1000),
            eps: rust_decimal::Decimal::new(550, 2),
            price_high: rust_decimal::Decimal::from(200),
            price_low: rust_decimal::Decimal::from(150),
            dividend_per_share: Some(rust_decimal::Decimal::new(125, 2)), // 1.25
            shares_outstanding: Some(rust_decimal::Decimal::from(1_000_000)),
            ..Default::default()
        };

        // Round-trip: serialize then deserialize
        let json = serde_json::to_string(&data).unwrap();
        let roundtrip: HistoricalYearlyData = serde_json::from_str(&json).unwrap();
        assert_eq!(
            roundtrip.dividend_per_share,
            Some(rust_decimal::Decimal::new(125, 2))
        );
        assert_eq!(
            roundtrip.shares_outstanding,
            Some(rust_decimal::Decimal::from(1_000_000))
        );
        assert_eq!(roundtrip, data);
    }
}
