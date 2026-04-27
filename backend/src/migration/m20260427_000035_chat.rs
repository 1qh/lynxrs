use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        let conn = manager.get_connection();
        // conversations: top-level chat thread, owned by a user (no shared
        // conversations yet — mirror the file_object ownership scope).
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "CREATE TABLE conversations ( \
               id UUID PRIMARY KEY, \
               owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE, \
               title TEXT NOT NULL DEFAULT '', \
               model TEXT NOT NULL DEFAULT 'claude-opus-4-7', \
               created_at TIMESTAMPTZ NOT NULL DEFAULT now(), \
               updated_at TIMESTAMPTZ NOT NULL DEFAULT now(), \
               archived_at TIMESTAMPTZ NULL \
             )"
            .to_string(),
        ))
        .await?;
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "CREATE INDEX idx_conversations_owner_updated \
             ON conversations(owner_id, updated_at DESC) \
             WHERE archived_at IS NULL"
                .to_string(),
        ))
        .await?;
        // messages: ordered by chain_seq under the conversation. Roles match
        // the OpenAI/Anthropic convention so we can replay verbatim into any
        // SDK without a shape conversion.
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "CREATE TABLE messages ( \
               id UUID PRIMARY KEY, \
               conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE, \
               role TEXT NOT NULL CHECK (role IN ('system','user','assistant','tool')), \
               content TEXT NOT NULL, \
               attachments JSONB NOT NULL DEFAULT '[]'::jsonb, \
               input_tokens INT NULL, \
               output_tokens INT NULL, \
               created_at TIMESTAMPTZ NOT NULL DEFAULT now(), \
               chain_seq BIGSERIAL \
             )"
            .to_string(),
        ))
        .await?;
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "CREATE INDEX idx_messages_conv_seq \
             ON messages(conversation_id, chain_seq)"
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
            "DROP TABLE IF EXISTS messages".to_string(),
        ))
        .await?;
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "DROP TABLE IF EXISTS conversations".to_string(),
        ))
        .await?;
        Ok(())
    }
}
