use axum::{
    extract::{Path, State},
    response::IntoResponse,
};
use axum_extra::extract::PrivateCookieJar;
use bytes::Bytes;
use object_store::{ObjectStoreExt, PutPayload, path::Path as ObjPath};
use sea_orm::EntityTrait;
use uuid::Uuid;

use super::can_access;
use crate::{
    entity::file_object,
    error::{AppError, Result},
    state::AppState,
};

pub(crate) fn thumb_key(file_id: Uuid, uid: Uuid) -> String {
    format!("u/{uid}/thumb/{file_id}.jpg")
}

pub(crate) async fn maybe_store_thumbnail(
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
