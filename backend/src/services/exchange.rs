use loco_rs::prelude::*;
use crate::models::exchange_rates;
use rust_decimal::Decimal;

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
