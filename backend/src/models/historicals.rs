//! Historical financial data model.
//!
//! Wraps the `historicals` entity for storing per-ticker, per-year financial
//! records (sales, EPS, price high/low, income, equity).

use sea_orm::entity::prelude::*;
pub use super::_entities::historicals::{self, ActiveModel, Column, Entity, Model};
/// Type alias for the historicals entity.
pub type Historicals = Entity;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, _insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        Ok(self)
    }
}

// implement your read-oriented logic here
impl Model {}

// implement your write-oriented logic here
impl ActiveModel {}

// implement your custom finders, selectors oriented logic here
impl Entity {}
