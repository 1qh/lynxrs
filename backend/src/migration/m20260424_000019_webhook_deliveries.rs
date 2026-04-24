use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(WebhookDeliveries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WebhookDeliveries::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(WebhookDeliveries::WebhookId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WebhookDeliveries::Attempt)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(WebhookDeliveries::Status).integer().null())
                    .col(
                        ColumnDef::new(WebhookDeliveries::DurationMs)
                            .integer()
                            .null(),
                    )
                    .col(ColumnDef::new(WebhookDeliveries::Error).string().null())
                    .col(
                        ColumnDef::new(WebhookDeliveries::EventKind)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WebhookDeliveries::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_whd_webhook")
                            .from(WebhookDeliveries::Table, WebhookDeliveries::WebhookId)
                            .to(Webhooks::Table, Webhooks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_whd_webhook_created")
                    .table(WebhookDeliveries::Table)
                    .col(WebhookDeliveries::WebhookId)
                    .col(WebhookDeliveries::CreatedAt)
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WebhookDeliveries::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Webhooks {
    Table,
    Id,
}

#[derive(Iden)]
enum WebhookDeliveries {
    Table,
    Id,
    WebhookId,
    Attempt,
    Status,
    DurationMs,
    Error,
    EventKind,
    CreatedAt,
}
