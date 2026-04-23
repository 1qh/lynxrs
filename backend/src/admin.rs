use axum::{Json, extract::State};
use axum_extra::extract::PrivateCookieJar;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, Statement};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    auth::authenticate,
    entity::{file_object, user},
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
