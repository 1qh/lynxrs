use axum::{
    Json,
    extract::{Path, State},
};
use axum_extra::extract::PrivateCookieJar;
use object_store::{ObjectStoreExt, path::Path as ObjPath};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QuerySelect, Set};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use super::{FileDto, USER_QUOTA_BYTES, can_access, used_bytes};
use crate::{
    entity::{file_object, file_share},
    error::{AppError, Result},
    state::AppState,
};

#[derive(Serialize, ToSchema)]
pub struct QuotaDto {
    pub used_bytes: i64,
    pub limit_bytes: i64,
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct RenameInput {
    #[validate(length(min = 1, max = 255))]
    pub filename: String,
}

#[utoipa::path(
    patch,
    path = "/files/{id}",
    request_body = RenameInput,
    responses((status = 200, body = FileDto), (status = 404))
)]
pub async fn rename(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
    Json(input): Json<RenameInput>,
) -> Result<Json<FileDto>> {
    input
        .validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if !can_access(&state.db, uid, &row).await? {
        return Err(AppError::NotFound);
    }
    let mut am: file_object::ActiveModel = row.into();
    am.filename = Set(input.filename);
    let updated = am.update(&state.db).await?;
    Ok(Json(FileDto::from(updated)))
}

#[derive(Serialize, ToSchema)]
pub struct VerifyDto {
    pub ok: bool,
    pub stored: Option<String>,
    pub computed: String,
}

#[utoipa::path(post, path = "/files/{id}/verify",
    responses((status = 200, body = VerifyDto), (status = 404)))]
pub async fn verify(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<VerifyDto>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if !can_access(&state.db, uid, &row).await? {
        return Err(AppError::NotFound);
    }
    let obj_path = ObjPath::from(row.storage_key.clone());
    let result = state.storage.get(&obj_path).await?;
    let bytes = result.bytes().await?;
    let computed = hex::encode(sha2::Sha256::digest(&bytes));
    let ok = row.sha256.as_deref() == Some(computed.as_str());
    Ok(Json(VerifyDto {
        ok,
        stored: row.sha256,
        computed,
    }))
}

#[utoipa::path(get, path = "/me/export", responses((status = 200)))]
pub async fn me_export(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<axum::response::Response> {
    use axum::response::IntoResponse;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let u = crate::entity::user::Entity::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let files = file_object::Entity::find()
        .filter(file_object::Column::OwnerId.eq(uid))
        .all(&state.db)
        .await?;
    let shares = file_share::Entity::find()
        .inner_join(file_object::Entity)
        .filter(file_object::Column::OwnerId.eq(uid))
        .all(&state.db)
        .await?;
    let tokens = crate::entity::api_token::Entity::find()
        .filter(crate::entity::api_token::Column::UserId.eq(uid))
        .all(&state.db)
        .await?;
    let webhooks = crate::entity::webhook::Entity::find()
        .filter(crate::entity::webhook::Column::UserId.eq(uid))
        .all(&state.db)
        .await?;
    let audit = crate::entity::audit_event::Entity::find()
        .filter(crate::entity::audit_event::Column::UserId.eq(uid))
        .all(&state.db)
        .await?;
    let export = serde_json::json!({
        "user": u,
        "files": files,
        "shares": shares,
        "tokens": tokens,
        "webhooks": webhooks,
        "audit": audit,
        "exported_at": chrono::Utc::now(),
    });
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "application/json"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=\"simu-export.json\"",
            ),
        ],
        serde_json::to_string_pretty(&export).unwrap_or_default(),
    )
        .into_response())
}

#[derive(Serialize, ToSchema)]
pub struct MeStatsDto {
    pub files: u64,
    pub trashed: u64,
    pub total_bytes: i64,
    pub shares: u64,
    pub tokens: u64,
    pub webhooks: u64,
}

#[utoipa::path(get, path = "/me/stats", responses((status = 200, body = MeStatsDto)))]
pub async fn me_stats(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<MeStatsDto>> {
    use sea_orm::PaginatorTrait;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let files = file_object::Entity::find()
        .filter(file_object::Column::OwnerId.eq(uid))
        .filter(file_object::Column::DeletedAt.is_null())
        .count(&state.db)
        .await?;
    let trashed = file_object::Entity::find()
        .filter(file_object::Column::OwnerId.eq(uid))
        .filter(file_object::Column::DeletedAt.is_not_null())
        .count(&state.db)
        .await?;
    let sizes: Vec<i64> = file_object::Entity::find()
        .filter(file_object::Column::OwnerId.eq(uid))
        .filter(file_object::Column::DeletedAt.is_null())
        .select_only()
        .column(file_object::Column::SizeBytes)
        .into_tuple()
        .all(&state.db)
        .await?;
    let total_bytes: i64 = sizes.iter().sum();
    let shares = file_share::Entity::find()
        .inner_join(file_object::Entity)
        .filter(file_object::Column::OwnerId.eq(uid))
        .filter(file_share::Column::RevokedAt.is_null())
        .count(&state.db)
        .await?;
    let tokens = crate::entity::api_token::Entity::find()
        .filter(crate::entity::api_token::Column::UserId.eq(uid))
        .filter(crate::entity::api_token::Column::RevokedAt.is_null())
        .count(&state.db)
        .await?;
    let webhooks = crate::entity::webhook::Entity::find()
        .filter(crate::entity::webhook::Column::UserId.eq(uid))
        .count(&state.db)
        .await?;
    Ok(Json(MeStatsDto {
        files,
        trashed,
        total_bytes,
        shares,
        tokens,
        webhooks,
    }))
}

#[derive(Deserialize, Serialize, ToSchema, Validate)]
pub struct ImportFile {
    #[validate(length(min = 1, max = 255))]
    pub filename: String,
    #[validate(length(min = 1, max = 255))]
    pub content_type: String,
    pub data_base64: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Deserialize, Serialize, ToSchema, Validate)]
pub struct ImportRequest {
    #[validate(length(min = 1, max = 1000))]
    pub files: Vec<ImportFile>,
}

#[derive(Serialize, ToSchema)]
pub struct ImportResult {
    pub imported: u64,
    pub skipped: u64,
    pub bytes: i64,
}

#[utoipa::path(post, path = "/me/import", request_body = ImportRequest,
    responses((status = 200, body = ImportResult), (status = 401), (status = 400)))]
pub async fn me_import(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Json(input): Json<ImportRequest>,
) -> Result<Json<ImportResult>> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
    use bytes::Bytes;
    use object_store::{ObjectStoreExt, PutPayload, path::Path as ObjPath};
    use sea_orm::Set;
    use sha2::Digest;
    input
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;

    let mut imported: u64 = 0;
    let mut skipped: u64 = 0;
    let mut bytes_total: i64 = 0;

    for f in input.files {
        // Decode + size check
        let Ok(data) = B64.decode(f.data_base64.as_bytes()) else {
            skipped += 1;
            continue;
        };
        if data.len() > super::MAX_FILE_BYTES {
            skipped += 1;
            continue;
        }
        // Per-file quota check
        if super::enforce_quota(&state.db, uid, data.len() as i64)
            .await
            .is_err()
        {
            skipped += 1;
            continue;
        }
        let id = uuid::Uuid::now_v7();
        let key = super::storage_key_for(uid, None, id);
        if state
            .storage
            .put(
                &ObjPath::from(key.clone()),
                PutPayload::from_bytes(Bytes::from(data.clone())),
            )
            .await
            .is_err()
        {
            skipped += 1;
            continue;
        }
        let sha = hex::encode(sha2::Sha256::digest(&data));
        let row = file_object::ActiveModel {
            id: Set(id),
            owner_id: Set(uid),
            storage_key: Set(key),
            filename: Set(f.filename),
            content_type: Set(f.content_type),
            size_bytes: Set(data.len() as i64),
            created_at: Set(chrono::Utc::now()),
            sha256: Set(Some(sha)),
            deleted_at: Set(None),
            tags: Set(f.tags),
            org_id: Set(None),
            description: Set(f.description),
        };
        use sea_orm::ActiveModelTrait;
        if row.insert(&state.db).await.is_err() {
            skipped += 1;
            continue;
        }
        imported += 1;
        bytes_total += data.len() as i64;
    }
    Ok(Json(ImportResult {
        imported,
        skipped,
        bytes: bytes_total,
    }))
}

#[utoipa::path(get, path = "/me/quota", responses((status = 200, body = QuotaDto)))]
pub async fn quota(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<QuotaDto>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    Ok(Json(QuotaDto {
        used_bytes: used_bytes(&state.db, uid).await?,
        limit_bytes: USER_QUOTA_BYTES,
    }))
}
