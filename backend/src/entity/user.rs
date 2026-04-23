use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    #[sea_orm(default_value = "user")]
    pub role: String,
    pub email_verified_at: Option<ChronoDateTimeUtc>,
    #[sea_orm(default_value = 0)]
    pub session_version: i32,
    pub created_at: ChronoDateTimeUtc,
    pub updated_at: ChronoDateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::file_object::Entity")]
    FileObjects,
}

impl Related<super::file_object::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FileObjects.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
