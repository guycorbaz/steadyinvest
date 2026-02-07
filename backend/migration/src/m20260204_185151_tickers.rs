use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        create_table(
            m,
            "tickers",
            &[
                ("id", ColType::PkAuto),
                ("ticker", ColType::StringUniq),
                ("name", ColType::String),
                ("exchange", ColType::String),
                ("currency", ColType::String),
            ],
            &[],
        )
        .await?;

        // Seed data
        let db = m.get_connection();
        db.execute_unprepared("
            INSERT INTO tickers (ticker, name, exchange, currency) VALUES
            ('NESN.SW', 'Nestle', 'SMI', 'CHF'),
            ('ROG.SW', 'Roche', 'SMI', 'CHF'),
            ('SAP.DE', 'SAP', 'DAX', 'EUR'),
            ('MBG.DE', 'Mercedes-Benz', 'DAX', 'EUR'),
            ('AAPL', 'Apple', 'NASDAQ', 'USD'),
            ('MSFT', 'Microsoft', 'NASDAQ', 'USD'),
            ('GOOGL', 'Alphabet', 'NASDAQ', 'USD'),
            ('AMZN', 'Amazon', 'NASDAQ', 'USD');
        ").await?;

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        drop_table(m, "tickers").await
    }
}
