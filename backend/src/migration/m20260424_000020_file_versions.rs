use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FileVersions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FileVersions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(FileVersions::FileId).uuid().not_null())
                    .col(ColumnDef::new(FileVersions::VersionNo).integer().not_null())
                    .col(ColumnDef::new(FileVersions::StorageKey).string().not_null())
                    .col(ColumnDef::new(FileVersions::Filename).string().not_null())
                    .col(
                        ColumnDef::new(FileVersions::ContentType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FileVersions::SizeBytes)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(FileVersions::Sha256).string().null())
                    .col(
                        ColumnDef::new(FileVersions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_file_versions_file")
                            .from(FileVersions::Table, FileVersions::FileId)
                            .to(FileObjects::Table, FileObjects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_file_versions_file_version")
                    .table(FileVersions::Table)
                    .col(FileVersions::FileId)
                    .col(FileVersions::VersionNo)
                    .unique()
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FileVersions::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum FileObjects {
    Table,
    Id,
}

#[derive(Iden)]
enum FileVersions {
    Table,
    Id,
    FileId,
    VersionNo,
    StorageKey,
    Filename,
    ContentType,
    SizeBytes,
    Sha256,
    CreatedAt,
}
