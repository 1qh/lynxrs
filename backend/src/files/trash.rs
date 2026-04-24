use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use axum_extra::extract::PrivateCookieJar;
use object_store::{ObjectStoreExt, path::Path as ObjPath};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use uuid::Uuid;

use super::{FileDto, FileList};
use crate::{
    entity::file_object,
    error::{AppError, Result},
    state::AppState,
};

#[utoipa::path(get, path = "/trash", responses((status = 200, body = FileList)))]
pub async fn list_trash(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<FileList>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let rows = file_object::Entity::find()
        .filter(file_object::Column::OwnerId.eq(uid))
        .filter(file_object::Column::DeletedAt.is_not_null())
        .order_by_desc(file_object::Column::DeletedAt)
        .limit(200)
        .all(&state.db)
        .await?;
    Ok(Json(FileList {
        items: rows.into_iter().map(FileDto::from).collect(),
        next_cursor: None,
    }))
}

#[utoipa::path(post, path = "/trash/{id}/restore", responses((status = 204), (status = 404)))]
pub async fn restore(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.owner_id != uid || row.deleted_at.is_none() {
        return Err(AppError::NotFound);
    }
    let mut am: file_object::ActiveModel = row.into();
    am.deleted_at = Set(None);
    am.update(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/trash",
    responses((status = 200, description = "number of files purged")))]
pub async fn empty_trash(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<serde_json::Value>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let rows = file_object::Entity::find()
        .filter(file_object::Column::OwnerId.eq(uid))
        .filter(file_object::Column::DeletedAt.is_not_null())
        .all(&state.db)
        .await?;
    let mut purged = 0;
    for r in rows {
        let p = ObjPath::from(r.storage_key.clone());
        let _ = state.storage.delete(&p).await;
        let _ = file_object::Entity::delete_by_id(r.id)
            .exec(&state.db)
            .await;
        purged += 1;
    }
    Ok(Json(serde_json::json!({ "purged": purged })))
}

#[utoipa::path(delete, path = "/trash/{id}",
    responses((status = 204), (status = 404)))]
pub async fn purge(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.owner_id != uid || row.deleted_at.is_none() {
        return Err(AppError::NotFound);
    }
    let obj_path = ObjPath::from(row.storage_key.clone());
    let _ = state.storage.delete(&obj_path).await;
    file_object::Entity::delete_by_id(id)
        .exec(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
