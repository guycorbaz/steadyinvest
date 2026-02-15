use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum ComparisonSets {
    Table,
    Id,
    UserId,
    Name,
    BaseCurrency,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ComparisonSetItems {
    Table,
    Id,
    ComparisonSetId,
    AnalysisSnapshotId,
    SortOrder,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum AnalysisSnapshots {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // ── Step 1: Create comparison_sets table ─────────────────────────
        m.create_table(
            Table::create()
                .table(ComparisonSets::Table)
                .if_not_exists()
                .col(pk_auto(ComparisonSets::Id))
                .col(integer(ComparisonSets::UserId).default(1))
                .col(string(ComparisonSets::Name).not_null())
                .col(
                    ColumnDef::new(ComparisonSets::BaseCurrency)
                        .string_len(3)
                        .not_null()
                        .to_owned(),
                )
                .col(timestamp_with_time_zone(ComparisonSets::CreatedAt))
                .col(timestamp_with_time_zone(ComparisonSets::UpdatedAt))
                // FK → users.id
                .foreign_key(
                    ForeignKey::create()
                        .name("fk-comparison_sets-user_id")
                        .from(ComparisonSets::Table, ComparisonSets::UserId)
                        .to(Users::Table, Users::Id),
                )
                .to_owned(),
        )
        .await?;

        // Index on user_id for query performance
        m.create_index(
            Index::create()
                .name("idx-comparison_sets-user_id")
                .table(ComparisonSets::Table)
                .col(ComparisonSets::UserId)
                .to_owned(),
        )
        .await?;

        // ── Step 2: Create comparison_set_items table ────────────────────
        m.create_table(
            Table::create()
                .table(ComparisonSetItems::Table)
                .if_not_exists()
                .col(pk_auto(ComparisonSetItems::Id))
                .col(integer(ComparisonSetItems::ComparisonSetId).not_null())
                .col(integer(ComparisonSetItems::AnalysisSnapshotId).not_null())
                .col(integer(ComparisonSetItems::SortOrder).not_null())
                // FK → comparison_sets.id (CASCADE delete — removing a set removes its items)
                .foreign_key(
                    ForeignKey::create()
                        .name("fk-comparison_set_items-set_id")
                        .from(
                            ComparisonSetItems::Table,
                            ComparisonSetItems::ComparisonSetId,
                        )
                        .to(ComparisonSets::Table, ComparisonSets::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                // FK → analysis_snapshots.id (RESTRICT delete — cannot remove referenced snapshots)
                .foreign_key(
                    ForeignKey::create()
                        .name("fk-comparison_set_items-snapshot_id")
                        .from(
                            ComparisonSetItems::Table,
                            ComparisonSetItems::AnalysisSnapshotId,
                        )
                        .to(AnalysisSnapshots::Table, AnalysisSnapshots::Id)
                        .on_delete(ForeignKeyAction::Restrict),
                )
                .to_owned(),
        )
        .await?;

        // Index on comparison_set_id for join performance
        m.create_index(
            Index::create()
                .name("idx-comparison_set_items-set_id")
                .table(ComparisonSetItems::Table)
                .col(ComparisonSetItems::ComparisonSetId)
                .to_owned(),
        )
        .await?;

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Drop items first (FK dependency)
        m.drop_table(
            Table::drop()
                .table(ComparisonSetItems::Table)
                .to_owned(),
        )
        .await?;

        m.drop_table(
            Table::drop()
                .table(ComparisonSets::Table)
                .to_owned(),
        )
        .await?;

        Ok(())
    }
}
