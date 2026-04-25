use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use axum_extra::extract::PrivateCookieJar;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QuerySelect, Set};
use uuid::Uuid;

use super::{FileDto, can_access};
use crate::{
    entity::{file_object, file_star},
    error::{AppError, Result},
    state::AppState,
};

#[utoipa::path(post, path = "/files/{id}/star", responses((status = 204), (status = 404)))]
pub async fn star_file(
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
