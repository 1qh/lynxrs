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
                    .add_column(ColumnDef::new(FileObjects::Sha256).string().null())
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FileObjects::Table)
                    .drop_column(FileObjects::Sha256)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum FileObjects { Table, Sha256 }
