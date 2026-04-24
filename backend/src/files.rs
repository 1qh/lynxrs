use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use axum_extra::extract::PrivateCookieJar;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use bytes::Bytes;
use object_store::{ObjectStoreExt, PutPayload, path::Path as ObjPath};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    entity::{file_object, file_share},
    error::{AppError, Result},
    events::EventMsg,
    state::AppState,
};
use sha2::Digest;

#[derive(Serialize, ToSchema)]
pub struct FileDto {
    pub id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<file_object::Model> for FileDto {
    fn from(m: file_object::Model) -> Self {
        Self {
            id: m.id,
            filename: m.filename,
            content_type: m.content_type,
            size_bytes: m.size_bytes,
            created_at: m.created_at,
        }
    }
}

const MAX_FILE_BYTES: usize = 50 * 1024 * 1024; // 50 MB cap for spike
const USER_QUOTA_BYTES: i64 = 500 * 1024 * 1024; // 500 MB per user

async fn used_bytes(db: &sea_orm::DatabaseConnection, uid: Uuid) -> Result<i64> {
    let sizes: Vec<i64> = file_object::Entity::find()
        .filter(file_object::Column::OwnerId.eq(uid))
        .select_only()
        .column(file_object::Column::SizeBytes)
        .into_tuple()
        .all(db)
        .await?;
    Ok(sizes.iter().sum())
}

async fn enforce_quota(
    db: &sea_orm::DatabaseConnection,
    uid: Uuid,
    incoming: i64,
) -> Result<()> {
    let used = used_bytes(db, uid).await?;
    if used + incoming > USER_QUOTA_BYTES {
        return Err(AppError::BadRequest(format!(
            "quota exceeded: {} used + {} incoming > {} limit",
            used, incoming, USER_QUOTA_BYTES
        )));
    }
    Ok(())
}

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
    if row.owner_id != uid {
        return Err(AppError::NotFound);
    }
    let mut am: file_object::ActiveModel = row.into();
    am.filename = Set(input.filename);
    let updated = am.update(&state.db).await?;
    Ok(Json(FileDto::from(updated)))
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

#[utoipa::path(post, path = "/files", responses((status = 201, body = FileDto), (status = 413)))]
pub async fn upload(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    mut multipart: Multipart,
) -> Result<impl IntoResponse> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;

    let field = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("multipart: {e}")))?
        .ok_or_else(|| AppError::BadRequest("missing file field".into()))?;

    let filename = field.file_name().unwrap_or("upload").to_string();
    let content_type = field
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_string();
    let data: Bytes = field
        .bytes()
        .await
        .map_err(|e| AppError::BadRequest(format!("read: {e}")))?;

    if data.len() > MAX_FILE_BYTES {
        return Err(AppError::BadRequest(format!(
            "file too large: max {MAX_FILE_BYTES} bytes"
        )));
    }
    enforce_quota(&state.db, uid, data.len() as i64).await?;

    let id = Uuid::now_v7();
    let storage_key = format!("u/{uid}/{id}");
    let obj_path = ObjPath::from(storage_key.clone());

    state
        .storage
        .put(&obj_path, PutPayload::from_bytes(data.clone()))
        .await?;

    let model = file_object::ActiveModel {
        id: Set(id),
        owner_id: Set(uid),
        storage_key: Set(storage_key),
        filename: Set(filename.clone()),
        content_type: Set(content_type),
        size_bytes: Set(data.len() as i64),
        created_at: Set(chrono::Utc::now()),
    }
    .insert(&state.db)
    .await?;

    let _ = state.bus.send(EventMsg::FileCreated {
        file_id: model.id,
        owner_id: uid,
        filename,
    });
    Ok((StatusCode::CREATED, Json(FileDto::from(model))))
}

#[derive(Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListQuery {
    /// Max rows to return (default 50, cap 200).
    pub limit: Option<u64>,
    /// Cursor = most-recently-seen `created_at` RFC3339 timestamp. Returns rows strictly older.
    pub cursor: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Serialize, ToSchema)]
pub struct FileList {
    pub items: Vec<FileDto>,
    pub next_cursor: Option<chrono::DateTime<chrono::Utc>>,
}

#[utoipa::path(
    get,
    path = "/files",
    params(ListQuery),
    responses((status = 200, body = FileList))
)]
pub async fn list(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    axum::extract::Query(q): axum::extract::Query<ListQuery>,
) -> Result<Json<FileList>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let limit = q.limit.unwrap_or(50).min(200);

    let mut query = file_object::Entity::find()
        .filter(file_object::Column::OwnerId.eq(uid))
        .order_by_desc(file_object::Column::CreatedAt)
        .limit(limit + 1);
    if let Some(cursor) = q.cursor {
        query = query.filter(file_object::Column::CreatedAt.lt(cursor));
    }

    let mut rows = query.all(&state.db).await?;
    let has_more = rows.len() as u64 > limit;
    if has_more {
        rows.pop();
    }
    let next_cursor = if has_more {
        rows.last().map(|m| m.created_at)
    } else {
        None
    };
    let items = rows.into_iter().map(FileDto::from).collect();
    Ok(Json(FileList { items, next_cursor }))
}

#[utoipa::path(delete, path = "/files/{id}", responses((status = 204), (status = 404)))]
pub async fn delete(
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
    if row.owner_id != uid {
        return Err(AppError::NotFound);
    }
    let obj_path = ObjPath::from(row.storage_key.clone());
    // best-effort storage delete; DB is source of truth
    let _ = state.storage.delete(&obj_path).await;
    file_object::Entity::delete_by_id(id)
        .exec(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/files/{id}", responses((status = 200), (status = 404)))]
pub async fn download(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.owner_id != uid {
        return Err(AppError::NotFound);
    }

    let obj_path = ObjPath::from(row.storage_key.clone());
    let result = state.storage.get(&obj_path).await?;
    let bytes = result.bytes().await?;

    Ok((
        [
            (axum::http::header::CONTENT_TYPE, row.content_type.clone()),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", row.filename),
            ),
        ],
        bytes,
    ))
}

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
    /// Optional expiry in hours (default 24, max 720 = 30 days).
    pub ttl_hours: Option<i64>,
}

fn sha256_hex(value: &str) -> String {
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

    let model = file_share::ActiveModel {
        id: Set(id),
        file_id: Set(file_id),
        token_hash: Set(token_hash),
        expires_at: Set(expires_at),
        created_at: Set(chrono::Utc::now()),
        revoked_at: Set(None),
        download_count: Set(0),
    }
    .insert(&state.db)
    .await?;

    let url = format!(
        "{}/api/shares/{raw}",
        state.public_base_url.trim_end_matches('/')
    );
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

#[utoipa::path(
    get,
    path = "/shares/{token}",
    responses((status = 200), (status = 404), (status = 410))
)]
pub async fn download_share(
    State(state): State<AppState>,
    Path(token): Path<String>,
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
    let row = file_object::Entity::find_by_id(share.file_id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let obj_path = ObjPath::from(row.storage_key.clone());
    let result = state.storage.get(&obj_path).await?;
    let bytes = result.bytes().await?;
    let new_count = share.download_count + 1;
    let mut am: file_share::ActiveModel = share.into();
    am.download_count = Set(new_count);
    let _ = am.update(&state.db).await;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, row.content_type.clone()),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", row.filename),
            ),
        ],
        bytes,
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
    if row.owner_id != uid {
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

#[derive(Deserialize, ToSchema, Validate)]
pub struct Base64UploadInput {
    #[validate(length(min = 1, max = 255))]
    pub filename: String,
    #[validate(length(min = 1, max = 255))]
    pub content_type: String,
    pub data_base64: String,
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
    enforce_quota(&state.db, uid, data.len() as i64).await?;

    let id = Uuid::now_v7();
    let storage_key = format!("u/{uid}/{id}");
    let obj_path = ObjPath::from(storage_key.clone());
    state
        .storage
        .put(&obj_path, PutPayload::from_bytes(Bytes::from(data.clone())))
        .await?;

    let filename = input.filename.clone();
    let model = file_object::ActiveModel {
        id: Set(id),
        owner_id: Set(uid),
        storage_key: Set(storage_key),
        filename: Set(input.filename),
        content_type: Set(input.content_type),
        size_bytes: Set(data.len() as i64),
        created_at: Set(chrono::Utc::now()),
    }
    .insert(&state.db)
    .await?;

    let _ = state.bus.send(EventMsg::FileCreated {
        file_id: model.id,
        owner_id: uid,
        filename,
    });
    Ok((StatusCode::CREATED, Json(FileDto::from(model))))
}
