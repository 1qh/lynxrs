use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use axum_extra::extract::PrivateCookieJar;
use object_store::{ObjectStoreExt, path::Path as ObjPath};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::can_access;

/// Parse an HTTP Range header `bytes=start-end` (or `bytes=start-`) into (start, end)
/// inclusive byte offsets. Returns None for unsupported / invalid forms.
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
use crate::{
    entity::file_object,
    error::{AppError, Result},
    state::AppState,
};

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
