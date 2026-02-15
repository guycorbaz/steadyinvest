#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;
mod m20220101_000001_users;

mod m20260204_185151_tickers;
mod m20260204_195545_historicals;
mod m20260207_000158_add_adjustment_metadata_to_historicals;
mod m20260207_001419_exchange_rates;
mod m20260207_101051_add_quality_fields_to_historicals;
mod m20260207_181500_historicals_overrides;
mod m20260207_191500_locked_analyses;
mod m20260208_114000_provider_rate_limits;
mod m20260208_120000_audit_logs;
mod m20260212_000001_analysis_snapshots;
mod m20260212_000002_add_snapshot_deleted_at;
mod m20260215_000001_seed_default_user;
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
            Box::new(m20260207_101051_add_quality_fields_to_historicals::Migration),
            Box::new(m20260207_181500_historicals_overrides::Migration),
            Box::new(m20260207_191500_locked_analyses::Migration),
            Box::new(m20260208_114000_provider_rate_limits::Migration),
            Box::new(m20260208_120000_audit_logs::Migration),
            Box::new(m20260212_000001_analysis_snapshots::Migration),
            Box::new(m20260212_000002_add_snapshot_deleted_at::Migration),
            Box::new(m20260215_000001_seed_default_user::Migration),
            // inject-above (do not remove this comment)
        ]
    }
}