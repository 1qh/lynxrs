use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use axum_extra::extract::PrivateCookieJar;
use base64::Engine as _;
use object_store::{ObjectStoreExt, path::Path as ObjPath};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use super::{FileDto as _FileDto, can_access};
use crate::{
    entity::{file_object, file_share},
    error::{AppError, Result},
    state::AppState,
};

#[allow(dead_code)]
type _ImportFix = _FileDto;

#[derive(Serialize, ToSchema)]
pub struct FileShareDto {
    pub download_count: i64,
    pub id: Uuid,
    pub file_id: Uuid,
    pub url: String,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct CreateShareInput {
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    #[validate(email)]
    pub email_to: Option<String>,
    pub ttl_hours: Option<i64>,
}

pub(crate) fn sha256_hex(value: &str) -> String {
    let digest = sha2::Sha256::digest(value.as_bytes());
    hex::encode(digest)
}

fn random_share_token() -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let mut buf = [0u8; 24];
    OsRng.fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

#[utoipa::path(
    post,
    path = "/files/{id}/shares",
    request_body = CreateShareInput,
    responses((status = 201, body = FileShareDto), (status = 404))
)]
pub async fn create_share(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(file_id): Path<Uuid>,
    Json(input): Json<CreateShareInput>,
) -> Result<impl IntoResponse> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(file_id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.owner_id != uid {
        return Err(AppError::NotFound);
    }
    let ttl = input.ttl_hours.unwrap_or(24).clamp(1, 720);
    let expires_at = Some(chrono::Utc::now() + chrono::Duration::hours(ttl));

    let raw = random_share_token();
    let token_hash = sha256_hex(&raw);
    let id = Uuid::now_v7();

    let password_hash = input.password.as_ref().map(|p| sha256_hex(p));
    let model = file_share::ActiveModel {
        id: Set(id),
        file_id: Set(file_id),
        token_hash: Set(token_hash),
        expires_at: Set(expires_at),
        created_at: Set(chrono::Utc::now()),
        revoked_at: Set(None),
        download_count: Set(0),
        password_hash: Set(password_hash),
    }
    .insert(&state.db)
    .await?;

    let url = format!(
        "{}/api/shares/{raw}",
        state.public_base_url.trim_end_matches('/')
    );
    if let Some(to) = input.email_to.clone() {
        let mailer = state.mailer.clone();
        let url_c = url.clone();
        let fname = row.filename.clone();
        tokio::spawn(async move {
            if let Err(e) = mailer.send_share_link(&to, &url_c, &fname).await {
                tracing::warn!(error=%e, "share link email failed");
            }
        });
    }
    metrics::counter!("simu_shares_created_total").increment(1);
    Ok((
        StatusCode::CREATED,
        Json(FileShareDto {
            id: model.id,
            file_id,
            url,
            expires_at: model.expires_at,
            created_at: model.created_at,
            download_count: model.download_count,
        }),
    ))
}

#[derive(Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ShareDownloadQuery {
    pub inline: Option<bool>,
    pub password: Option<String>,
}

#[utoipa::path(
    get,
    path = "/shares/{token}",
    params(ShareDownloadQuery),
    responses((status = 200), (status = 404), (status = 410), (status = 401))
)]
pub async fn download_share(
    State(state): State<AppState>,
    Path(token): Path<String>,
    axum::extract::Query(dq): axum::extract::Query<ShareDownloadQuery>,
) -> Result<impl IntoResponse> {
    let token_hash = sha256_hex(&token);
    let share = file_share::Entity::find()
        .filter(file_share::Column::TokenHash.eq(&token_hash))
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if share.revoked_at.is_some() {
        return Err(AppError::BadRequest("share revoked".into()));
    }
    if let Some(exp) = share.expires_at
        && exp < chrono::Utc::now()
    {
        return Err(AppError::BadRequest("share expired".into()));
    }
    if let Some(expected) = share.password_hash.as_deref() {
        // Constant-time compare: a variable-time `!=` on hex strings leaks
        // the matching prefix length, enabling a 1-byte-at-a-time guess.
        use subtle::ConstantTimeEq;
        let provided = dq.password.as_deref().map(sha256_hex).unwrap_or_default();
        if provided.as_bytes().ct_eq(expected.as_bytes()).unwrap_u8() == 0 {
            return Err(AppError::Unauthorized);
        }
    }
    let row = file_object::Entity::find_by_id(share.file_id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let obj_path = ObjPath::from(row.storage_key.clone());
    let result = state.storage.get(&obj_path).await?;
    let new_count = share.download_count + 1;
    let mut am: file_share::ActiveModel = share.into();
    am.download_count = Set(new_count);
    let _ = am.update(&state.db).await;
    let body = axum::body::Body::from_stream(result.into_stream());
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, row.content_type.clone()),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!(
                    "{}; filename=\"{}\"",
                    if dq.inline == Some(true) {
                        "inline"
                    } else {
                        "attachment"
                    },
                    row.filename
                ),
            ),
            (
                axum::http::header::CONTENT_LENGTH,
                row.size_bytes.to_string(),
            ),
        ],
        body,
    ))
}

#[utoipa::path(
    get,
    path = "/files/{id}/shares",
    responses((status = 200, body = [FileShareDto]), (status = 404))
)]
pub async fn list_shares(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(file_id): Path<Uuid>,
) -> Result<Json<Vec<FileShareDto>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(file_id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if !can_access(&state.db, uid, &row).await? {
        return Err(AppError::NotFound);
    }
    let shares = file_share::Entity::find()
        .filter(file_share::Column::FileId.eq(file_id))
        .filter(file_share::Column::RevokedAt.is_null())
        .order_by_desc(file_share::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(
        shares
            .into_iter()
            .map(|s| FileShareDto {
                id: s.id,
                file_id: s.file_id,
                url: String::new(),
                expires_at: s.expires_at,
                created_at: s.created_at,
                download_count: s.download_count,
            })
            .collect(),
    ))
}

#[utoipa::path(
    delete,
    path = "/files/shares/{id}",
    responses((status = 204), (status = 404))
)]
pub async fn revoke_share(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let share = file_share::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let file = file_object::Entity::find_by_id(share.file_id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if file.owner_id != uid {
        return Err(AppError::NotFound);
    }
    let mut am: file_share::ActiveModel = share.into();
    am.revoked_at = Set(Some(chrono::Utc::now()));
    am.update(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}
