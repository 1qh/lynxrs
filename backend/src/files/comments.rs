use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use axum_extra::extract::PrivateCookieJar;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use super::can_access;
use crate::{
    entity::{file_comment, file_object},
    error::{AppError, Result},
    state::AppState,
};

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
    if c.user_id != uid && file.owner_id != uid {
        return Err(AppError::NotFound);
    }
    file_comment::Entity::delete_by_id(id)
        .exec(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
