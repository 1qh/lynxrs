use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
    http::StatusCode,
};
use axum_extra::extract::PrivateCookieJar;
use hmac::{Hmac, KeyInit, Mac};
use rand::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    entity::{webhook, webhook_delivery},
    error::{AppError, Result},
    events::EventMsg,
    state::AppState,
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
        Self {
            id: m.id,
            url: m.url,
            enabled: m.enabled,
            created_at: m.created_at,
        }
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
pub async fn create_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Json(input): Json<CreateWebhookInput>,
) -> Result<(StatusCode, Json<WebhookCreated>)> {
    input
        .validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let secret = gen_secret();
    let model = webhook::ActiveModel {
        id: Set(Uuid::now_v7()),
        user_id: Set(uid),
        url: Set(input.url),
        secret: Set(secret.clone()),
        enabled: Set(true),
        created_at: Set(chrono::Utc::now()),
        consecutive_failures: Set(0),
    }
    .insert(&state.db)
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(WebhookCreated {
            webhook: model.into(),
            secret,
        }),
    ))
}

#[utoipa::path(get, path = "/webhooks", responses((status = 200, body = [WebhookDto])))]
pub async fn list_webhooks(
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
pub async fn revoke_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let row = webhook::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.user_id != uid {
        return Err(AppError::NotFound);
    }
    webhook::Entity::delete_by_id(id).exec(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
pub struct DeliveryDto {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub attempt: i32,
    pub status: Option<i32>,
    pub duration_ms: Option<i32>,
    pub error: Option<String>,
    pub event_kind: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[utoipa::path(get, path = "/webhooks/{id}/deliveries",
    responses((status = 200, body = [DeliveryDto]), (status = 404)))]
pub async fn list_deliveries(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<DeliveryDto>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let hook = webhook::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if hook.user_id != uid {
        return Err(AppError::NotFound);
    }
    let rows = webhook_delivery::Entity::find()
        .filter(webhook_delivery::Column::WebhookId.eq(id))
        .order_by_desc(webhook_delivery::Column::CreatedAt)
        .limit(200)
        .all(&state.db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| DeliveryDto {
                id: r.id,
                webhook_id: r.webhook_id,
                attempt: r.attempt,
                status: r.status,
                duration_ms: r.duration_ms,
                error: r.error,
                event_kind: r.event_kind,
                created_at: r.created_at,
            })
            .collect(),
    ))
}

#[derive(Serialize, ToSchema)]
pub struct TestResult {
    pub status: Option<i32>,
    pub error: Option<String>,
    pub duration_ms: i32,
}

#[utoipa::path(post, path = "/webhooks/{id}/test",
    responses((status = 200, body = TestResult), (status = 404)))]
pub async fn test_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<TestResult>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let h = webhook::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if h.user_id != uid {
        return Err(AppError::NotFound);
    }
    let body = serde_json::json!({"kind":"test","at_ms":chrono::Utc::now().timestamp_millis()})
        .to_string();
    let mac = HmacSha256::new_from_slice(h.secret.as_bytes()).unwrap();
    let mut m = mac.clone();
    m.update(body.as_bytes());
    let sig = hex::encode(m.finalize().into_bytes());
    let started = std::time::Instant::now();
    let send = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap()
        .post(&h.url)
        .header("content-type", "application/json")
        .header("x-simu-signature", format!("sha256={sig}"))
        .header("x-simu-attempt", "0")
        .body(body.clone())
        .send()
        .await;
    let elapsed = started.elapsed().as_millis() as i32;
    let (status, err) = match &send {
        Ok(r) => (Some(r.status().as_u16() as i32), None),
        Err(e) => (None, Some(e.to_string())),
    };
    let _ = webhook_delivery::ActiveModel {
        id: Set(Uuid::now_v7()),
        webhook_id: Set(h.id),
        attempt: Set(0),
        status: Set(status),
        duration_ms: Set(Some(elapsed)),
        error: Set(err.clone()),
        event_kind: Set("test".to_string()),
        created_at: Set(chrono::Utc::now()),
    }
    .insert(&state.db)
    .await;
    Ok(Json(TestResult {
        status,
        error: err,
        duration_ms: elapsed,
    }))
}

#[utoipa::path(post, path = "/webhooks/{id}/enable",
    responses((status = 204), (status = 404)))]
pub async fn enable_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let h = webhook::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if h.user_id != uid {
        return Err(AppError::NotFound);
    }
    let mut am: webhook::ActiveModel = h.into();
    am.enabled = Set(true);
    am.consecutive_failures = Set(0);
    am.update(&state.db).await?;
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
            let (uid, body, kind) = match &msg {
                EventMsg::FileCreated { owner_id, .. } => (
                    *owner_id,
                    serde_json::to_string(&msg).unwrap_or_default(),
                    "file_created",
                ),
                EventMsg::FileDeleted { owner_id, .. } => (
                    *owner_id,
                    serde_json::to_string(&msg).unwrap_or_default(),
                    "file_deleted",
                ),
                _ => continue,
            };
            let hooks = match webhook::Entity::find()
                .filter(webhook::Column::UserId.eq(uid))
                .filter(webhook::Column::Enabled.eq(true))
                .all(&state.db)
                .await
            {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!(error=%e, "webhook lookup failed");
                    continue;
                }
            };
            for h in hooks {
                let client = client.clone();
                let body = body.clone();
                let db = state.db.clone();
                let kind = kind.to_string();
                tokio::spawn(async move {
                    let mac = HmacSha256::new_from_slice(h.secret.as_bytes()).unwrap();
                    let mut m = mac.clone();
                    m.update(body.as_bytes());
                    let sig = hex::encode(m.finalize().into_bytes());
                    let mut delay_ms = 500u64;
                    let mut ok_once = false;
                    for attempt in 1..=4i32 {
                        let started = std::time::Instant::now();
                        let send = client
                            .post(&h.url)
                            .header("content-type", "application/json")
                            .header("x-simu-signature", format!("sha256={sig}"))
                            .header("x-simu-attempt", attempt.to_string())
                            .body(body.clone())
                            .send()
                            .await;
                        let elapsed = started.elapsed().as_millis() as i32;
                        let (status, err): (Option<i32>, Option<String>) = match &send {
                            Ok(r) => (Some(r.status().as_u16() as i32), None),
                            Err(e) => (None, Some(e.to_string())),
                        };
                        let _ = webhook_delivery::ActiveModel {
                            id: Set(Uuid::now_v7()),
                            webhook_id: Set(h.id),
                            attempt: Set(attempt),
                            status: Set(status),
                            duration_ms: Set(Some(elapsed)),
                            error: Set(err),
                            event_kind: Set(kind.clone()),
                            created_at: Set(chrono::Utc::now()),
                        }
                        .insert(&db)
                        .await;
                        match send {
                            Ok(r) if r.status().is_success() => {
                                metrics::counter!("simu_webhooks_delivered_total").increment(1);
                                tracing::info!(webhook_id=%h.id, attempt, status=%r.status(), "webhook delivered");
                                ok_once = true;
                                break;
                            }
                            Ok(r) => {
                                tracing::warn!(webhook_id=%h.id, attempt, status=%r.status(), "webhook non-2xx")
                            }
                            Err(e) => {
                                tracing::warn!(webhook_id=%h.id, attempt, error=%e, "webhook delivery failed")
                            }
                        }
                        if attempt == 4 {
                            metrics::counter!("simu_webhooks_failed_total").increment(1);
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                        delay_ms *= 2;
                    }
                    // Persist consecutive_failures; auto-disable at 10.
                    let new_count = if ok_once {
                        0
                    } else {
                        h.consecutive_failures + 1
                    };
                    let should_disable = new_count >= 10;
                    let mut am: webhook::ActiveModel = h.clone().into();
                    am.consecutive_failures = Set(new_count);
                    if should_disable {
                        am.enabled = Set(false);
                        tracing::warn!(webhook_id=%h.id, "webhook auto-disabled after 10 consecutive failures");
                    }
                    let _ = am.update(&db).await;
                });
            }
        }
    });
}
