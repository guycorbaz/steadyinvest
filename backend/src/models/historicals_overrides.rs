//! Manual data override model — analyst corrections to historical records.

use sea_orm::entity::prelude::*;
pub use super::_entities::historicals_overrides::{self, ActiveModel, Column, Entity, Model};
/// Type alias for the historicals_overrides entity.
pub type HistoricalsOverrides = Entity;

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}

// implement your read-oriented logic here
impl Model {}

// implement your write-oriented logic here
impl ActiveModel {}

// implement your custom finders, selectors oriented logic here
impl Entity {}
