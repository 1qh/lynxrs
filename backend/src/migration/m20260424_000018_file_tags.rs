use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        manager
            .get_connection()
            .execute(sea_orm::Statement::from_string(
                backend,
                "ALTER TABLE file_objects ADD COLUMN tags text[] NOT NULL DEFAULT '{}'".to_string(),
            ))
            .await?;
        manager
            .get_connection()
            .execute(sea_orm::Statement::from_string(
                backend,
                "CREATE INDEX idx_file_objects_tags ON file_objects USING GIN(tags)".to_string(),
            ))
            .await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        manager
            .get_connection()
            .execute(sea_orm::Statement::from_string(
                backend,
                "DROP INDEX IF EXISTS idx_file_objects_tags".to_string(),
            ))
            .await?;
        manager
            .get_connection()
            .execute(sea_orm::Statement::from_string(
                backend,
                "ALTER TABLE file_objects DROP COLUMN tags".to_string(),
            ))
            .await?;
        Ok(())
    }
}
