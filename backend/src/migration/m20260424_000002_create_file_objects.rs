use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FileObjects::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FileObjects::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(FileObjects::OwnerId).uuid().not_null())
                    .col(ColumnDef::new(FileObjects::StorageKey).string().not_null())
                    .col(ColumnDef::new(FileObjects::Filename).string().not_null())
                    .col(ColumnDef::new(FileObjects::ContentType).string().not_null())
                    .col(
                        ColumnDef::new(FileObjects::SizeBytes)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FileObjects::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_file_objects_owner")
                            .from(FileObjects::Table, FileObjects::OwnerId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_file_objects_owner")
                    .table(FileObjects::Table)
                    .col(FileObjects::OwnerId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FileObjects::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum FileObjects {
    Table,
    Id,
    OwnerId,
    StorageKey,
    Filename,
    ContentType,
    SizeBytes,
    CreatedAt,
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
}
