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
                .add_column(ColumnDef::new(Alias::new("net_income")).decimal_len(19, 4).null())
                .add_column(ColumnDef::new(Alias::new("pretax_income")).decimal_len(19, 4).null())
                .add_column(ColumnDef::new(Alias::new("total_equity")).decimal_len(19, 4).null())
                .to_owned(),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        remove_column(m, "historicals", "net_income").await?;
        remove_column(m, "historicals", "pretax_income").await?;
        remove_column(m, "historicals", "total_equity").await?;
        Ok(())
    }
}
