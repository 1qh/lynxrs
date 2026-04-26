use axum::{Json, extract::State};
use axum_extra::extract::PrivateCookieJar;
use object_store::{ObjectStoreExt, path::Path as ObjPath};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::user_org_ids;
use crate::{entity::file_object, error::Result, state::AppState};

#[derive(Deserialize, ToSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BulkAction {
    Delete { ids: Vec<Uuid> },
    Purge { ids: Vec<Uuid> },
    Tag { ids: Vec<Uuid>, tag: String },
    Untag { ids: Vec<Uuid>, tag: String },
}

#[derive(Serialize, ToSchema)]
pub struct BulkResult {
    pub affected: u64,
}

async fn resolve_accessible(
    db: &sea_orm::DatabaseConnection,
    uid: Uuid,
    org_ids: &[Uuid],
    ids: &[Uuid],
) -> Result<Vec<file_object::Model>> {
    let rows = file_object::Entity::find()
        .filter(file_object::Column::Id.is_in(ids.to_vec()))
        .all(db)
        .await?;
    Ok(rows
        .into_iter()
        .filter(|r| r.owner_id == uid || r.org_id.map(|o| org_ids.contains(&o)).unwrap_or(false))
        .collect())
}

#[utoipa::path(post, path = "/files/bulk", request_body = BulkAction,
    responses((status = 200, body = BulkResult)))]
pub async fn bulk(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Json(action): Json<BulkAction>,
) -> Result<Json<BulkResult>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let org_ids = user_org_ids(&state.db, uid).await?;
    let now = chrono::Utc::now();
    let affected = match action {
        BulkAction::Delete { ids } => {
            let rows = resolve_accessible(&state.db, uid, &org_ids, &ids).await?;
            let mut n = 0u64;
            for r in rows {
                if r.deleted_at.is_some() {
                    continue;
                }
                let mut am: file_object::ActiveModel = r.into();
                am.deleted_at = Set(Some(now));
                if am.update(&state.db).await.is_ok() {
                    n += 1;
                }
            }
            n
        }
        BulkAction::Purge { ids } => {
            let rows = resolve_accessible(&state.db, uid, &org_ids, &ids).await?;
            let mut n = 0u64;
            for r in rows {
                if r.deleted_at.is_none() {
                    continue;
                }
                if r.owner_id != uid {
                    continue;
                }
                // DB first; storage delete best-effort (orphan reclaimable
                // by housekeeping). Same reasoning as `trash::purge`.
                if file_object::Entity::delete_by_id(r.id)
                    .exec(&state.db)
                    .await
                    .is_ok()
                {
                    let _ = state.storage.delete(&ObjPath::from(r.storage_key)).await;
                    n += 1;
                }
            }
            n
        }
        BulkAction::Tag { ids, tag } => {
            let rows = resolve_accessible(&state.db, uid, &org_ids, &ids).await?;
            let mut n = 0u64;
            for r in rows {
                let mut tags = r.tags.clone();
                if !tags.contains(&tag) {
                    tags.push(tag.clone());
                }
                let mut am: file_object::ActiveModel = r.into();
                am.tags = Set(tags);
                if am.update(&state.db).await.is_ok() {
                    n += 1;
                }
            }
            n
        }
        BulkAction::Untag { ids, tag } => {
            let rows = resolve_accessible(&state.db, uid, &org_ids, &ids).await?;
            let mut n = 0u64;
            for r in rows {
                let tags: Vec<String> = r.tags.iter().filter(|t| **t != tag).cloned().collect();
                let mut am: file_object::ActiveModel = r.into();
                am.tags = Set(tags);
                if am.update(&state.db).await.is_ok() {
                    n += 1;
                }
            }
            n
        }
    };
    Ok(Json(BulkResult { affected }))
}
