use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuditLogs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuditLogs::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AuditLogs::CreatedAt)
                            .date_time()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(AuditLogs::Ticker).string().not_null())
                    .col(ColumnDef::new(AuditLogs::Exchange).string().not_null())
                    .col(ColumnDef::new(AuditLogs::FieldName).string().not_null())
                    .col(ColumnDef::new(AuditLogs::OldValue).string())
                    .col(ColumnDef::new(AuditLogs::NewValue).string())
                    .col(ColumnDef::new(AuditLogs::EventType).string().not_null())
                    .col(ColumnDef::new(AuditLogs::Source).string().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuditLogs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AuditLogs {
    Table,
    Id,
    CreatedAt,
    Ticker,
    Exchange,
    FieldName,
    OldValue,
    NewValue,
    EventType,
    Source,
}
