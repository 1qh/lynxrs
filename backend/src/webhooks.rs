use axum::{Json, extract::{Path, State}, http::HeaderMap, http::StatusCode};
use axum_extra::extract::PrivateCookieJar;
use hmac::{Hmac, Mac};
use rand::Rng;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    entity::webhook, error::{AppError, Result}, events::EventMsg, state::AppState,
};

type HmacSha256 = Hmac<Sha256>;

#[derive(Serialize, ToSchema)]
pub struct WebhookDto {
    pub id: Uuid,
    pub url: String,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<webhook::Model> for WebhookDto {
    fn from(m: webhook::Model) -> Self {
        Self { id: m.id, url: m.url, enabled: m.enabled, created_at: m.created_at }
    }
}

#[derive(Serialize, ToSchema)]
pub struct WebhookCreated {
    pub webhook: WebhookDto,
    pub secret: String,
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct CreateWebhookInput {
    #[validate(url, length(max = 512))]
    pub url: String,
}

fn gen_secret() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    hex::encode(bytes)
}

#[utoipa::path(post, path = "/webhooks", request_body = CreateWebhookInput,
    responses((status = 201, body = WebhookCreated)))]
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Json(input): Json<CreateWebhookInput>,
) -> Result<(StatusCode, Json<WebhookCreated>)> {
    input.validate().map_err(|e| AppError::BadRequest(e.to_string()))?;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let secret = gen_secret();
    let model = webhook::ActiveModel {
        id: Set(Uuid::now_v7()),
        user_id: Set(uid),
        url: Set(input.url),
        secret: Set(secret.clone()),
        enabled: Set(true),
        created_at: Set(chrono::Utc::now()),
    }
    .insert(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(WebhookCreated { webhook: model.into(), secret })))
}

#[utoipa::path(get, path = "/webhooks", responses((status = 200, body = [WebhookDto])))]
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<Vec<WebhookDto>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let rows = webhook::Entity::find()
        .filter(webhook::Column::UserId.eq(uid))
        .order_by_desc(webhook::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(WebhookDto::from).collect()))
}

#[utoipa::path(delete, path = "/webhooks/{id}", responses((status = 204), (status = 404)))]
pub async fn revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = webhook::Entity::find_by_id(id).one(&state.db).await?.ok_or(AppError::NotFound)?;
    if row.user_id != uid { return Err(AppError::NotFound); }
    webhook::Entity::delete_by_id(id).exec(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn spawn_dispatcher(state: AppState) {
    let mut rx = state.bus.subscribe();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("http client");
    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let (uid, body) = match &msg {
                EventMsg::FileCreated { owner_id, .. } => {
                    let body = serde_json::to_string(&msg).unwrap_or_default();
                    (*owner_id, body)
                }
                _ => continue,
            };
            let hooks = match webhook::Entity::find()
                .filter(webhook::Column::UserId.eq(uid))
                .filter(webhook::Column::Enabled.eq(true))
                .all(&state.db)
                .await
            {
                Ok(h) => h,
                Err(e) => { tracing::warn!(error=%e, "webhook lookup failed"); continue }
            };
            for h in hooks {
                let mac = HmacSha256::new_from_slice(h.secret.as_bytes()).unwrap();
                let mut m = mac.clone();
                m.update(body.as_bytes());
                let sig = hex::encode(m.finalize().into_bytes());
                let send = client
                    .post(&h.url)
                    .header("content-type", "application/json")
                    .header("x-simu-signature", format!("sha256={sig}"))
                    .body(body.clone())
                    .send()
                    .await;
                match send {
                    Ok(r) => tracing::info!(webhook_id=%h.id, status=%r.status(), "webhook delivered"),
                    Err(e) => tracing::warn!(webhook_id=%h.id, error=%e, "webhook delivery failed"),
                }
            }
        }
    });
}
