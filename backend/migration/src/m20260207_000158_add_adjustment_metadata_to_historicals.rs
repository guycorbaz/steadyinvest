use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        add_column(m, "historicals", "is_split_adjusted", ColType::BooleanNull).await?;
        add_column(m, "historicals", "adjustment_factor", ColType::DecimalNull).await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        remove_column(m, "historicals", "is_split_adjusted").await?;
        remove_column(m, "historicals", "adjustment_factor").await?;
        Ok(())
    }
}
