use axum::{Json, extract::State, http::HeaderMap};
use axum_extra::extract::PrivateCookieJar;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, Statement, TransactionTrait,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{entity::audit_event, error::Result, state::AppState};

/// Canonical serialization for hashing. Stable field order, no whitespace.
fn canonical_row(
    id: &Uuid,
    user_id: &Option<Uuid>,
    action: &str,
    ip: &Option<String>,
    ua: &Option<String>,
    meta: &serde_json::Value,
    created_at: &chrono::DateTime<chrono::Utc>,
) -> String {
    let v = serde_json::json!({
        "id": id,
        "user_id": user_id,
        "action": action,
        "ip": ip,
        "user_agent": ua,
        "meta": meta,
        "created_at": created_at.to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
    });
    serde_json::to_string(&v).unwrap_or_default()
}

fn hash_chain(prev: &str, canonical: &str) -> String {
    let mut h = Sha256::new();
    h.update(prev.as_bytes());
    h.update(b"|");
    h.update(canonical.as_bytes());
    hex::encode(h.finalize())
}

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
    let id = Uuid::now_v7();
    let created_at = chrono::Utc::now();

    // Tx + xact advisory lock serializes chain growth under concurrent writes.
    let res = async {
        let tx = db.begin().await?;
        tx.execute(Statement::from_string(
            tx.get_database_backend(),
            "SELECT pg_advisory_xact_lock(4242424242)".to_string(),
        ))
        .await?;
        let prev = audit_event::Entity::find()
            .order_by_desc(audit_event::Column::CreatedAt)
            .order_by_desc(audit_event::Column::Id)
            .limit(1)
            .one(&tx)
            .await?
            .and_then(|r| r.row_hash)
            .unwrap_or_else(|| "GENESIS".to_string());
        let canonical = canonical_row(&id, &user_id, action, &ip, &ua, &meta, &created_at);
        let row_hash = hash_chain(&prev, &canonical);
        let row = audit_event::ActiveModel {
            id: Set(id),
            user_id: Set(user_id),
            action: Set(action.to_string()),
            ip: Set(ip),
            user_agent: Set(ua),
            meta: Set(meta),
            created_at: Set(created_at),
            prev_hash: Set(Some(prev)),
            row_hash: Set(Some(row_hash)),
        };
        audit_event::Entity::insert(row).exec(&tx).await?;
        tx.commit().await?;
        Ok::<_, sea_orm::DbErr>(())
    }
    .await;
    if let Err(e) = res {
        tracing::warn!(error = %e, "audit record failed");
    }
}

#[derive(Serialize, ToSchema)]
pub struct AuditVerifyReport {
    pub total: u64,
    pub verified: u64,
    pub broken_at: Option<Uuid>,
    pub ok: bool,
}

#[utoipa::path(get, path = "/admin/audit/verify", responses((status = 200, body = AuditVerifyReport)))]
pub async fn verify_chain(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<AuditVerifyReport>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let u = crate::entity::user::Entity::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or(crate::error::AppError::Unauthorized)?;
    if u.role != "admin" {
        return Err(crate::error::AppError::Unauthorized);
    }
    let rows = audit_event::Entity::find()
        .order_by_asc(audit_event::Column::CreatedAt)
        .order_by_asc(audit_event::Column::Id)
        .all(&state.db)
        .await?;
    let total = rows.len() as u64;
    let mut prev = "GENESIS".to_string();
    let mut verified = 0u64;
    let mut broken_at = None;
    for r in &rows {
        // Legacy rows without row_hash are skipped from the chain (pre-migration).
        let Some(got) = r.row_hash.clone() else {
            verified += 1;
            continue;
        };
        let stored_prev = r.prev_hash.clone().unwrap_or_default();
        let canonical = canonical_row(
            &r.id,
            &r.user_id,
            &r.action,
            &r.ip,
            &r.user_agent,
            &r.meta,
            &r.created_at,
        );
        let expected = hash_chain(&prev, &canonical);
        if stored_prev != prev || expected != got {
            broken_at = Some(r.id);
            break;
        }
        verified += 1;
        prev = got;
    }
    let ok = broken_at.is_none();
    Ok(Json(AuditVerifyReport {
        total,
        verified,
        broken_at,
        ok,
    }))
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

#[utoipa::path(get, path = "/me/sessions", responses((status = 200, body = [AuditDto])))]
pub async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<Vec<AuditDto>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let rows = audit_event::Entity::find()
        .filter(audit_event::Column::UserId.eq(uid))
        .filter(audit_event::Column::Action.eq("login"))
        .order_by_desc(audit_event::Column::CreatedAt)
        .limit(50)
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

#[derive(serde::Deserialize, utoipa::IntoParams)]
pub struct AuditQuery {
    pub limit: Option<u64>,
    pub cursor: Option<chrono::DateTime<chrono::Utc>>,
}

#[utoipa::path(get, path = "/me/audit", params(AuditQuery),
    responses((status = 200, body = [AuditDto])))]
pub async fn list_mine(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    axum::extract::Query(q): axum::extract::Query<AuditQuery>,
) -> Result<Json<Vec<AuditDto>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let limit = q.limit.unwrap_or(100).min(500);
    let mut qb = audit_event::Entity::find()
        .filter(audit_event::Column::UserId.eq(uid))
        .order_by_desc(audit_event::Column::CreatedAt)
        .limit(limit);
    if let Some(c) = q.cursor {
        qb = qb.filter(audit_event::Column::CreatedAt.lt(c));
    }
    let rows = qb.all(&state.db).await?;
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
