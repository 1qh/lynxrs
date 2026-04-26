use axum::{
    Json,
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
};
use axum_extra::extract::PrivateCookieJar;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use bytes::Bytes;
use object_store::{ObjectStoreExt, PutPayload, path::Path as ObjPath};
use sea_orm::{ActiveModelTrait, Set};
use serde::Deserialize;
use sha2::Digest;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use super::thumb::maybe_store_thumbnail;
use super::{
    FileDto, MAX_FILE_BYTES, enforce_org_quota, enforce_quota, image_magic_ok, storage_key_for,
    user_org_ids,
};
use crate::{
    entity::file_object,
    error::{AppError, Result},
    events::EventMsg,
    state::AppState,
};

#[utoipa::path(post, path = "/files", responses((status = 201, body = FileDto), (status = 413)))]
pub async fn upload(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    mut multipart: Multipart,
) -> Result<impl IntoResponse> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;

    let mut field = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("multipart: {e}")))?
        .ok_or_else(|| AppError::BadRequest("missing file field".into()))?;

    let filename = field.file_name().unwrap_or("upload").to_string();
    let content_type = field
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_string();

    let id = Uuid::now_v7();
    let storage_key = storage_key_for(uid, None, id);
    let obj_path = ObjPath::from(storage_key.clone());

    let mut upload = state.storage.put_multipart(&obj_path).await?;

    const PART: usize = 8 * 1024 * 1024;
    let mut buf: Vec<u8> = Vec::with_capacity(PART);
    let mut hasher = sha2::Sha256::new();
    let mut total: usize = 0;
    let mut head: Vec<u8> = Vec::new();
    const HEAD_CAP: usize = 4 * 1024 * 1024;
    let is_image = content_type.starts_with("image/");

    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|e| AppError::BadRequest(format!("read: {e}")))?
    {
        total += chunk.len();
        if total > MAX_FILE_BYTES {
            let _ = upload.abort().await;
            return Err(AppError::BadRequest(format!(
                "file too large: max {MAX_FILE_BYTES} bytes"
            )));
        }
        hasher.update(&chunk);
        if head.len() < HEAD_CAP {
            let take = (HEAD_CAP - head.len()).min(chunk.len());
            head.extend_from_slice(&chunk[..take]);
        }
        buf.extend_from_slice(&chunk);
        if buf.len() >= PART {
            let part = std::mem::replace(&mut buf, Vec::with_capacity(PART));
            upload
                .put_part(PutPayload::from_bytes(Bytes::from(part)))
                .await?;
        }
    }
    if !buf.is_empty() {
        upload
            .put_part(PutPayload::from_bytes(Bytes::from(buf)))
            .await?;
    }
    upload.complete().await?;

    if is_image && !image_magic_ok(&content_type, &head) {
        let _ = state.storage.delete(&obj_path).await;
        return Err(AppError::BadRequest("image magic-byte check failed".into()));
    }

    enforce_quota(&state.db, uid, total as i64).await?;

    let sha = hex::encode(hasher.finalize());
    let model = file_object::ActiveModel {
        id: Set(id),
        owner_id: Set(uid),
        storage_key: Set(storage_key),
        filename: Set(filename.clone()),
        content_type: Set(content_type.clone()),
        size_bytes: Set(total as i64),
        created_at: Set(chrono::Utc::now()),
        sha256: Set(Some(sha)),
        deleted_at: Set(None),
        tags: Set(vec![]),
        org_id: Set(None),
        description: Set(None),
    }
    .insert(&state.db)
    .await?;

    metrics::counter!("simu_uploads_bytes_total").increment(total as u64);
    metrics::counter!("simu_uploads_total").increment(1);
    metrics::histogram!("simu_upload_bytes").record(total as f64);
    if is_image {
        maybe_store_thumbnail(&state, uid, model.id, &content_type, &head).await;
    }
    let _ = state.bus.send(EventMsg::FileCreated {
        file_id: model.id,
        owner_id: uid,
        filename,
    });
    Ok((StatusCode::CREATED, Json(FileDto::from(model))))
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct Base64UploadInput {
    #[validate(length(min = 1, max = 255))]
    pub filename: String,
    #[validate(length(min = 1, max = 255))]
    pub content_type: String,
    pub data_base64: String,
    #[serde(default)]
    pub org_id: Option<Uuid>,
}

#[utoipa::path(post, path = "/files/json", request_body = Base64UploadInput,
    responses((status = 201, body = FileDto)))]
pub async fn upload_json(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Json(input): Json<Base64UploadInput>,
) -> Result<impl IntoResponse> {
    input
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;

    let data = B64
        .decode(input.data_base64.as_bytes())
        .map_err(|e| AppError::BadRequest(format!("invalid base64: {e}")))?;

    if data.len() > MAX_FILE_BYTES {
        return Err(AppError::BadRequest(format!(
            "file too large: max {MAX_FILE_BYTES} bytes"
        )));
    }
    if !image_magic_ok(&input.content_type, &data) {
        return Err(AppError::BadRequest("image magic-byte check failed".into()));
    }
    enforce_quota(&state.db, uid, data.len() as i64).await?;

    if let Some(org) = input.org_id {
        let ids = user_org_ids(&state.db, uid).await?;
        if !ids.contains(&org) {
            return Err(AppError::Unauthorized);
        }
        enforce_org_quota(&state.db, org, data.len() as i64).await?;
    }

    let id = Uuid::now_v7();
    let storage_key = storage_key_for(uid, input.org_id, id);
    let obj_path = ObjPath::from(storage_key.clone());
    state
        .storage
        .put(&obj_path, PutPayload::from_bytes(Bytes::from(data.clone())))
        .await?;

    let filename = input.filename.clone();
    let sha = hex::encode(sha2::Sha256::digest(&data));
    let model = file_object::ActiveModel {
        id: Set(id),
        owner_id: Set(uid),
        storage_key: Set(storage_key),
        filename: Set(input.filename),
        content_type: Set(input.content_type),
        size_bytes: Set(data.len() as i64),
        created_at: Set(chrono::Utc::now()),
        sha256: Set(Some(sha)),
        deleted_at: Set(None),
        tags: Set(vec![]),
        org_id: Set(input.org_id),
        description: Set(None),
    }
    .insert(&state.db)
    .await?;

    metrics::counter!("simu_uploads_bytes_total").increment(data.len() as u64);
    metrics::counter!("simu_uploads_total").increment(1);
    metrics::histogram!("simu_upload_bytes").record(data.len() as f64);
    maybe_store_thumbnail(&state, uid, model.id, &model.content_type, &data).await;
    let _ = state.bus.send(EventMsg::FileCreated {
        file_id: model.id,
        owner_id: uid,
        filename,
    });
    Ok((StatusCode::CREATED, Json(FileDto::from(model))))
}
