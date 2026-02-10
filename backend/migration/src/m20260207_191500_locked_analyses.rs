use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(LockedAnalyses::Table)
                .if_not_exists()
                .col(pk_auto(LockedAnalyses::Id))
                .col(integer(LockedAnalyses::TickerId))
                .col(json_binary(LockedAnalyses::SnapshotData))
                .col(string(LockedAnalyses::AnalystNote))
                .col(timestamp_with_time_zone(LockedAnalyses::CreatedAt))
                .foreign_key(
                    ForeignKey::create()
                        .name("fk-locked_analyses-ticker_id")
                        .from(LockedAnalyses::Table, LockedAnalyses::TickerId)
                        .to(Tickers::Table, Tickers::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(LockedAnalyses::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum LockedAnalyses {
    Table,
    Id,
    TickerId,
    SnapshotData,
    AnalystNote,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Tickers {
    Table,
    Id,
}
