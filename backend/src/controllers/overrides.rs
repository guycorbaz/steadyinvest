//! Manual data override controller.
//!
//! Provides CRUD endpoints for analyst overrides of historical financial data.
//! An audit note is required on every override to maintain a review trail.

use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};
use crate::models::historicals_overrides;

/// Request body for creating or updating a manual data override.
#[derive(Debug, Deserialize, Serialize)]
pub struct OverrideRequest {
    /// Ticker symbol the override applies to.
    pub ticker: String,
    /// Fiscal year of the record being overridden.
    pub fiscal_year: i32,
    /// The field being overridden (e.g., `"eps"`, `"sales"`).
    pub field_name: String,
    /// The replacement value.
    pub value: rust_decimal::Decimal,
    /// Audit note explaining the reason (required, must not be blank).
    pub note: Option<String>,
}

/// Creates or updates a manual data override.
///
/// **POST** `/api/overrides/`
///
/// Upserts by `(ticker, fiscal_year, field_name)` composite key.
///
/// # Errors
///
/// Returns an error if the audit note is missing or blank.
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

/// Deletes a manual data override by ticker, year, and field.
///
/// **DELETE** `/api/overrides/{ticker}/{year}/{field}`
///
/// # Errors
///
/// Returns `404 Not Found` if no matching override exists.
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

/// Registers override routes under `/api/overrides`.
pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/overrides")
        .add("/", post(save_override))
        .add("/{ticker}/{year}/{field}", delete(delete_override))
}
