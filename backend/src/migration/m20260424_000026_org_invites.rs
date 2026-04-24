use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OrgInvites::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(OrgInvites::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(OrgInvites::OrgId).uuid().not_null())
                    .col(ColumnDef::new(OrgInvites::Email).string().not_null())
                    .col(ColumnDef::new(OrgInvites::Role).string().not_null())
                    .col(ColumnDef::new(OrgInvites::TokenHash).string().not_null().unique_key())
                    .col(ColumnDef::new(OrgInvites::ExpiresAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(OrgInvites::AcceptedAt).timestamp_with_time_zone().null())
                    .col(
                        ColumnDef::new(OrgInvites::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_inv_org")
                            .from(OrgInvites::Table, OrgInvites::OrgId)
                            .to(Orgs::Table, Orgs::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OrgInvites::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Orgs { Table, Id }
#[derive(Iden)]
enum OrgInvites { Table, Id, OrgId, Email, Role, TokenHash, ExpiresAt, AcceptedAt, CreatedAt }
