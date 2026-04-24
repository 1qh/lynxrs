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
                    .add_column(
                        ColumnDef::new(FileShares::DownloadCount)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FileShares::Table)
                    .drop_column(FileShares::DownloadCount)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum FileShares {
    Table,
    DownloadCount,
}
