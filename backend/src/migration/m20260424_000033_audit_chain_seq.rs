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
        // Add the column nullable + sequence first so we can backfill in
        // chain order (created_at asc, id asc) before letting BIGSERIAL take
        // over for new inserts.
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "ALTER TABLE audit_events ADD COLUMN chain_seq BIGINT".to_string(),
        ))
        .await?;
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "CREATE SEQUENCE IF NOT EXISTS audit_events_chain_seq_seq".to_string(),
        ))
        .await?;
        // Backfill existing rows in their original chain order.
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "WITH ordered AS ( \
               SELECT id, ROW_NUMBER() OVER (ORDER BY created_at ASC, id ASC) AS rn \
               FROM audit_events \
             ) \
             UPDATE audit_events ae SET chain_seq = ordered.rn \
             FROM ordered WHERE ae.id = ordered.id"
                .to_string(),
        ))
        .await?;
        // Advance the sequence past the highest backfilled value.
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "SELECT setval('audit_events_chain_seq_seq', \
                COALESCE((SELECT MAX(chain_seq) FROM audit_events), 0) + 1, false)"
                .to_string(),
        ))
        .await?;
        // Wire DEFAULT + NOT NULL so future inserts auto-assign.
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "ALTER TABLE audit_events \
             ALTER COLUMN chain_seq SET DEFAULT nextval('audit_events_chain_seq_seq'), \
             ALTER COLUMN chain_seq SET NOT NULL"
                .to_string(),
        ))
        .await?;
        conn.execute(sea_orm::Statement::from_string(
            backend,
            "ALTER SEQUENCE audit_events_chain_seq_seq OWNED BY audit_events.chain_seq".to_string(),
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
