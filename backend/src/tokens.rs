//! API token management — `/api/tokens` CRUD.

use axum::{Json, extract::{Path, State}, http::StatusCode, response::IntoResponse};
use axum_extra::extract::PrivateCookieJar;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{

    entity::api_token,
    error::{AppError, Result},
    state::AppState,
};

#[derive(Serialize, ToSchema)]
pub struct ApiTokenDto {
    pub id: Uuid,
    pub name: String,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<api_token::Model> for ApiTokenDto {
    fn from(m: api_token::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            last_used_at: m.last_used_at,
            created_at: m.created_at,
            revoked_at: m.revoked_at,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct ApiTokenCreated {
    pub token: ApiTokenDto,
    /// The only time the plaintext token is returned — store it now.
    pub plaintext: String,
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct CreateTokenInput {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
}

fn sha256_hex(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex::encode(digest)
}

fn random_plaintext_token() -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    let mut buf = [0u8; 32];
    OsRng.fill_bytes(&mut buf);
    format!("simu_{}", URL_SAFE_NO_PAD.encode(buf))
}

#[utoipa::path(
    post,
    path = "/tokens",
    request_body = CreateTokenInput,
    responses((status = 201, body = ApiTokenCreated), (status = 401))
)]
pub async fn create(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Json(input): Json<CreateTokenInput>,
) -> Result<impl IntoResponse> {
    input
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;

    let plaintext = random_plaintext_token();
    let token_hash = sha256_hex(&plaintext);

    let model = api_token::ActiveModel {
        id: Set(Uuid::now_v7()),
        user_id: Set(uid),
        name: Set(input.name),
        token_hash: Set(token_hash),
        last_used_at: Set(None),
        created_at: Set(chrono::Utc::now()),
        revoked_at: Set(None),
    }
    .insert(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiTokenCreated {
            token: ApiTokenDto::from(model),
            plaintext,
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/tokens",
    responses((status = 200, body = [ApiTokenDto]), (status = 401))
)]
pub async fn list(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<Vec<ApiTokenDto>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let rows = api_token::Entity::find()
        .filter(api_token::Column::UserId.eq(uid))
        .order_by_desc(api_token::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(ApiTokenDto::from).collect()))
}

#[utoipa::path(
    delete,
    path = "/tokens/{id}",
    responses((status = 204), (status = 404), (status = 401))
)]
pub async fn revoke(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = api_token::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.user_id != uid {
        return Err(AppError::NotFound);
    }
    let mut am: api_token::ActiveModel = row.into();
    am.revoked_at = Set(Some(chrono::Utc::now()));
    am.update(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}
