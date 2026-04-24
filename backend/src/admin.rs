use axum::{Json, extract::State};
use axum_extra::extract::PrivateCookieJar;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, Statement};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    audit::AuditDto,
    auth::authenticate,
    entity::{audit_event, file_object, org, user},
    error::{AppError, Result},
    state::AppState,
};

#[derive(Serialize, ToSchema)]
pub struct AdminStats {
    pub users: u64,
    pub files: u64,
    pub total_bytes: i64,
}

async fn require_admin(
    state: &AppState,
    headers: &axum::http::HeaderMap,
    jar: &PrivateCookieJar,
) -> Result<()> {
    let uid = authenticate(state, headers, jar).await?;
    let u = user::Entity::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if u.role != "admin" {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/admin/stats",
    responses((status = 200, body = AdminStats), (status = 401))
)]
pub async fn stats(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<AdminStats>> {
    require_admin(&state, &headers, &jar).await?;

    let users = user::Entity::find().count(&state.db).await?;
    let files = file_object::Entity::find().count(&state.db).await?;

    let sum_row = state
        .db
        .query_one(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT COALESCE(SUM(size_bytes), 0)::bigint AS total FROM file_objects".to_string(),
        ))
        .await?;
    let total_bytes = sum_row
        .and_then(|r| r.try_get::<i64>("", "total").ok())
        .unwrap_or(0);

    Ok(Json(AdminStats {
        users,
        files,
        total_bytes,
    }))
}

#[derive(Serialize, ToSchema)]
pub struct UserSummary {
    pub id: uuid::Uuid,
    pub email: String,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<user::Model> for UserSummary {
    fn from(m: user::Model) -> Self {
        Self {
            id: m.id,
            email: m.email,
            role: m.role,
            created_at: m.created_at,
        }
    }
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct UserSearchQuery {
    pub q: Option<String>,
}

#[utoipa::path(
    get,
    path = "/admin/users",
    params(UserSearchQuery),
    responses((status = 200, body = [UserSummary]), (status = 401))
)]
pub async fn list_users(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    axum::extract::Query(q): axum::extract::Query<UserSearchQuery>,
) -> Result<Json<Vec<UserSummary>>> {
    require_admin(&state, &headers, &jar).await?;

    let rows = user::Entity::find()
        .filter(
            user::Column::Role
                .eq("user")
                .or(user::Column::Role.eq("admin")),
        )
        .limit(500)
        .all(&state.db)
        .await?;
    let filtered: Vec<UserSummary> = if let Some(s) = q.q.as_ref().filter(|s| !s.is_empty()) {
        let n = s.to_lowercase();
        rows.into_iter()
            .filter(|r| r.email.to_lowercase().contains(&n))
            .map(UserSummary::from)
            .collect()
    } else {
        rows.into_iter().map(UserSummary::from).collect()
    };
    Ok(Json(filtered))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct AdminAuditQuery {
    pub action: Option<String>,
    pub user_id: Option<uuid::Uuid>,
}

#[utoipa::path(
    get,
    path = "/admin/audit",
    params(AdminAuditQuery),
    responses((status = 200, body = [AuditDto]), (status = 401))
)]
pub async fn audit_all(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    axum::extract::Query(q): axum::extract::Query<AdminAuditQuery>,
) -> Result<Json<Vec<AuditDto>>> {
    require_admin(&state, &headers, &jar).await?;
    let mut qb = audit_event::Entity::find()
        .order_by_desc(audit_event::Column::CreatedAt)
        .limit(500);
    if let Some(a) = q.action.as_ref().filter(|s| !s.is_empty()) {
        qb = qb.filter(audit_event::Column::Action.eq(a.as_str()));
    }
    if let Some(u) = q.user_id {
        qb = qb.filter(audit_event::Column::UserId.eq(u));
    }
    let rows = qb.all(&state.db).await?;
    Ok(Json(audit_to_dtos(rows)))
}

fn audit_to_dtos(rows: Vec<audit_event::Model>) -> Vec<AuditDto> {
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
        .collect()
}

#[utoipa::path(
    get,
    path = "/admin/audit.csv",
    responses((status = 200, content_type = "text/csv"), (status = 401))
)]
pub async fn audit_csv(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<axum::response::Response> {
    use axum::response::IntoResponse;
    require_admin(&state, &headers, &jar).await?;
    let rows = audit_event::Entity::find()
        .order_by_desc(audit_event::Column::CreatedAt)
        .limit(10000)
        .all(&state.db)
        .await?;
    let mut out = String::from("id,user_id,action,ip,user_agent,created_at\n");
    for r in rows {
        fn esc(s: &str) -> String {
            if s.contains(',') || s.contains('"') || s.contains('\n') {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_string()
            }
        }
        out.push_str(&format!(
            "{},{},{},{},{},{}\n",
            r.id,
            r.user_id.map(|u| u.to_string()).unwrap_or_default(),
            esc(&r.action),
            esc(r.ip.as_deref().unwrap_or("")),
            esc(r.user_agent.as_deref().unwrap_or("")),
            r.created_at.to_rfc3339(),
        ));
    }
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "text/csv"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=\"audit.csv\"",
            ),
        ],
        out,
    )
        .into_response())
}

#[derive(Deserialize, ToSchema)]
pub struct SetRoleInput {
    pub role: String,
}

#[utoipa::path(
    post,
    path = "/admin/users/{id}/role",
    request_body = SetRoleInput,
    responses((status = 204), (status = 400), (status = 401))
)]
pub async fn set_role(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
    Json(input): Json<SetRoleInput>,
) -> Result<axum::http::StatusCode> {
    require_admin(&state, &headers, &jar).await?;
    if !matches!(input.role.as_str(), "admin" | "user") {
        return Err(AppError::BadRequest("role must be admin|user".into()));
    }
    let u = user::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut am: user::ActiveModel = u.into();
    am.role = Set(input.role);
    am.updated_at = Set(chrono::Utc::now());
    am.update(&state.db).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/admin/users/{id}",
    responses((status = 204), (status = 401), (status = 404))
)]
pub async fn delete_user(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<axum::http::StatusCode> {
    require_admin(&state, &headers, &jar).await?;
    let res = user::Entity::delete_by_id(id).exec(&state.db).await?;
    if res.rows_affected == 0 {
        return Err(AppError::NotFound);
    }
    crate::audit::record(&state.db, None, "admin_user_deleted", Some(&headers), serde_json::json!({"user_id": id})).await;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/admin/users/{id}/lock", responses((status = 204), (status = 404)))]
pub async fn lock_user(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<axum::http::StatusCode> {
    require_admin(&state, &headers, &jar).await?;
    let u = user::Entity::find_by_id(id).one(&state.db).await?.ok_or(AppError::NotFound)?;
    let mut am: user::ActiveModel = u.into();
    am.locked_until = Set(Some(chrono::Utc::now() + chrono::Duration::days(3650)));
    am.session_version = Set(am.session_version.unwrap() + 1);
    am.updated_at = Set(chrono::Utc::now());
    am.update(&state.db).await?;
    crate::audit::record(&state.db, None, "admin_user_locked", Some(&headers), serde_json::json!({"user_id": id})).await;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/admin/backup",
    responses((status = 200), (status = 401)))]
pub async fn backup(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<axum::Json<serde_json::Value>> {
    require_admin(&state, &headers, &jar).await?;
    let db_url = std::env::var("DATABASE_URL").map_err(|_| AppError::Other(anyhow::anyhow!("DATABASE_URL missing")))?;
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let key = format!("backups/{ts}.sql.gz");

    let out = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(format!("pg_dump --no-owner --no-privileges --format=plain '{db_url}' | gzip -9"))
        .output()
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("pg_dump spawn: {e}")))?;
    if !out.status.success() {
        return Err(AppError::Other(anyhow::anyhow!(
            "pg_dump failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    let size = out.stdout.len() as i64;
    use object_store::ObjectStoreExt;
    state
        .storage
        .put(&object_store::path::Path::from(key.clone()), object_store::PutPayload::from(bytes::Bytes::from(out.stdout)))
        .await?;
    crate::audit::record(&state.db, None, "admin_backup", Some(&headers), serde_json::json!({"key": key, "size": size})).await;
    Ok(axum::Json(serde_json::json!({"key": key, "size_bytes": size})))
}

#[utoipa::path(post, path = "/admin/users/{id}/impersonate",
    responses((status = 200), (status = 404)))]
pub async fn impersonate(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<axum::response::Response> {
    use axum::response::IntoResponse;
    require_admin(&state, &headers, &jar).await?;
    let target = user::Entity::find_by_id(id)
        .one(&state.db).await?.ok_or(AppError::NotFound)?;
    let admin_uid = crate::auth::current_user_id(&state, &jar).await.ok();
    crate::audit::record(
        &state.db,
        admin_uid,
        "admin_impersonate",
        Some(&headers),
        serde_json::json!({"target_user_id": id}),
    ).await;
    let jar = jar.add(crate::auth::issue_cookie_public(target.id, target.session_version));
    Ok((
        jar,
        axum::Json(serde_json::json!({"impersonating": target.email})),
    ).into_response())
}

#[utoipa::path(post, path = "/admin/users/{id}/unlock", responses((status = 204), (status = 404)))]
pub async fn unlock_user(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<axum::http::StatusCode> {
    require_admin(&state, &headers, &jar).await?;
    let u = user::Entity::find_by_id(id).one(&state.db).await?.ok_or(AppError::NotFound)?;
    let mut am: user::ActiveModel = u.into();
    am.locked_until = Set(None);
    am.failed_login_count = Set(0);
    am.updated_at = Set(chrono::Utc::now());
    am.update(&state.db).await?;
    crate::audit::record(&state.db, None, "admin_user_unlocked", Some(&headers), serde_json::json!({"user_id": id})).await;
    Ok(axum::http::StatusCode::NO_CONTENT)
}


#[utoipa::path(get, path = "/admin/orgs", responses((status = 200), (status = 401)))]
pub async fn list_all_orgs(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<axum::Json<Vec<crate::orgs::OrgDto>>> {
    require_admin(&state, &headers, &jar).await?;
    let rows = org::Entity::find()
        .order_by_desc(org::Column::CreatedAt)
        .all(&state.db).await?;
    Ok(axum::Json(rows.into_iter().map(crate::orgs::OrgDto::from).collect()))
}

#[utoipa::path(get, path = "/admin/webhooks",
    responses((status = 200), (status = 401)))]
pub async fn list_all_webhooks(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<axum::Json<Vec<crate::webhooks::WebhookDto>>> {
    use crate::entity::webhook;
    require_admin(&state, &headers, &jar).await?;
    let rows = webhook::Entity::find()
        .order_by_desc(webhook::Column::CreatedAt)
        .all(&state.db).await?;
    Ok(axum::Json(rows.into_iter().map(crate::webhooks::WebhookDto::from).collect()))
}

#[derive(Serialize, ToSchema)]
pub struct UserDetail {
    pub id: uuid::Uuid,
    pub email: String,
    pub role: String,
    pub display_name: Option<String>,
    pub email_verified: bool,
    pub totp_enabled: bool,
    pub locked: bool,
    pub failed_login_count: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub orgs: Vec<uuid::Uuid>,
    pub files_count: u64,
}

#[utoipa::path(get, path = "/admin/users/{id}/detail",
    responses((status = 200, body = UserDetail), (status = 404)))]
pub async fn user_detail(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<Json<UserDetail>> {
    require_admin(&state, &headers, &jar).await?;
    let u = user::Entity::find_by_id(id).one(&state.db).await?.ok_or(AppError::NotFound)?;
    use crate::entity::{file_object as fo, membership};
    let orgs: Vec<uuid::Uuid> = membership::Entity::find()
        .filter(membership::Column::UserId.eq(id))
        .select_only()
        .column(membership::Column::OrgId)
        .into_tuple()
        .all(&state.db).await?;
    let files_count = fo::Entity::find()
        .filter(fo::Column::OwnerId.eq(id))
        .filter(fo::Column::DeletedAt.is_null())
        .count(&state.db).await?;
    Ok(Json(UserDetail {
        id: u.id,
        email: u.email,
        role: u.role,
        display_name: u.display_name,
        email_verified: u.email_verified_at.is_some(),
        totp_enabled: u.totp_enabled,
        locked: u.locked_until.map(|t| t > chrono::Utc::now()).unwrap_or(false),
        failed_login_count: u.failed_login_count,
        created_at: u.created_at,
        updated_at: u.updated_at,
        orgs,
        files_count,
    }))
}
