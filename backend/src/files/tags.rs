use axum::{
    Json,
    extract::{Path, State},
};
use axum_extra::extract::PrivateCookieJar;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use super::FileDto;
use crate::{
    entity::file_object,
    error::{AppError, Result},
    state::AppState,
};

#[derive(Deserialize, ToSchema, Validate)]
pub struct TagInput {
    #[validate(length(min = 1, max = 40))]
    pub tag: String,
}

#[utoipa::path(post, path = "/files/{id}/tags", request_body = TagInput,
    responses((status = 200, body = FileDto), (status = 404)))]
pub async fn add_tag(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
    Json(input): Json<TagInput>,
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
    let mut tags = row.tags.clone();
    if !tags.contains(&input.tag) {
        tags.push(input.tag);
    }
    let mut am: file_object::ActiveModel = row.into();
    am.tags = Set(tags);
    let updated = am.update(&state.db).await?;
    Ok(Json(FileDto::from(updated)))
}

#[utoipa::path(delete, path = "/files/{id}/tags/{tag}",
    responses((status = 200, body = FileDto), (status = 404)))]
pub async fn remove_tag(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path((id, tag)): Path<(Uuid, String)>,
) -> Result<Json<FileDto>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = file_object::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.owner_id != uid {
        return Err(AppError::NotFound);
    }
    let tags: Vec<String> = row.tags.iter().filter(|t| **t != tag).cloned().collect();
    let mut am: file_object::ActiveModel = row.into();
    am.tags = Set(tags);
    let updated = am.update(&state.db).await?;
    Ok(Json(FileDto::from(updated)))
}
