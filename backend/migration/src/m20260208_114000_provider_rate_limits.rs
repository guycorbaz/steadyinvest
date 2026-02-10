use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProviderRateLimits::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProviderRateLimits::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ProviderRateLimits::Name).string().not_null().unique_key())
                    .col(ColumnDef::new(ProviderRateLimits::QuotaConsumed).integer().not_null().default(0))
                    .col(ColumnDef::new(ProviderRateLimits::LastUpdated).timestamp().not_null().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ProviderRateLimits::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ProviderRateLimits {
    Table,
    Id,
    Name,
    QuotaConsumed,
    LastUpdated,
}
