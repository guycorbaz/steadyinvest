//! Ticker search and autocomplete controller.
//!
//! Exposes `GET /api/tickers/search?q=…` for fuzzy-matching tickers by
//! symbol, company name, or exchange.

use crate::models::tickers;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;

/// Query parameters for the ticker search endpoint.
#[derive(Deserialize)]
pub struct SearchParams {
    /// The search query string (matched against ticker, name, and exchange).
    pub q: String,
}

/// Searches tickers by symbol, name, or exchange using a LIKE query.
///
/// **GET** `/api/tickers/search?q={query}`
///
/// Returns a JSON array of [`steady_invest_logic::TickerInfo`] matches.
///
/// # Errors
///
/// Returns a database error if the query execution fails.
#[debug_handler]
pub async fn search(
    State(ctx): State<AppContext>,
    Query(params): Query<SearchParams>,
) -> Result<Response> {
    let query = format!("%{}%", params.q);
    let tickers = tickers::Entity::find()
        .filter(
            sea_orm::Condition::any()
                .add(tickers::Column::Ticker.like(&query))
                .add(tickers::Column::Name.like(&query))
                .add(tickers::Column::Exchange.like(&query)),
        )
        .all(&ctx.db)
        .await?;

    let results: Vec<steady_invest_logic::TickerInfo> =
        tickers.into_iter().map(|t| t.to_ticker_info()).collect();

    format::json(results)
}

/// Registers ticker routes under `/api/tickers`.
pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/tickers")
        .add("/search", get(search))
}
