use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuditEvents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuditEvents::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AuditEvents::UserId).uuid().null())
                    .col(ColumnDef::new(AuditEvents::Action).string().not_null())
                    .col(ColumnDef::new(AuditEvents::Ip).string().null())
                    .col(ColumnDef::new(AuditEvents::UserAgent).string().null())
                    .col(
                        ColumnDef::new(AuditEvents::Meta)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(AuditEvents::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_audit_user_created")
                    .table(AuditEvents::Table)
                    .col(AuditEvents::UserId)
                    .col(AuditEvents::CreatedAt)
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuditEvents::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum AuditEvents {
    Table,
    Id,
    UserId,
    Action,
    Ip,
    UserAgent,
    Meta,
    CreatedAt,
}
