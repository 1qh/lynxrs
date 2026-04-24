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
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    entity::{file_comment, file_object, file_share},
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
    pub sha256: Option<String>,
    pub tags: Vec<String>,
    pub org_id: Option<Uuid>,
    pub description: Option<String>,
}

impl From<file_object::Model> for FileDto {
    fn from(m: file_object::Model) -> Self {
        Self {
            id: m.id,
            filename: m.filename,
            content_type: m.content_type,
            size_bytes: m.size_bytes,
            created_at: m.created_at,
            sha256: m.sha256,
            tags: m.tags,
            org_id: m.org_id,
            description: m.description,
        }
    }
}

pub(crate) const MAX_FILE_BYTES: usize = 50 * 1024 * 1024; // 50 MB cap for spike
pub(crate) const USER_QUOTA_BYTES: i64 = 500 * 1024 * 1024; // 500 MB per user
const ORG_QUOTA_BYTES: i64 = 5 * 1024 * 1024 * 1024; // 5 GB per org

fn image_magic_ok(claimed: &str, data: &[u8]) -> bool {
    if !claimed.starts_with("image/") {
        return true;
    }
    if data.len() < 12 {
        return false;
    }
    // PNG: 89 50 4E 47
    if data.starts_with(b"\x89PNG") {
        return claimed.contains("png");
    }
    // JPEG: FF D8 FF
    if data.starts_with(b"\xff\xd8\xff") {
        return claimed.contains("jpeg") || claimed.contains("jpg");
    }
    // GIF: GIF87a / GIF89a
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        return claimed.contains("gif");
    }
    // WebP: RIFF....WEBP
    if &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return claimed.contains("webp");
    }
    // AVIF/HEIC etc: ftypavif / ftypheic in bytes 4..12
    if data.len() >= 12 && &data[4..8] == b"ftyp" {
        let brand = &data[8..12];
        if brand == b"avif" {
            return claimed.contains("avif");
        }
        if brand == b"heic" || brand == b"heif" {
            return claimed.contains("heic") || claimed.contains("heif");
        }
    }
    false
}

async fn enforce_org_quota(
    db: &sea_orm::DatabaseConnection,
    org_id: Uuid,
    incoming: i64,
) -> Result<()> {
    let sizes: Vec<i64> = file_object::Entity::find()
        .filter(file_object::Column::OrgId.eq(org_id))
        .filter(file_object::Column::DeletedAt.is_null())
        .select_only()
        .column(file_object::Column::SizeBytes)
        .into_tuple()
        .all(db)
        .await?;
    let used: i64 = sizes.iter().sum();
    if used + incoming > ORG_QUOTA_BYTES {
        return Err(AppError::BadRequest(format!(
            "org quota exceeded: {} used + {} incoming > {} limit",
            used, incoming, ORG_QUOTA_BYTES
        )));
    }
    Ok(())
}

pub(crate) async fn user_org_ids(db: &sea_orm::DatabaseConnection, uid: Uuid) -> Result<Vec<Uuid>> {
    use crate::entity::membership;
    let mbrs = membership::Entity::find()
        .filter(membership::Column::UserId.eq(uid))
        .select_only()
        .column(membership::Column::OrgId)
        .into_tuple::<Uuid>()
        .all(db)
        .await?;
    Ok(mbrs)
}

pub(crate) async fn can_access(
    db: &sea_orm::DatabaseConnection,
    uid: Uuid,
    file: &file_object::Model,
) -> Result<bool> {
    if file.owner_id == uid {
        return Ok(true);
    }
    if let Some(org) = file.org_id {
        let ids = user_org_ids(db, uid).await?;
        return Ok(ids.contains(&org));
    }
    Ok(false)
}

pub(crate) async fn used_bytes(db: &sea_orm::DatabaseConnection, uid: Uuid) -> Result<i64> {
    let sizes: Vec<i64> = file_object::Entity::find()
        .filter(file_object::Column::OwnerId.eq(uid))
        .select_only()
        .column(file_object::Column::SizeBytes)
        .into_tuple()
        .all(db)
        .await?;
    Ok(sizes.iter().sum())
}

pub(crate) async fn enforce_quota(
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
    let storage_key = format!("u/{uid}/{id}");
    let obj_path = ObjPath::from(storage_key.clone());

    let mut upload = state.storage.put_multipart(&obj_path).await?;

    const PART: usize = 8 * 1024 * 1024; // 8 MiB parts
    let mut buf: Vec<u8> = Vec::with_capacity(PART);
    let mut hasher = sha2::Sha256::new();
    let mut total: usize = 0;
    // Keep head bytes for image magic-byte sniff + thumbnail (first ≤4 MiB).
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
    // Flush tail
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

#[derive(Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListQuery {
    /// Max rows to return (default 50, cap 200).
    pub limit: Option<u64>,
    /// Cursor = most-recently-seen `created_at` RFC3339 timestamp. Returns rows strictly older.
    pub cursor: Option<chrono::DateTime<chrono::Utc>>,
    /// Case-insensitive filename substring filter.
    pub q: Option<String>,
    /// Filter: only files containing this tag.
    pub tag: Option<String>,
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

    let org_ids = user_org_ids(&state.db, uid).await?;
    let mut access = sea_orm::Condition::any().add(file_object::Column::OwnerId.eq(uid));
    if !org_ids.is_empty() {
        access = access.add(file_object::Column::OrgId.is_in(org_ids));
    }
    let mut query = file_object::Entity::find()
        .filter(access)
        .filter(file_object::Column::DeletedAt.is_null())
        .order_by_desc(file_object::Column::CreatedAt)
        .limit(limit + 1);
    if let Some(cursor) = q.cursor {
        query = query.filter(file_object::Column::CreatedAt.lt(cursor));
    }

    let mut rows = query.all(&state.db).await?;
    if let Some(t) = q.tag.as_ref().filter(|s| !s.is_empty()) {
        rows.retain(|r| r.tags.iter().any(|x| x == t));
    }
    if let Some(needle) = q.q.as_ref().filter(|s| !s.is_empty()) {
        let n = needle.to_lowercase();
        rows.retain(|r| r.filename.to_lowercase().contains(&n));
    }
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
    if !can_access(&state.db, uid, &row).await? {
        return Err(AppError::NotFound);
    }
    let mut am: file_object::ActiveModel = row.into();
    am.deleted_at = Set(Some(chrono::Utc::now()));
    am.update(&state.db).await?;
    let _ = state.bus.send(EventMsg::FileDeleted {
        file_id: id,
        owner_id: uid,
    });
    Ok(StatusCode::NO_CONTENT)
}

fn thumb_key(file_id: Uuid, uid: Uuid) -> String {
    format!("u/{uid}/thumb/{file_id}.jpg")
}

async fn maybe_store_thumbnail(
    state: &AppState,
    uid: Uuid,
    file_id: Uuid,
    content_type: &str,
    bytes: &[u8],
) {
    if !content_type.starts_with("image/") {
        return;
    }
    let bytes = bytes.to_vec();
    let state = state.clone();
    tokio::task::spawn(async move {
        let Ok(thumb) = tokio::task::spawn_blocking(move || -> Option<Vec<u8>> {
            let img = image::load_from_memory(&bytes).ok()?;
            let small = img.thumbnail(256, 256);
            let mut out = std::io::Cursor::new(Vec::new());
            small.write_to(&mut out, image::ImageFormat::Jpeg).ok()?;
            Some(out.into_inner())
        })
        .await
        else {
            return;
        };
        let Some(data) = thumb else { return };
        let key = thumb_key(file_id, uid);
        if let Err(e) = state
            .storage
            .put(&ObjPath::from(key), PutPayload::from(Bytes::from(data)))
            .await
        {
            tracing::warn!(%file_id, error=%e, "thumbnail upload failed");
        }
    });
}

#[utoipa::path(get, path = "/files/{id}/thumbnail",
    responses((status = 200), (status = 404)))]
pub async fn thumbnail(
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
    if row.deleted_at.is_some() || !can_access(&state.db, uid, &row).await? {
        return Err(AppError::NotFound);
    }
    let key = thumb_key(id, uid);
    let result = state
        .storage
        .get(&ObjPath::from(key))
        .await
        .map_err(|_| AppError::NotFound)?;
    let body = axum::body::Body::from_stream(result.into_stream());
    axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "image/jpeg")
        .body(body)
        .map_err(|e| AppError::Other(anyhow::anyhow!(e)))
}

pub mod versions;
#[allow(unused_imports)]
pub use versions::*;

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
    // Fetch all requested rows; then keep those the user can access.
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
            .filter(|r| {
                r.owner_id == uid || r.org_id.map(|o| org_ids.contains(&o)).unwrap_or(false)
            })
            .collect())
    }
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
                // Only file owner may permanently purge.
                if r.owner_id != uid {
                    continue;
                }
                let p = ObjPath::from(r.storage_key.clone());
                let _ = state.storage.delete(&p).await;
                if file_object::Entity::delete_by_id(r.id)
                    .exec(&state.db)
                    .await
                    .is_ok()
                {
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

pub mod tags;
#[allow(unused_imports)]
pub use tags::*;

pub mod trash;
#[allow(unused_imports)]
pub use trash::*;

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
    #[validate(range(min = 1, max = 5368709120i64))] // 5 GiB cap for presigned PUT
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

    // Pre-create the row in "pending" state (size known, no sha256 yet).
    // Mark deleted_at = NULL; client must confirm to finalize.
    // Use a sentinel content_type prefix "pending:" on storage_key? Simpler: just insert.
    file_object::ActiveModel {
        id: Set(file_id),
        owner_id: Set(uid),
        storage_key: Set(storage_key.clone()),
        filename: Set(input.filename),
        content_type: Set(input.content_type),
        size_bytes: Set(input.size_bytes),
        created_at: Set(chrono::Utc::now()),
        sha256: Set(None),
        deleted_at: Set(Some(chrono::Utc::now())), // hidden until confirmed
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

    // HEAD the object to confirm it exists + fetch size.
    let meta = state
        .storage
        .head(&ObjPath::from(row.storage_key.clone()))
        .await
        .map_err(|_| AppError::BadRequest("object not uploaded yet".into()))?;

    let actual_size = meta.size as i64;
    let mut am: file_object::ActiveModel = row.clone().into();
    am.size_bytes = Set(actual_size);
    am.deleted_at = Set(None); // un-hide
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

#[utoipa::path(head, path = "/files/{id}", responses((status = 200), (status = 404)))]
pub async fn head_file(
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
    if row.deleted_at.is_some() || !can_access(&state.db, uid, &row).await? {
        return Err(AppError::NotFound);
    }
    Ok(([
        (axum::http::header::CONTENT_TYPE, row.content_type.clone()),
        (
            axum::http::header::CONTENT_LENGTH,
            row.size_bytes.to_string(),
        ),
        (axum::http::header::ACCEPT_RANGES, "bytes".to_string()),
        (
            axum::http::header::ETAG,
            format!("\"{}\"", row.sha256.as_deref().unwrap_or("")),
        ),
    ],))
}

#[derive(Deserialize, ToSchema, utoipa::IntoParams)]
pub struct DownloadQuery {
    /// If true, serve Content-Disposition: inline (for browser preview).
    pub inline: Option<bool>,
}

#[utoipa::path(get, path = "/files/{id}", params(DownloadQuery),
    responses((status = 200), (status = 206), (status = 404)))]
pub async fn download(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
    axum::extract::Query(dq): axum::extract::Query<DownloadQuery>,
) -> Result<impl IntoResponse> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.deleted_at.is_some() || !can_access(&state.db, uid, &row).await? {
        return Err(AppError::NotFound);
    }

    metrics::counter!("simu_downloads_total").increment(1);
    metrics::counter!("simu_downloads_bytes_total").increment(row.size_bytes as u64);
    metrics::histogram!("simu_download_bytes").record(row.size_bytes as f64);
    let obj_path = ObjPath::from(row.storage_key.clone());
    let total = row.size_bytes as u64;

    if let Some(range_hdr) = headers.get(axum::http::header::RANGE)
        && let Some((start, end)) = parse_range(range_hdr.to_str().unwrap_or(""), total)
    {
        let bytes = state.storage.get_range(&obj_path, start..end + 1).await?;
        let len = bytes.len() as u64;
        return axum::response::Response::builder()
            .status(axum::http::StatusCode::PARTIAL_CONTENT)
            .header(axum::http::header::CONTENT_TYPE, row.content_type.clone())
            .header(
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
            )
            .header(axum::http::header::CONTENT_LENGTH, len.to_string())
            .header(axum::http::header::ACCEPT_RANGES, "bytes")
            .header(
                axum::http::header::CONTENT_RANGE,
                format!("bytes {start}-{end}/{total}"),
            )
            .body(axum::body::Body::from(bytes))
            .map_err(|e| AppError::Other(anyhow::anyhow!(e)));
    }

    let result = state.storage.get(&obj_path).await?;
    let body = axum::body::Body::from_stream(result.into_stream());
    axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, row.content_type.clone())
        .header(
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
        )
        .header(axum::http::header::CONTENT_LENGTH, total.to_string())
        .header(axum::http::header::ACCEPT_RANGES, "bytes")
        .body(body)
        .map_err(|e| AppError::Other(anyhow::anyhow!(e)))
}

fn parse_range(raw: &str, total: u64) -> Option<(u64, u64)> {
    let rest = raw.strip_prefix("bytes=")?;
    let (a, b) = rest.split_once('-')?;
    let start: u64 = a.parse().ok()?;
    let end: u64 = if b.is_empty() {
        total - 1
    } else {
        b.parse().ok()?
    };
    if start > end || end >= total {
        return None;
    }
    Some((start, end))
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
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    #[validate(email)]
    pub email_to: Option<String>,
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
        let provided = dq.password.as_deref().map(sha256_hex).unwrap_or_default();
        if provided != expected {
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

    // If org_id set, caller must be a member and org must have quota.
    if let Some(org) = input.org_id {
        let ids = user_org_ids(&state.db, uid).await?;
        if !ids.contains(&org) {
            return Err(AppError::Unauthorized);
        }
        enforce_org_quota(&state.db, org, data.len() as i64).await?;
    }

    let id = Uuid::now_v7();
    let storage_key = format!("u/{uid}/{id}");
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

#[derive(Deserialize, ToSchema)]
pub struct ZipDownloadInput {
    pub ids: Vec<Uuid>,
}

#[utoipa::path(post, path = "/files/download-zip", request_body = ZipDownloadInput,
    responses((status = 200)))]
pub async fn download_zip(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Json(input): Json<ZipDownloadInput>,
) -> Result<axum::response::Response> {
    use axum::response::IntoResponse;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    if input.ids.is_empty() || input.ids.len() > 200 {
        return Err(AppError::BadRequest("ids required, max 200".into()));
    }
    let rows = file_object::Entity::find()
        .filter(file_object::Column::Id.is_in(input.ids.clone()))
        .filter(file_object::Column::DeletedAt.is_null())
        .all(&state.db)
        .await?;
    let mut accessible = Vec::with_capacity(rows.len());
    for r in rows {
        if can_access(&state.db, uid, &r).await? {
            accessible.push(r);
        }
    }
    if accessible.is_empty() {
        return Err(AppError::NotFound);
    }

    // Fetch all object bytes serially (simple).
    let mut items: Vec<(String, bytes::Bytes)> = Vec::with_capacity(accessible.len());
    for r in accessible {
        let result = state
            .storage
            .get(&ObjPath::from(r.storage_key.clone()))
            .await?;
        let b = result.bytes().await?;
        items.push((r.filename, b));
    }

    let zipped = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        use std::io::{Cursor, Write};
        use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};
        let mut buf = Cursor::new(Vec::<u8>::new());
        {
            let mut w = ZipWriter::new(&mut buf);
            let opts: SimpleFileOptions = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .unix_permissions(0o644);
            let mut seen = std::collections::HashMap::<String, u32>::new();
            for (name, bytes) in items {
                let final_name = {
                    let c = seen.entry(name.clone()).or_insert(0);
                    *c += 1;
                    if *c == 1 {
                        name
                    } else {
                        format!("{}__{}", name, *c - 1)
                    }
                };
                w.start_file(final_name, opts)
                    .map_err(|e| AppError::Other(anyhow::anyhow!("zip: {e}")))?;
                w.write_all(&bytes)
                    .map_err(|e| AppError::Other(anyhow::anyhow!("zip write: {e}")))?;
            }
            w.finish()
                .map_err(|e| AppError::Other(anyhow::anyhow!("zip finish: {e}")))?;
        }
        Ok(buf.into_inner())
    })
    .await
    .map_err(|e| AppError::Other(anyhow::anyhow!("spawn_blocking: {e}")))??;

    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "application/zip"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=\"simu.zip\"",
            ),
        ],
        zipped,
    )
        .into_response())
}

#[derive(Serialize, ToSchema)]
pub struct CommentDto {
    pub id: Uuid,
    pub file_id: Uuid,
    pub user_id: Uuid,
    pub body: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct CommentInput {
    #[validate(length(min = 1, max = 4000))]
    pub body: String,
}

#[utoipa::path(get, path = "/files/{id}/comments",
    responses((status = 200, body = [CommentDto]), (status = 404)))]
pub async fn list_comments(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<CommentDto>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if !can_access(&state.db, uid, &row).await? {
        return Err(AppError::NotFound);
    }
    let rows = file_comment::Entity::find()
        .filter(file_comment::Column::FileId.eq(id))
        .order_by_desc(file_comment::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| CommentDto {
                id: r.id,
                file_id: r.file_id,
                user_id: r.user_id,
                body: r.body,
                created_at: r.created_at,
            })
            .collect(),
    ))
}

#[utoipa::path(post, path = "/files/{id}/comments", request_body = CommentInput,
    responses((status = 201, body = CommentDto), (status = 404)))]
pub async fn add_comment(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
    Json(input): Json<CommentInput>,
) -> Result<(StatusCode, Json<CommentDto>)> {
    input
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if !can_access(&state.db, uid, &row).await? {
        return Err(AppError::NotFound);
    }
    let c = file_comment::ActiveModel {
        id: Set(Uuid::now_v7()),
        file_id: Set(id),
        user_id: Set(uid),
        body: Set(input.body),
        created_at: Set(chrono::Utc::now()),
    }
    .insert(&state.db)
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(CommentDto {
            id: c.id,
            file_id: c.file_id,
            user_id: c.user_id,
            body: c.body,
            created_at: c.created_at,
        }),
    ))
}

#[utoipa::path(delete, path = "/files/{file_id}/comments/{id}",
    responses((status = 204), (status = 404)))]
pub async fn delete_comment(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path((file_id, id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let c = file_comment::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if c.file_id != file_id {
        return Err(AppError::NotFound);
    }
    let file = file_object::Entity::find_by_id(file_id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    // Delete allowed if: user authored the comment OR owns the file.
    if c.user_id != uid && file.owner_id != uid {
        return Err(AppError::NotFound);
    }
    file_comment::Entity::delete_by_id(id)
        .exec(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/files/{id}/star", responses((status = 204), (status = 404)))]
pub async fn star_file(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    use crate::entity::file_star;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if !can_access(&state.db, uid, &row).await? {
        return Err(AppError::NotFound);
    }
    let _ = file_star::ActiveModel {
        user_id: Set(uid),
        file_id: Set(id),
        created_at: Set(chrono::Utc::now()),
    }
    .insert(&state.db)
    .await;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/files/{id}/star", responses((status = 204)))]
pub async fn unstar_file(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    use crate::entity::file_star;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    file_star::Entity::delete_many()
        .filter(file_star::Column::UserId.eq(uid))
        .filter(file_star::Column::FileId.eq(id))
        .exec(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/files/starred", responses((status = 200, body = [FileDto])))]
pub async fn list_starred(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<Vec<FileDto>>> {
    use crate::entity::file_star;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let fids: Vec<Uuid> = file_star::Entity::find()
        .filter(file_star::Column::UserId.eq(uid))
        .select_only()
        .column(file_star::Column::FileId)
        .into_tuple()
        .all(&state.db)
        .await?;
    if fids.is_empty() {
        return Ok(Json(vec![]));
    }
    let rows = file_object::Entity::find()
        .filter(file_object::Column::Id.is_in(fids))
        .filter(file_object::Column::DeletedAt.is_null())
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(FileDto::from).collect()))
}

#[derive(Deserialize, ToSchema)]
pub struct MoveInput {
    #[serde(default)]
    pub org_id: Option<Uuid>,
}

#[utoipa::path(patch, path = "/files/{id}/move", request_body = MoveInput,
    responses((status = 200, body = FileDto), (status = 404)))]
pub async fn move_file(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
    Json(input): Json<MoveInput>,
) -> Result<Json<FileDto>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.owner_id != uid {
        return Err(AppError::NotFound);
    }
    if let Some(org) = input.org_id {
        let ids = user_org_ids(&state.db, uid).await?;
        if !ids.contains(&org) {
            return Err(AppError::Unauthorized);
        }
        enforce_org_quota(&state.db, org, row.size_bytes).await?;
    }
    let mut am: file_object::ActiveModel = row.into();
    am.org_id = Set(input.org_id);
    let updated = am.update(&state.db).await?;
    Ok(Json(FileDto::from(updated)))
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct DescribeInput {
    #[validate(length(max = 4000))]
    pub description: String,
}

#[utoipa::path(patch, path = "/files/{id}/describe", request_body = DescribeInput,
    responses((status = 200, body = FileDto), (status = 404)))]
pub async fn describe(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
    Json(input): Json<DescribeInput>,
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
    am.description = Set(Some(input.description));
    let updated = am.update(&state.db).await?;
    Ok(Json(FileDto::from(updated)))
}
