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
