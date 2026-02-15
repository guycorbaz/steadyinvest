use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Seed a default user (id=1) required by analysis_snapshots FK constraint.
        // Until Epic 10 (Multi-User Authentication), all snapshots use user_id=1.
        // Uses INSERT IGNORE to be idempotent (no-op if user already exists).
        let db = m.get_connection();
        db.execute_unprepared("
            INSERT IGNORE INTO users (id, pid, email, password, api_key, name, created_at, updated_at)
            VALUES (
                1,
                '11111111-1111-1111-1111-111111111111',
                'default@steadyinvest.local',
                '$argon2id$v=19$m=19456,t=2,p=1$ETQBx4rTgNAZhSaeYZKOZg$eYTdH26CRT6nUJtacLDEboP0li6xUwUF/q5nSlQ8uuc',
                'lo-00000000-0000-0000-0000-000000000000',
                'Default User',
                NOW(),
                NOW()
            );
        ").await?;

        Ok(())
    }

    async fn down(&self, _m: &SchemaManager) -> Result<(), DbErr> {
        // Do not delete the default user on rollback — snapshots may reference it.
        Ok(())
    }
}
