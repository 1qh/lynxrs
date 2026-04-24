use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FileShares::Table)
                    .add_column(ColumnDef::new(FileShares::PasswordHash).string().null())
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FileShares::Table)
                    .drop_column(FileShares::PasswordHash)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum FileShares {
    Table,
    PasswordHash,
}
