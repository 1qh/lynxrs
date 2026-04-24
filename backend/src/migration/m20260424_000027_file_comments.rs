use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FileComments::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(FileComments::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(FileComments::FileId).uuid().not_null())
                    .col(ColumnDef::new(FileComments::UserId).uuid().not_null())
                    .col(ColumnDef::new(FileComments::Body).text().not_null())
                    .col(
                        ColumnDef::new(FileComments::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_fc_file")
                            .from(FileComments::Table, FileComments::FileId)
                            .to(FileObjects::Table, FileObjects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_fc_user")
                            .from(FileComments::Table, FileComments::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_fc_file_created")
                    .table(FileComments::Table)
                    .col(FileComments::FileId)
                    .col(FileComments::CreatedAt)
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FileComments::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum FileObjects { Table, Id }
#[derive(Iden)]
enum Users { Table, Id }
#[derive(Iden)]
enum FileComments { Table, Id, FileId, UserId, Body, CreatedAt }
