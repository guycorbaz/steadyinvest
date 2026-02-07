use crate::models::tickers;
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: String,
}

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

    let results: Vec<naic_logic::TickerInfo> =
        tickers.into_iter().map(|t| t.to_ticker_info()).collect();

    format::json(results)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/tickers")
        .add("/search", get(search))
}
