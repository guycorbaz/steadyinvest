#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;
mod m20220101_000001_users;

mod m20260204_185151_tickers;
mod m20260204_195545_historicals;
mod m20260207_000158_add_adjustment_metadata_to_historicals;
mod m20260207_001419_exchange_rates;
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_users::Migration),
            Box::new(m20260204_185151_tickers::Migration),
            Box::new(m20260204_195545_historicals::Migration),
            Box::new(m20260207_000158_add_adjustment_metadata_to_historicals::Migration),
            Box::new(m20260207_001419_exchange_rates::Migration),
            // inject-above (do not remove this comment)
        ]
    }
}