use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FileStars::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(FileStars::UserId).uuid().not_null())
                    .col(ColumnDef::new(FileStars::FileId).uuid().not_null())
                    .col(
                        ColumnDef::new(FileStars::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(FileStars::UserId)
                            .col(FileStars::FileId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_fs_file")
                            .from(FileStars::Table, FileStars::FileId)
                            .to(FileObjects::Table, FileObjects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_fs_user")
                            .from(FileStars::Table, FileStars::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FileStars::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Users { Table, Id }
#[derive(Iden)]
enum FileObjects { Table, Id }
#[derive(Iden)]
enum FileStars { Table, UserId, FileId, CreatedAt }
