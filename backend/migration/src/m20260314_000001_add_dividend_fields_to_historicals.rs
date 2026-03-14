use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Alias::new("historicals"))
                .add_column(
                    ColumnDef::new(Alias::new("dividend_per_share"))
                        .decimal_len(19, 4)
                        .null(),
                )
                .add_column(
                    ColumnDef::new(Alias::new("shares_outstanding"))
                        .decimal_len(19, 4)
                        .null(),
                )
                .to_owned(),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        remove_column(m, "historicals", "dividend_per_share").await?;
        remove_column(m, "historicals", "shares_outstanding").await?;
        Ok(())
    }
}
