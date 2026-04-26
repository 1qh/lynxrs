use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use axum_extra::extract::PrivateCookieJar;
use object_store::{ObjectStoreExt, PutPayload, path::Path as ObjPath};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    entity::file_object,
    error::{AppError, Result},
    events::EventMsg,
    state::AppState,
};

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

/// Object-store key generator. Files in an org live under `o/{org}/{file_id}`;
/// personal files under `u/{uid}/{file_id}`. Keep this the single source of
/// truth — move_file uses it to compute the new prefix when changing org_id.
pub fn storage_key_for(uid: Uuid, org_id: Option<Uuid>, file_id: Uuid) -> String {
    match org_id {
        Some(org) => format!("o/{org}/{file_id}"),
        None => format!("u/{uid}/{file_id}"),
    }
}
const ORG_QUOTA_BYTES: i64 = 5 * 1024 * 1024 * 1024; // 5 GB per org

pub fn image_magic_ok(claimed: &str, data: &[u8]) -> bool {
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

pub(crate) async fn enforce_org_quota(
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

pub mod meta;
#[allow(unused_imports)]
pub use meta::*;

pub mod uploads;
#[allow(unused_imports)]
pub use uploads::*;

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
    let now = chrono::Utc::now();
    let mut am: file_object::ActiveModel = row.into();
    am.deleted_at = Set(Some(now));
    am.update(&state.db).await?;
    // Cascade: revoke active shares so the public link stops working.
    use crate::entity::file_share;
    let _ = file_share::Entity::update_many()
        .filter(file_share::Column::FileId.eq(id))
        .filter(file_share::Column::RevokedAt.is_null())
        .col_expr(
            file_share::Column::RevokedAt,
            sea_orm::sea_query::Expr::value(Some(now)),
        )
        .exec(&state.db)
        .await;
    let _ = state.bus.send(EventMsg::FileDeleted {
        file_id: id,
        owner_id: uid,
    });
    Ok(StatusCode::NO_CONTENT)
}

pub mod thumb;
#[allow(unused_imports)]
pub use thumb::*;

pub mod versions;
#[allow(unused_imports)]
pub use versions::*;

pub mod bulk;
#[allow(unused_imports)]
pub use bulk::*;

pub mod tags;
#[allow(unused_imports)]
pub use tags::*;

pub mod trash;
#[allow(unused_imports)]
pub use trash::*;

pub mod presign;
#[allow(unused_imports)]
pub use presign::*;

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

pub fn parse_range(raw: &str, total: u64) -> Option<(u64, u64)> {
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

pub mod shares;
#[allow(unused_imports)]
pub use shares::*;

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

pub mod comments;
#[allow(unused_imports)]
pub use comments::*;

pub mod stars;
#[allow(unused_imports)]
pub use stars::*;

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
    // Rewrite storage_key prefix when crossing org boundary so files end up
    // under the right namespace (`o/{org}/...` for org-owned, `u/{uid}/...`
    // for personal). Copy then delete the old key — object_store doesn't have
    // a primitive rename.
    let new_key = storage_key_for(uid, input.org_id, id);
    let key_changed = new_key != row.storage_key;
    if key_changed {
        let old = ObjPath::from(row.storage_key.clone());
        let bytes = state
            .storage
            .get(&old)
            .await?
            .bytes()
            .await
            .map_err(|e| AppError::Other(anyhow::anyhow!("get for move: {e}")))?;
        state
            .storage
            .put(
                &ObjPath::from(new_key.clone()),
                PutPayload::from_bytes(bytes),
            )
            .await?;
        let _ = state.storage.delete(&old).await;
    }
    let mut am: file_object::ActiveModel = row.into();
    am.org_id = Set(input.org_id);
    if key_changed {
        am.storage_key = Set(new_key);
    }
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
