use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(Historicals::Table)
                .if_not_exists()
                .col(pk_auto(Historicals::Id))
                .col(string(Historicals::Ticker))
                .col(integer(Historicals::FiscalYear))
                .col(decimal_len(Historicals::Sales, 19, 4))
                .col(decimal_len(Historicals::Eps, 19, 4))
                .col(decimal_len(Historicals::PriceHigh, 19, 4))
                .col(decimal_len(Historicals::PriceLow, 19, 4))
                .col(string(Historicals::Currency))
                .to_owned(),
        )
        .await?;

        m.create_index(
            Index::create()
                .name("idx-historicals-ticker-year")
                .table(Historicals::Table)
                .col(Historicals::Ticker)
                .col(Historicals::FiscalYear)
                .unique()
                .to_owned(),
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(Historicals::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Historicals {
    Table,
    Id,
    Ticker,
    FiscalYear,
    Sales,
    Eps,
    PriceHigh,
    PriceLow,
    Currency,
}

