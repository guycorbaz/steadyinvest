//! Current exchange rate endpoint.
//!
//! Serves live EUR/CHF/USD rates via the Frankfurter API with in-memory
//! caching and database fallback. Public endpoint — no authentication
//! required (per architecture Story 10.3 AC).

use loco_rs::prelude::*;
use crate::services::exchange_rate_provider;

/// Returns current exchange rates for EUR, CHF, and USD pairs.
///
/// **GET** `/api/v1/exchange-rates`
///
/// Response includes a `stale` flag and `rates_as_of` timestamp.
/// Returns 503 only when no data source is available.
#[debug_handler]
pub async fn get_exchange_rates(
    State(ctx): State<AppContext>,
) -> Result<Response> {
    match exchange_rate_provider::get_rates(&ctx.db).await {
        Ok(response) => {
            let json = serde_json::to_string(&response)
                .map_err(|e| Error::string(&e.to_string()))?;
            Ok(Response::builder()
                .status(axum::http::StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Cache-Control", "public, max-age=300")
                .body(json.into())
                .map_err(|e| Error::string(&e.to_string()))?)
        }
        Err(_) => Ok(Response::builder()
            .status(axum::http::StatusCode::SERVICE_UNAVAILABLE)
            .header("Content-Type", "application/json")
            .body(
                serde_json::json!({
                    "error": "Exchange rate data is temporarily unavailable"
                })
                .to_string()
                .into(),
            )
            .map_err(|e| Error::string(&e.to_string()))?),
    }
}

/// Registers exchange rate routes under `/api/v1/exchange-rates`.
pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/v1/exchange-rates")
        .add("/", get(get_exchange_rates))
}
