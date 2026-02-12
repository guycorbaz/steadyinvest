use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Column identifiers for the new `analysis_snapshots` table.
#[derive(DeriveIden)]
enum AnalysisSnapshots {
    Table,
    Id,
    UserId,
    TickerId,
    SnapshotData,
    ThesisLocked,
    ChartImage,
    Notes,
    CapturedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Tickers {
    Table,
    Id,
}

/// Reference to the old table for data migration.
#[derive(DeriveIden)]
enum LockedAnalyses {
    Table,
    // Column identifiers for rollback recreation
    Id,
    TickerId,
    SnapshotData,
    AnalystNote,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // ── Step 1: Create analysis_snapshots table ──────────────────────
        m.create_table(
            Table::create()
                .table(AnalysisSnapshots::Table)
                .if_not_exists()
                .col(pk_auto(AnalysisSnapshots::Id))
                .col(integer(AnalysisSnapshots::UserId).default(1))
                .col(integer(AnalysisSnapshots::TickerId))
                .col(json_binary(AnalysisSnapshots::SnapshotData))
                .col(boolean(AnalysisSnapshots::ThesisLocked).default(false))
                .col(
                    ColumnDef::new(AnalysisSnapshots::ChartImage)
                        .custom(Alias::new("MEDIUMBLOB"))
                        .null()
                        .to_owned(),
                )
                .col(
                    ColumnDef::new(AnalysisSnapshots::Notes)
                        .text()
                        .null()
                        .to_owned(),
                )
                .col(timestamp_with_time_zone(AnalysisSnapshots::CapturedAt))
                // FK → users.id (no cascade — user deletion needs explicit handling)
                .foreign_key(
                    ForeignKey::create()
                        .name("fk-analysis_snapshots-user_id")
                        .from(AnalysisSnapshots::Table, AnalysisSnapshots::UserId)
                        .to(Users::Table, Users::Id),
                )
                // FK → tickers.id (cascade delete — ticker removal drops its snapshots)
                .foreign_key(
                    ForeignKey::create()
                        .name("fk-analysis_snapshots-ticker_id")
                        .from(AnalysisSnapshots::Table, AnalysisSnapshots::TickerId)
                        .to(Tickers::Table, Tickers::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;

        // ── Step 2: Create indexes for query performance (NFR6) ──────────
        m.create_index(
            Index::create()
                .name("idx-snapshots-user_id")
                .table(AnalysisSnapshots::Table)
                .col(AnalysisSnapshots::UserId)
                .to_owned(),
        )
        .await?;

        m.create_index(
            Index::create()
                .name("idx-snapshots-ticker_id")
                .table(AnalysisSnapshots::Table)
                .col(AnalysisSnapshots::TickerId)
                .to_owned(),
        )
        .await?;

        m.create_index(
            Index::create()
                .name("idx-snapshots-captured_at")
                .table(AnalysisSnapshots::Table)
                .col(AnalysisSnapshots::CapturedAt)
                .to_owned(),
        )
        .await?;

        // ── Step 3: Migrate data from locked_analyses ────────────────────
        //
        // Column mapping (actual locked_analyses columns → analysis_snapshots):
        //   locked_analyses.snapshot_data  → analysis_snapshots.snapshot_data  (same name)
        //   locked_analyses.created_at     → analysis_snapshots.captured_at    (renamed)
        //   locked_analyses.analyst_note   → analysis_snapshots.notes          (renamed)
        //   (new) user_id = 1              — default single-user; Phase 3 adds auth
        //   (new) thesis_locked = TRUE     — all existing rows were locked analyses
        //   (new) chart_image = NULL       — no images captured yet; Story 7.4 adds this
        //
        let db = m.get_connection();
        db.execute_unprepared(
            "INSERT INTO analysis_snapshots (user_id, ticker_id, snapshot_data, thesis_locked, chart_image, notes, captured_at)
             SELECT 1, ticker_id, snapshot_data, TRUE, NULL, analyst_note, created_at
             FROM locked_analyses",
        )
        .await?;

        // ── Step 4: Drop the old table ───────────────────────────────────
        m.drop_table(Table::drop().table(LockedAnalyses::Table).to_owned())
            .await?;

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Recreate locked_analyses for rollback
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
        .await?;

        // Reverse data migration: only copy locked rows back
        // (non-locked snapshots didn't exist in the old table)
        let db = m.get_connection();
        db.execute_unprepared(
            "INSERT INTO locked_analyses (ticker_id, snapshot_data, analyst_note, created_at)
             SELECT ticker_id, snapshot_data, COALESCE(notes, ''), captured_at
             FROM analysis_snapshots
             WHERE thesis_locked = TRUE",
        )
        .await?;

        // Drop analysis_snapshots
        m.drop_table(
            Table::drop()
                .table(AnalysisSnapshots::Table)
                .to_owned(),
        )
        .await?;

        Ok(())
    }
}
