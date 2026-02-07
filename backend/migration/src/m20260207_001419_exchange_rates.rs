use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(ExchangeRates::Table)
                .if_not_exists()
                .col(pk_auto(ExchangeRates::Id))
                .col(string(ExchangeRates::FromCurrency))
                .col(string(ExchangeRates::ToCurrency))
                .col(integer(ExchangeRates::FiscalYear))
                .col(decimal_len(ExchangeRates::Rate, 19, 4))
                .to_owned(),
        )
        .await?;

        m.create_index(
            Index::create()
                .name("idx-ex-rates-from-to-year")
                .table(ExchangeRates::Table)
                .col(ExchangeRates::FromCurrency)
                .col(ExchangeRates::ToCurrency)
                .col(ExchangeRates::FiscalYear)
                .unique()
                .to_owned(),
        )
        .await?;

        // Seed some historical data for testing
        let db = m.get_connection();
        db.execute_unprepared("
            INSERT INTO exchange_rates (from_currency, to_currency, fiscal_year, rate) VALUES
            ('CHF', 'USD', 2025, 1.15), ('CHF', 'USD', 2024, 1.12), ('CHF', 'USD', 2023, 1.10),
            ('CHF', 'USD', 2022, 1.05), ('CHF', 'USD', 2021, 1.09), ('CHF', 'USD', 2020, 1.07),
            ('CHF', 'USD', 2019, 1.01), ('CHF', 'USD', 2018, 1.02), ('CHF', 'USD', 2017, 1.01),
            ('CHF', 'USD', 2016, 1.01),
            ('EUR', 'USD', 2025, 1.08), ('EUR', 'USD', 2024, 1.09), ('EUR', 'USD', 2023, 1.08),
            ('EUR', 'USD', 2022, 1.05), ('EUR', 'USD', 2021, 1.18), ('EUR', 'USD', 2020, 1.14),
            ('EUR', 'USD', 2019, 1.12), ('EUR', 'USD', 2018, 1.18), ('EUR', 'USD', 2017, 1.13),
            ('EUR', 'USD', 2016, 1.11);
        ").await?;

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(ExchangeRates::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ExchangeRates {
    Table,
    Id,
    FromCurrency,
    ToCurrency,
    FiscalYear,
    Rate,
}

