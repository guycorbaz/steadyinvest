use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};
use crate::models::historicals_overrides;

#[derive(Debug, Deserialize, Serialize)]
pub struct OverrideRequest {
    pub ticker: String,
    pub fiscal_year: i32,
    pub field_name: String,
    pub value: rust_decimal::Decimal,
    pub note: Option<String>,
}

#[debug_handler]
pub async fn save_override(
    State(ctx): State<AppContext>,
    Json(req): Json<OverrideRequest>,
) -> Result<Response> {
    if req.note.as_ref().map(|n| n.trim().is_empty()).unwrap_or(true) {
        return Err(Error::string("Audit note is required (AC 3)"));
    }
    let ticker = req.ticker.to_uppercase();
    
    // Find existing or create new
    let existing = historicals_overrides::Entity::find()
        .filter(historicals_overrides::Column::Ticker.eq(&ticker))
        .filter(historicals_overrides::Column::FiscalYear.eq(req.fiscal_year))
        .filter(historicals_overrides::Column::FieldName.eq(&req.field_name))
        .one(&ctx.db)
        .await?;

    let is_existing = existing.is_some();
    let mut active: historicals_overrides::ActiveModel = match existing {
        Some(m) => m.into(),
        None => historicals_overrides::ActiveModel {
            ticker: ActiveValue::set(ticker),
            fiscal_year: ActiveValue::set(req.fiscal_year),
            field_name: ActiveValue::set(req.field_name),
            ..Default::default()
        },
    };

    active.value = ActiveValue::set(req.value);
    active.note = ActiveValue::set(req.note);

    let model = if is_existing {
        active.update(&ctx.db).await?
    } else {
        active.insert(&ctx.db).await?
    };

    format::json(model)
}

#[debug_handler]
pub async fn delete_override(
    State(ctx): State<AppContext>,
    Path((ticker, year, field)): Path<(String, i32, String)>,
) -> Result<Response> {
    let ticker = ticker.to_uppercase();
    
    let existing = historicals_overrides::Entity::find()
        .filter(historicals_overrides::Column::Ticker.eq(&ticker))
        .filter(historicals_overrides::Column::FiscalYear.eq(year))
        .filter(historicals_overrides::Column::FieldName.eq(&field))
        .one(&ctx.db)
        .await?;

    if let Some(m) = existing {
        m.delete(&ctx.db).await?;
        return format::json(serde_json::json!({ "status": "deleted" }));
    }

    Err(Error::NotFound)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/overrides")
        .add("/", post(save_override))
        .add("/{ticker}/{year}/{field}", delete(delete_override))
}
