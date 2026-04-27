use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        let conn = manager.get_connection();
        // Per-conversation knobs the user can tune from the UI.
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "ALTER TABLE conversations \
             ADD COLUMN system_prompt TEXT NOT NULL DEFAULT '', \
             ADD COLUMN temperature REAL NOT NULL DEFAULT 0.7, \
             ADD COLUMN share_token_hash TEXT NULL"
                .to_string(),
        ))
        .await?;
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "CREATE UNIQUE INDEX idx_conversations_share_token_hash \
             ON conversations(share_token_hash) \
             WHERE share_token_hash IS NOT NULL"
                .to_string(),
        ))
        .await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        let conn = manager.get_connection();
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "DROP INDEX IF EXISTS idx_conversations_share_token_hash".to_string(),
        ))
        .await?;
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "ALTER TABLE conversations \
             DROP COLUMN IF EXISTS system_prompt, \
             DROP COLUMN IF EXISTS temperature, \
             DROP COLUMN IF EXISTS share_token_hash"
                .to_string(),
        ))
        .await?;
        Ok(())
    }
}
