use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Generated tsvector over filename + description. STORED + GIN index so
        // queries are O(log N), not O(N) substring scan in Rust.
        let backend = manager.get_database_backend();
        let conn = manager.get_connection();
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "ALTER TABLE file_objects \
             ADD COLUMN search_tsv tsvector \
             GENERATED ALWAYS AS ( \
                 to_tsvector('simple', \
                     coalesce(filename, '') || ' ' || coalesce(description, '')) \
             ) STORED"
                .to_string(),
        ))
        .await?;
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "CREATE INDEX idx_file_objects_search_tsv \
             ON file_objects USING GIN (search_tsv)"
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
            "DROP INDEX IF EXISTS idx_file_objects_search_tsv".to_string(),
        ))
        .await?;
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "ALTER TABLE file_objects DROP COLUMN IF EXISTS search_tsv".to_string(),
        ))
        .await?;
        Ok(())
    }
}
