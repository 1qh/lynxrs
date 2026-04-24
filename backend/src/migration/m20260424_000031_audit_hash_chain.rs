use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AuditEvents::Table)
                    .add_column(ColumnDef::new(AuditEvents::PrevHash).string().null())
                    .add_column(ColumnDef::new(AuditEvents::RowHash).string().null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_audit_created_id")
                    .table(AuditEvents::Table)
                    .col(AuditEvents::CreatedAt)
                    .col(AuditEvents::Id)
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AuditEvents::Table)
                    .drop_column(AuditEvents::PrevHash)
                    .drop_column(AuditEvents::RowHash)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum AuditEvents {
    Table,
    Id,
    CreatedAt,
    PrevHash,
    RowHash,
}
