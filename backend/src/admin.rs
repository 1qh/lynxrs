use axum::{Json, extract::State};
use axum_extra::extract::PrivateCookieJar;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, Statement};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    audit::AuditDto,
    auth::authenticate,
    entity::{audit_event, file_object, user},
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

#[utoipa::path(
    get,
    path = "/admin/users",
    responses((status = 200, body = [UserSummary]), (status = 401))
)]
pub async fn list_users(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<Vec<UserSummary>>> {
    require_admin(&state, &headers, &jar).await?;

    let rows = user::Entity::find()
        .filter(
            user::Column::Role
                .eq("user")
                .or(user::Column::Role.eq("admin")),
        )
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(UserSummary::from).collect()))
}

#[utoipa::path(
    get,
    path = "/admin/audit",
    responses((status = 200, body = [AuditDto]), (status = 401))
)]
pub async fn audit_all(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<Vec<AuditDto>>> {
    require_admin(&state, &headers, &jar).await?;
    let rows = audit_event::Entity::find()
        .order_by_desc(audit_event::Column::CreatedAt)
        .limit(500)
        .all(&state.db)
        .await?;
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

