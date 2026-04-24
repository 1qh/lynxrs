use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FileObjects::Table)
                    .add_column(
                        ColumnDef::new(FileObjects::DeletedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_file_objects_owner_deleted_created")
                    .table(FileObjects::Table)
                    .col(FileObjects::OwnerId)
                    .col(FileObjects::DeletedAt)
                    .col(FileObjects::CreatedAt)
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FileObjects::Table)
                    .drop_column(FileObjects::DeletedAt)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum FileObjects { Table, OwnerId, CreatedAt, DeletedAt }
