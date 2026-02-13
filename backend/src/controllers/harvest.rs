//! Data harvesting controller.
//!
//! Exposes `POST /api/harvest/{ticker}` to trigger a 10-year historical data
//! fetch, split adjustment, and P/E analysis for the given ticker.

use loco_rs::prelude::*;
use axum::extract::Path;
use crate::services::harvest;

/// Triggers a full 10-year data harvest for the given ticker symbol.
///
/// **POST** `/api/harvest/{ticker}`
///
/// Validates the ticker format, delegates to [`harvest::run_harvest`], and
/// returns the assembled [`steady_invest_logic::HistoricalData`] as JSON.
///
/// # Errors
///
/// Returns `400 Bad Request` if the ticker is empty or longer than 10 characters.
#[debug_handler]
pub async fn harvest_ticker(
    State(ctx): State<AppContext>,
    Path(ticker): Path<String>,
) -> Result<Response> {
    // Basic validation
    if ticker.is_empty() || ticker.len() > 10 {
        return Err(Error::BadRequest("Invalid ticker format".to_string()));
    }
    let ticker = ticker.to_uppercase();
    
    let data = harvest::run_harvest(&ctx, &ticker).await?;
    format::json(data)
}

/// Registers harvest routes under `/api/harvest`.
pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/harvest")
        .add("/{ticker}", post(harvest_ticker))
}
