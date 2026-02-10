//! Currency exchange rate service.
//!
//! Provides cached exchange rate lookups from the database. Used during data
//! harvesting to normalize foreign-currency financials to the user's display currency.

use loco_rs::prelude::*;
use crate::models::exchange_rates;
use rust_decimal::Decimal;

/// Looks up the exchange rate between two currencies for a given fiscal year.
///
/// Returns `Ok(Some(Decimal::ONE))` when `from == to` (no conversion needed),
/// `Ok(None)` when no rate is cached for the requested pair and year.
///
/// # Errors
///
/// Returns a database error if the query fails.
pub async fn get_rate(
    db: &DatabaseConnection,
    from: &str,
    to: &str,
    year: i32,
) -> Result<Option<Decimal>> {
    if from == to {
        return Ok(Some(Decimal::ONE));
    }

    let rate = exchange_rates::Entity::find()
        .filter(exchange_rates::Column::FromCurrency.eq(from))
        .filter(exchange_rates::Column::ToCurrency.eq(to))
        .filter(exchange_rates::Column::FiscalYear.eq(year))
        .one(db)
        .await?;

    Ok(rate.map(|r| r.rate))
}
