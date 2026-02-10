//! Ticker model — security registry and conversion helpers.

pub use super::_entities::tickers::{self, ActiveModel, Column, Entity, Model};
use loco_rs::prelude::*;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, _insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        Ok(self)
    }
}


impl Model {
    /// Converts the database model to the shared [`naic_logic::TickerInfo`] DTO.
    pub fn to_ticker_info(&self) -> naic_logic::TickerInfo {
        naic_logic::TickerInfo {
            ticker: self.ticker.clone(),
            name: self.name.clone(),
            exchange: self.exchange.clone(),
            currency: self.currency.clone(),
        }
    }
}
