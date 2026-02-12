//! Adds `deleted_at` nullable timestamp to `analysis_snapshots` for soft-delete support.
//!
//! Story 7.3 introduces soft-delete: unlocked snapshots can be marked as deleted
//! (setting `deleted_at`) rather than hard-deleted.  All queries filter on
//! `deleted_at IS NULL` by default.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum AnalysisSnapshots {
    Table,
    DeletedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Add nullable deleted_at column
        m.alter_table(
            Table::alter()
                .table(AnalysisSnapshots::Table)
                .add_column(
                    ColumnDef::new(AnalysisSnapshots::DeletedAt)
                        .timestamp_with_time_zone()
                        .null(),
                )
                .to_owned(),
        )
        .await?;

        // Index for filtering active (non-deleted) records
        m.create_index(
            Index::create()
                .name("idx-snapshots-deleted_at")
                .table(AnalysisSnapshots::Table)
                .col(AnalysisSnapshots::DeletedAt)
                .to_owned(),
        )
        .await?;

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_index(
            Index::drop()
                .name("idx-snapshots-deleted_at")
                .table(AnalysisSnapshots::Table)
                .to_owned(),
        )
        .await?;

        m.alter_table(
            Table::alter()
                .table(AnalysisSnapshots::Table)
                .drop_column(AnalysisSnapshots::DeletedAt)
                .to_owned(),
        )
        .await?;

        Ok(())
    }
}
