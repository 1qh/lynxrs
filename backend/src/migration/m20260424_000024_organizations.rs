use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Orgs::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Orgs::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Orgs::Name).string().not_null())
                    .col(ColumnDef::new(Orgs::Slug).string().not_null().unique_key())
                    .col(
                        ColumnDef::new(Orgs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Memberships::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Memberships::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Memberships::OrgId).uuid().not_null())
                    .col(ColumnDef::new(Memberships::UserId).uuid().not_null())
                    .col(ColumnDef::new(Memberships::Role).string().not_null())
                    .col(
                        ColumnDef::new(Memberships::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_mbr_org")
                            .from(Memberships::Table, Memberships::OrgId)
                            .to(Orgs::Table, Orgs::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_mbr_user")
                            .from(Memberships::Table, Memberships::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_mbr_org_user_unique")
                    .table(Memberships::Table)
                    .col(Memberships::OrgId)
                    .col(Memberships::UserId)
                    .unique()
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Memberships::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Orgs::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
}

#[derive(Iden)]
enum Orgs {
    Table,
    Id,
    Name,
    Slug,
    CreatedAt,
}

#[derive(Iden)]
enum Memberships {
    Table,
    Id,
    OrgId,
    UserId,
    Role,
    CreatedAt,
}
