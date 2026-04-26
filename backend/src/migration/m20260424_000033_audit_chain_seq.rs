use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Monotonic chain ordering — created_at can collide at millisecond
        // resolution under concurrent writers, so lookups by (created_at desc,
        // id desc) sometimes pick the wrong predecessor. chain_seq is assigned
        // by the database under the same advisory lock that gates the insert,
        // so it strictly matches insertion order.
        let backend = manager.get_database_backend();
        let conn = manager.get_connection();
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "ALTER TABLE audit_events ADD COLUMN chain_seq BIGSERIAL".to_string(),
        ))
        .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_audit_chain_seq")
                    .table(AuditEvents::Table)
                    .col(AuditEvents::ChainSeq)
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_audit_chain_seq")
                    .table(AuditEvents::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditEvents::Table)
                    .drop_column(AuditEvents::ChainSeq)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum AuditEvents {
    Table,
    ChainSeq,
}
