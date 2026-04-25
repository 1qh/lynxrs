use axum::{
    Json,
    extract::{Path, State},
};
use axum_extra::extract::PrivateCookieJar;
use object_store::{ObjectStoreExt, path::Path as ObjPath};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use super::{FileDto, can_access, enforce_quota};
use crate::{
    entity::file_object,
    error::{AppError, Result},
    events::EventMsg,
    state::AppState,
};

#[derive(Serialize, ToSchema)]
pub struct PresignedDto {
    pub url: String,
    pub expires_in_seconds: u64,
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct PresignUploadInput {
    #[validate(length(min = 1, max = 255))]
    pub filename: String,
    #[validate(length(min = 1, max = 255))]
    pub content_type: String,
    #[validate(range(min = 1, max = 5368709120i64))]
    pub size_bytes: i64,
}

#[derive(Serialize, ToSchema)]
pub struct PresignUploadDto {
    pub file_id: Uuid,
    pub put_url: String,
    pub expires_in_seconds: u64,
}

#[utoipa::path(post, path = "/files/presign-upload", request_body = PresignUploadInput,
    responses((status = 200, body = PresignUploadDto)))]
pub async fn presign_upload(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Json(input): Json<PresignUploadInput>,
) -> Result<Json<PresignUploadDto>> {
    input
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    enforce_quota(&state.db, uid, input.size_bytes).await?;
    let file_id = Uuid::now_v7();
    let storage_key = format!("u/{uid}/{file_id}");

    file_object::ActiveModel {
        id: Set(file_id),
        owner_id: Set(uid),
        storage_key: Set(storage_key.clone()),
        filename: Set(input.filename),
        content_type: Set(input.content_type),
        size_bytes: Set(input.size_bytes),
        created_at: Set(chrono::Utc::now()),
        sha256: Set(None),
        deleted_at: Set(Some(chrono::Utc::now())),
        tags: Set(vec![]),
        org_id: Set(None),
        description: Set(None),
    }
    .insert(&state.db)
    .await?;

    use object_store::signer::Signer;
    let expires = std::time::Duration::from_secs(900);
    let url = state
        .signer
        .signed_url(reqwest::Method::PUT, &ObjPath::from(storage_key), expires)
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("presign PUT: {e}")))?;
    Ok(Json(PresignUploadDto {
        file_id,
        put_url: url.to_string(),
        expires_in_seconds: expires.as_secs(),
    }))
}

#[utoipa::path(post, path = "/files/{id}/confirm-upload",
    responses((status = 200, body = FileDto), (status = 404)))]
pub async fn confirm_upload(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<FileDto>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.owner_id != uid {
        return Err(AppError::NotFound);
    }

    let meta = state
        .storage
        .head(&ObjPath::from(row.storage_key.clone()))
        .await
        .map_err(|_| AppError::BadRequest("object not uploaded yet".into()))?;

    let actual_size = meta.size as i64;
    let mut am: file_object::ActiveModel = row.clone().into();
    am.size_bytes = Set(actual_size);
    am.deleted_at = Set(None);
    let updated = am.update(&state.db).await?;

    metrics::counter!("simu_uploads_total").increment(1);
    metrics::counter!("simu_uploads_bytes_total").increment(actual_size as u64);
    let _ = state.bus.send(EventMsg::FileCreated {
        file_id: updated.id,
        owner_id: uid,
        filename: updated.filename.clone(),
    });
    Ok(Json(FileDto::from(updated)))
}

#[utoipa::path(get, path = "/files/{id}/presign",
    responses((status = 200, body = PresignedDto), (status = 404)))]
pub async fn presign(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<PresignedDto>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.deleted_at.is_some() || !can_access(&state.db, uid, &row).await? {
        return Err(AppError::NotFound);
    }
    use object_store::signer::Signer;
    let expires = std::time::Duration::from_secs(300);
    let url = state
        .signer
        .signed_url(
            reqwest::Method::GET,
            &ObjPath::from(row.storage_key.clone()),
            expires,
        )
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("presign: {e}")))?;
    Ok(Json(PresignedDto {
        url: url.to_string(),
        expires_in_seconds: expires.as_secs(),
    }))
}
