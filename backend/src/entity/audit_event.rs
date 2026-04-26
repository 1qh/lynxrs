use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audit_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub action: String,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub meta: serde_json::Value,
    pub created_at: ChronoDateTimeUtc,
    pub prev_hash: Option<String>,
    pub row_hash: Option<String>,
    /// Monotonic chain ordering, assigned by `BIGSERIAL` under the advisory
    /// lock that gates inserts. Use this — not `created_at` — to order the
    /// chain for verification.
    pub chain_seq: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
