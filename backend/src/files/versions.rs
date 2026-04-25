use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use axum_extra::extract::PrivateCookieJar;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use bytes::Bytes;
use object_store::{ObjectStoreExt, PutPayload, path::Path as ObjPath};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::Serialize;
use sha2::Digest;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use super::{
    Base64UploadInput, FileDto, MAX_FILE_BYTES, can_access, enforce_org_quota, enforce_quota,
};
use crate::{
    entity::{file_object, file_version},
    error::{AppError, Result},
    state::AppState,
};

#[derive(Serialize, ToSchema)]
pub struct VersionDto {
    pub id: Uuid,
    pub version_no: i32,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub sha256: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<file_version::Model> for VersionDto {
    fn from(m: file_version::Model) -> Self {
        Self {
            id: m.id,
            version_no: m.version_no,
            filename: m.filename,
            content_type: m.content_type,
            size_bytes: m.size_bytes,
            sha256: m.sha256,
            created_at: m.created_at,
        }
    }
}

#[utoipa::path(get, path = "/files/{id}/versions",
    responses((status = 200, body = [VersionDto]), (status = 404)))]
pub async fn list_versions(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<VersionDto>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if !can_access(&state.db, uid, &row).await? {
        return Err(AppError::NotFound);
    }
    let rows = file_version::Entity::find()
        .filter(file_version::Column::FileId.eq(id))
        .order_by_desc(file_version::Column::VersionNo)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(VersionDto::from).collect()))
}

pub(super) async fn snapshot_current(
    db: &sea_orm::DatabaseConnection,
    file: &file_object::Model,
) -> Result<i32> {
    let rows: Vec<i32> = file_version::Entity::find()
        .filter(file_version::Column::FileId.eq(file.id))
        .select_only()
        .column(file_version::Column::VersionNo)
        .into_tuple()
        .all(db)
        .await?;
    let next = rows.into_iter().max().unwrap_or(0) + 1;
    file_version::ActiveModel {
        id: Set(Uuid::now_v7()),
        file_id: Set(file.id),
        version_no: Set(next),
        storage_key: Set(file.storage_key.clone()),
        filename: Set(file.filename.clone()),
        content_type: Set(file.content_type.clone()),
        size_bytes: Set(file.size_bytes),
        sha256: Set(file.sha256.clone()),
        created_at: Set(chrono::Utc::now()),
    }
    .insert(db)
    .await?;
    Ok(next)
}

#[utoipa::path(post, path = "/files/{id}/versions", request_body = Base64UploadInput,
    responses((status = 201, body = FileDto), (status = 404)))]
pub async fn create_version(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
    Json(input): Json<Base64UploadInput>,
) -> Result<(StatusCode, Json<FileDto>)> {
    input
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.deleted_at.is_some() || !can_access(&state.db, uid, &row).await? {
        return Err(AppError::NotFound);
    }
    let data = B64
        .decode(input.data_base64.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("base64 decode: {e}")))?;
    if data.len() > MAX_FILE_BYTES {
        return Err(AppError::BadRequest(format!(
            "file too large: max {MAX_FILE_BYTES} bytes"
        )));
    }
    enforce_quota(&state.db, uid, data.len() as i64).await?;
    if let Some(org_id) = row.org_id {
        enforce_org_quota(&state.db, org_id, data.len() as i64).await?;
    }

    snapshot_current(&state.db, &row).await?;

    let new_key = format!("u/{uid}/{}", Uuid::now_v7());
    state
        .storage
        .put(
            &ObjPath::from(new_key.clone()),
            PutPayload::from_bytes(Bytes::from(data.clone())),
        )
        .await?;
    let sha = hex::encode(sha2::Sha256::digest(&data));
    let mut am: file_object::ActiveModel = row.into();
    am.storage_key = Set(new_key);
    am.content_type = Set(input.content_type);
    am.size_bytes = Set(data.len() as i64);
    am.sha256 = Set(Some(sha));
    let updated = am.update(&state.db).await?;
    Ok((StatusCode::CREATED, Json(FileDto::from(updated))))
}

#[utoipa::path(post, path = "/files/{id}/versions/{n}/restore",
    responses((status = 200, body = FileDto), (status = 404)))]
pub async fn restore_version(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path((id, n)): Path<(Uuid, i32)>,
) -> Result<Json<FileDto>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.deleted_at.is_some() || !can_access(&state.db, uid, &row).await? {
        return Err(AppError::NotFound);
    }
    let v = file_version::Entity::find()
        .filter(file_version::Column::FileId.eq(id))
        .filter(file_version::Column::VersionNo.eq(n))
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    snapshot_current(&state.db, &row).await?;
    let mut am: file_object::ActiveModel = row.into();
    am.storage_key = Set(v.storage_key);
    am.filename = Set(v.filename);
    am.content_type = Set(v.content_type);
    am.size_bytes = Set(v.size_bytes);
    am.sha256 = Set(v.sha256);
    let updated = am.update(&state.db).await?;
    Ok(Json(FileDto::from(updated)))
}
