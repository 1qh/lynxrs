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
                    .add_column(ColumnDef::new(FileObjects::OrgId).uuid().null())
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk_file_org")
                            .from_tbl(FileObjects::Table)
                            .from_col(FileObjects::OrgId)
                            .to_tbl(Orgs::Table)
                            .to_col(Orgs::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_file_org")
                    .table(FileObjects::Table)
                    .col(FileObjects::OrgId)
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FileObjects::Table)
                    .drop_column(FileObjects::OrgId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Orgs {
    Table,
    Id,
}
#[derive(Iden)]
enum FileObjects {
    Table,
    OrgId,
}
