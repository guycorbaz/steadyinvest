use loco_rs::prelude::*;
use axum::extract::Path;
use crate::services::harvest;

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

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/harvest")
        .add("/{ticker}", post(harvest_ticker))
}
