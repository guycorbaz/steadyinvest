use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(HistoricalsOverrides::Table)
                .if_not_exists()
                .col(pk_auto(HistoricalsOverrides::Id))
                .col(string(HistoricalsOverrides::Ticker))
                .col(integer(HistoricalsOverrides::FiscalYear))
                .col(string(HistoricalsOverrides::FieldName))
                .col(decimal_len(HistoricalsOverrides::Value, 19, 4))
                .col(string_null(HistoricalsOverrides::Note))
                .to_owned(),
        )
        .await?;

        m.create_index(
            Index::create()
                .name("idx-overrides-lookup")
                .table(HistoricalsOverrides::Table)
                .col(HistoricalsOverrides::Ticker)
                .col(HistoricalsOverrides::FiscalYear)
                .col(HistoricalsOverrides::FieldName)
                .unique()
                .to_owned(),
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(HistoricalsOverrides::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum HistoricalsOverrides {
    Table,
    Id,
    Ticker,
    FiscalYear,
    FieldName,
    Value,
    Note,
}
