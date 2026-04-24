use axum::{Json, extract::State, http::HeaderMap};
use axum_extra::extract::PrivateCookieJar;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{entity::audit_event, error::Result, state::AppState};

pub async fn record(
    db: &DatabaseConnection,
    user_id: Option<Uuid>,
    action: &str,
    headers: Option<&HeaderMap>,
    meta: serde_json::Value,
) {
    let ip = headers
        .and_then(|h| h.get("x-forwarded-for").or(h.get("x-real-ip")))
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let ua = headers
        .and_then(|h| h.get(axum::http::header::USER_AGENT))
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let row = audit_event::ActiveModel {
        id: Set(Uuid::now_v7()),
        user_id: Set(user_id),
        action: Set(action.to_string()),
        ip: Set(ip),
        user_agent: Set(ua),
        meta: Set(meta),
        created_at: Set(chrono::Utc::now()),
    };
    let _ = audit_event::Entity::insert(row).exec(db).await;
}

#[derive(Serialize, ToSchema)]
pub struct AuditDto {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub action: String,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub meta: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[utoipa::path(get, path = "/me/audit", responses((status = 200, body = [AuditDto])))]
pub async fn list_mine(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<Vec<AuditDto>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let rows = audit_event::Entity::find()
        .filter(audit_event::Column::UserId.eq(uid))
        .order_by_desc(audit_event::Column::CreatedAt)
        .limit(100)
        .all(&state.db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| AuditDto {
                id: r.id,
                user_id: r.user_id,
                action: r.action,
                ip: r.ip,
                user_agent: r.user_agent,
                meta: r.meta,
                created_at: r.created_at,
            })
            .collect(),
    ))
}
