use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Webhooks::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Webhooks::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Webhooks::UserId).uuid().not_null())
                    .col(ColumnDef::new(Webhooks::Url).string().not_null())
                    .col(ColumnDef::new(Webhooks::Secret).string().not_null())
                    .col(
                        ColumnDef::new(Webhooks::Enabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Webhooks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_webhooks_user")
                            .from(Webhooks::Table, Webhooks::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_webhooks_user")
                    .table(Webhooks::Table)
                    .col(Webhooks::UserId)
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Webhooks::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
}

#[derive(Iden)]
enum Webhooks {
    Table,
    Id,
    UserId,
    Url,
    Secret,
    Enabled,
    CreatedAt,
}
