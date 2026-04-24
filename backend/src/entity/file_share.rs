use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "file_shares")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub file_id: Uuid,
    #[serde(skip_serializing)]
    pub token_hash: String,
    pub expires_at: Option<ChronoDateTimeUtc>,
    pub created_at: ChronoDateTimeUtc,
    pub revoked_at: Option<ChronoDateTimeUtc>,
    pub download_count: i64,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::file_object::Entity",
        from = "Column::FileId",
        to = "super::file_object::Column::Id"
    )]
    File,
}

impl Related<super::file_object::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::File.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
