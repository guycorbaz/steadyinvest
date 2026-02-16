//! Shared metric extraction helpers for snapshot DTOs.
//!
//! Used by [`snapshots`](super::snapshots) and [`comparisons`](super::comparisons)
//! controllers to avoid duplicating JSON extraction logic.

use steady_invest_logic::{
    compute_upside_downside_from_snapshot, extract_snapshot_prices, AnalysisSnapshot,
};

/// Key projection metrics extracted from `snapshot_data` JSON.
#[derive(Debug)]
pub struct ProjectionMetrics {
    pub projected_sales_cagr: Option<f64>,
    pub projected_eps_cagr: Option<f64>,
    pub projected_high_pe: Option<f64>,
    pub projected_low_pe: Option<f64>,
}

/// Extract projection metrics from snapshot_data via lightweight `.get()` chains.
pub fn extract_projection_metrics(snapshot_data: &serde_json::Value) -> ProjectionMetrics {
    ProjectionMetrics {
        projected_sales_cagr: snapshot_data
            .get("projected_sales_cagr")
            .and_then(|v| v.as_f64()),
        projected_eps_cagr: snapshot_data
            .get("projected_eps_cagr")
            .and_then(|v| v.as_f64()),
        projected_high_pe: snapshot_data
            .get("projected_high_pe")
            .and_then(|v| v.as_f64()),
        projected_low_pe: snapshot_data
            .get("projected_low_pe")
            .and_then(|v| v.as_f64()),
    }
}

/// Monetary and derived fields extracted from snapshot JSON data.
///
/// Deserializes into [`AnalysisSnapshot`] and extracts native currency,
/// current price, target prices, and upside/downside ratio.
#[derive(Debug)]
pub struct MonetaryFields {
    pub native_currency: Option<String>,
    pub current_price: Option<f64>,
    pub target_high_price: Option<f64>,
    pub target_low_price: Option<f64>,
    pub upside_downside_ratio: Option<f64>,
}

/// Extract monetary fields by deserializing snapshot data via `steady-invest-logic`.
pub fn extract_monetary_fields(snapshot_data: &serde_json::Value) -> MonetaryFields {
    let snapshot: Option<AnalysisSnapshot> =
        serde_json::from_value(snapshot_data.clone()).ok();

    let Some(snapshot) = snapshot else {
        return MonetaryFields {
            native_currency: None,
            current_price: None,
            target_high_price: None,
            target_low_price: None,
            upside_downside_ratio: None,
        };
    };

    let native_currency = Some(snapshot.historical_data.currency.clone());
    let prices = extract_snapshot_prices(&snapshot);
    let upside_downside_ratio = compute_upside_downside_from_snapshot(&snapshot);

    MonetaryFields {
        native_currency,
        current_price: prices.current_price,
        target_high_price: prices.target_high_price,
        target_low_price: prices.target_low_price,
        upside_downside_ratio,
    }
}
