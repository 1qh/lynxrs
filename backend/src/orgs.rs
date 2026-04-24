use axum::{Json, extract::{Path, State}, http::{HeaderMap, StatusCode}};
use axum_extra::extract::PrivateCookieJar;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    entity::{membership, org, org_invite, user},
    error::{AppError, Result},
    state::AppState,
};
use sha2::Digest;

#[derive(Serialize, ToSchema)]
pub struct OrgDto {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<org::Model> for OrgDto {
    fn from(m: org::Model) -> Self {
        Self { id: m.id, name: m.name, slug: m.slug, created_at: m.created_at }
    }
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct CreateOrgInput {
    #[validate(length(min = 1, max = 80))]
    pub name: String,
    #[validate(length(min = 2, max = 40), regex(path = *SLUG_RE))]
    pub slug: String,
}

use once_cell::sync::Lazy;
static SLUG_RE: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"^[a-z0-9][a-z0-9-]{1,39}$").unwrap());

#[utoipa::path(post, path = "/orgs", request_body = CreateOrgInput,
    responses((status = 201, body = OrgDto), (status = 409)))]
pub async fn create_org(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Json(input): Json<CreateOrgInput>,
) -> Result<(StatusCode, Json<OrgDto>)> {
    input.validate().map_err(|e| AppError::BadRequest(e.to_string()))?;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    if org::Entity::find().filter(org::Column::Slug.eq(&input.slug)).one(&state.db).await?.is_some() {
        return Err(AppError::Conflict("slug already taken".into()));
    }
    let org_id = Uuid::now_v7();
    let now = chrono::Utc::now();
    let m = org::ActiveModel {
        id: Set(org_id),
        name: Set(input.name),
        slug: Set(input.slug),
        created_at: Set(now),
    }.insert(&state.db).await?;
    membership::ActiveModel {
        id: Set(Uuid::now_v7()),
        org_id: Set(org_id),
        user_id: Set(uid),
        role: Set("owner".into()),
        created_at: Set(now),
    }.insert(&state.db).await?;
    Ok((StatusCode::CREATED, Json(OrgDto::from(m))))
}

#[utoipa::path(get, path = "/orgs", responses((status = 200, body = [OrgDto])))]
pub async fn list_orgs(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<Vec<OrgDto>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let mbrs = membership::Entity::find()
        .filter(membership::Column::UserId.eq(uid))
        .all(&state.db)
        .await?;
    if mbrs.is_empty() { return Ok(Json(vec![])); }
    let ids: Vec<Uuid> = mbrs.iter().map(|m| m.org_id).collect();
    let orgs = org::Entity::find()
        .filter(org::Column::Id.is_in(ids))
        .order_by_desc(org::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(orgs.into_iter().map(OrgDto::from).collect()))
}

#[derive(Serialize, ToSchema)]
pub struct MemberDto {
    pub user_id: Uuid,
    pub email: String,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn require_member(db: &sea_orm::DatabaseConnection, org_id: Uuid, uid: Uuid) -> Result<membership::Model> {
    membership::Entity::find()
        .filter(membership::Column::OrgId.eq(org_id))
        .filter(membership::Column::UserId.eq(uid))
        .one(db).await?
        .ok_or(AppError::NotFound)
}

#[utoipa::path(get, path = "/orgs/{id}/members",
    responses((status = 200, body = [MemberDto]), (status = 404)))]
pub async fn list_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<MemberDto>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let _ = require_member(&state.db, id, uid).await?;
    let mbrs = membership::Entity::find()
        .filter(membership::Column::OrgId.eq(id))
        .all(&state.db).await?;
    let user_ids: Vec<Uuid> = mbrs.iter().map(|m| m.user_id).collect();
    let users = user::Entity::find()
        .filter(user::Column::Id.is_in(user_ids))
        .all(&state.db).await?;
    let by_id: std::collections::HashMap<Uuid, String> = users.into_iter().map(|u| (u.id, u.email)).collect();
    Ok(Json(mbrs.into_iter().map(|m| MemberDto {
        user_id: m.user_id,
        email: by_id.get(&m.user_id).cloned().unwrap_or_default(),
        role: m.role,
        created_at: m.created_at,
    }).collect()))
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct AddMemberInput {
    #[validate(email)]
    pub email: String,
    #[serde(default = "default_member_role")]
    pub role: String,
}
fn default_member_role() -> String { "member".into() }

#[utoipa::path(post, path = "/orgs/{id}/members", request_body = AddMemberInput,
    responses((status = 201, body = MemberDto), (status = 404), (status = 409)))]
pub async fn add_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
    Json(input): Json<AddMemberInput>,
) -> Result<(StatusCode, Json<MemberDto>)> {
    input.validate().map_err(|e| AppError::BadRequest(e.to_string()))?;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let caller = require_member(&state.db, id, uid).await?;
    if !matches!(caller.role.as_str(), "owner" | "admin") {
        return Err(AppError::Unauthorized);
    }
    let role = match input.role.as_str() {
        "owner" | "admin" | "member" => input.role,
        _ => return Err(AppError::BadRequest("role must be owner|admin|member".into())),
    };
    let email = input.email.trim().to_lowercase();
    let target = user::Entity::find()
        .filter(user::Column::Email.eq(&email))
        .one(&state.db).await?
        .ok_or_else(|| AppError::NotFound)?;
    if membership::Entity::find()
        .filter(membership::Column::OrgId.eq(id))
        .filter(membership::Column::UserId.eq(target.id))
        .one(&state.db).await?
        .is_some()
    {
        return Err(AppError::Conflict("already a member".into()));
    }
    let m = membership::ActiveModel {
        id: Set(Uuid::now_v7()),
        org_id: Set(id),
        user_id: Set(target.id),
        role: Set(role.clone()),
        created_at: Set(chrono::Utc::now()),
    }.insert(&state.db).await?;
    Ok((StatusCode::CREATED, Json(MemberDto {
        user_id: m.user_id, email, role: m.role, created_at: m.created_at,
    })))
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct InviteInput {
    #[validate(email)]
    pub email: String,
    #[serde(default = "default_member_role")]
    pub role: String,
}

#[derive(Serialize, ToSchema)]
pub struct InviteCreated {
    pub id: Uuid,
    pub url: String,
}

fn sha256_hex(s: &str) -> String {
    hex::encode(sha2::Sha256::digest(s.as_bytes()))
}

fn random_invite_token() -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}

#[utoipa::path(post, path = "/orgs/{id}/invites", request_body = InviteInput,
    responses((status = 201, body = InviteCreated), (status = 404)))]
pub async fn create_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
    Json(input): Json<InviteInput>,
) -> Result<(StatusCode, Json<InviteCreated>)> {
    input.validate().map_err(|e| AppError::BadRequest(e.to_string()))?;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let caller = require_member(&state.db, id, uid).await?;
    if !matches!(caller.role.as_str(), "owner" | "admin") {
        return Err(AppError::Unauthorized);
    }
    let role = match input.role.as_str() {
        "owner" | "admin" | "member" => input.role,
        _ => return Err(AppError::BadRequest("role must be owner|admin|member".into())),
    };
    let raw = random_invite_token();
    let token_hash = sha256_hex(&raw);
    let m = org_invite::ActiveModel {
        id: Set(Uuid::now_v7()),
        org_id: Set(id),
        email: Set(input.email.trim().to_lowercase()),
        role: Set(role),
        token_hash: Set(token_hash),
        expires_at: Set(chrono::Utc::now() + chrono::Duration::days(14)),
        accepted_at: Set(None),
        created_at: Set(chrono::Utc::now()),
    }.insert(&state.db).await?;
    let url = format!(
        "{}/invite?token={raw}",
        state.public_base_url.trim_end_matches('/')
    );
    // Fire-and-forget email
    {
        let mailer = state.mailer.clone();
        let to = m.email.clone();
        let url_c = url.clone();
        tokio::spawn(async move {
            let body = format!("You've been invited to join a simu organization.\n\n  Accept: {url_c}\n\n(This link expires in 14 days.)");
            if let Err(e) = mailer.send_share_link(&to, &url_c, "simu organization invite").await {
                tracing::warn!(error=%e, "invite email failed");
            }
            let _ = body;
        });
    }
    Ok((StatusCode::CREATED, Json(InviteCreated { id: m.id, url })))
}

#[derive(Serialize, ToSchema)]
pub struct InvitePreview {
    pub org: OrgDto,
    pub email: String,
    pub role: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[utoipa::path(get, path = "/invites/{token}",
    responses((status = 200, body = InvitePreview), (status = 404)))]
pub async fn preview_invite(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<InvitePreview>> {
    let hash = sha256_hex(&token);
    let inv = org_invite::Entity::find()
        .filter(org_invite::Column::TokenHash.eq(hash))
        .filter(org_invite::Column::AcceptedAt.is_null())
        .one(&state.db).await?
        .ok_or(AppError::NotFound)?;
    if inv.expires_at < chrono::Utc::now() {
        return Err(AppError::BadRequest("invite expired".into()));
    }
    let o = org::Entity::find_by_id(inv.org_id).one(&state.db).await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(InvitePreview {
        org: OrgDto::from(o), email: inv.email, role: inv.role, expires_at: inv.expires_at,
    }))
}

#[utoipa::path(post, path = "/invites/{token}/accept",
    responses((status = 200, body = MemberDto), (status = 404), (status = 400)))]
pub async fn accept_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(token): Path<String>,
) -> Result<Json<MemberDto>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let hash = sha256_hex(&token);
    let inv = org_invite::Entity::find()
        .filter(org_invite::Column::TokenHash.eq(hash))
        .filter(org_invite::Column::AcceptedAt.is_null())
        .one(&state.db).await?
        .ok_or(AppError::NotFound)?;
    if inv.expires_at < chrono::Utc::now() {
        return Err(AppError::BadRequest("invite expired".into()));
    }
    let u = user::Entity::find_by_id(uid).one(&state.db).await?.ok_or(AppError::Unauthorized)?;
    if u.email.to_lowercase() != inv.email {
        return Err(AppError::BadRequest("invite email mismatch".into()));
    }
    if membership::Entity::find()
        .filter(membership::Column::OrgId.eq(inv.org_id))
        .filter(membership::Column::UserId.eq(uid))
        .one(&state.db).await?
        .is_some()
    {
        return Err(AppError::Conflict("already a member".into()));
    }
    let m = membership::ActiveModel {
        id: Set(Uuid::now_v7()),
        org_id: Set(inv.org_id),
        user_id: Set(uid),
        role: Set(inv.role.clone()),
        created_at: Set(chrono::Utc::now()),
    }.insert(&state.db).await?;
    let mut am: org_invite::ActiveModel = inv.into();
    am.accepted_at = Set(Some(chrono::Utc::now()));
    am.update(&state.db).await?;
    Ok(Json(MemberDto { user_id: m.user_id, email: u.email, role: m.role, created_at: m.created_at }))
}

#[derive(Serialize, ToSchema)]
pub struct OrgStatsDto {
    pub members: u64,
    pub files: u64,
    pub total_bytes: i64,
}

#[utoipa::path(get, path = "/orgs/{id}/stats",
    responses((status = 200, body = OrgStatsDto), (status = 404)))]
pub async fn org_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<OrgStatsDto>> {
    use crate::entity::file_object;
    use sea_orm::PaginatorTrait;
    use sea_orm::QuerySelect;
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let _ = require_member(&state.db, id, uid).await?;
    let members = membership::Entity::find()
        .filter(membership::Column::OrgId.eq(id))
        .count(&state.db).await?;
    let files = file_object::Entity::find()
        .filter(file_object::Column::OrgId.eq(id))
        .filter(file_object::Column::DeletedAt.is_null())
        .count(&state.db).await?;
    let sizes: Vec<i64> = file_object::Entity::find()
        .filter(file_object::Column::OrgId.eq(id))
        .filter(file_object::Column::DeletedAt.is_null())
        .select_only()
        .column(file_object::Column::SizeBytes)
        .into_tuple()
        .all(&state.db)
        .await?;
    let total_bytes: i64 = sizes.iter().sum();
    Ok(Json(OrgStatsDto { members, files, total_bytes }))
}

#[utoipa::path(delete, path = "/orgs/{id}/members/{user_id}",
    responses((status = 204), (status = 404)))]
pub async fn remove_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path((id, target_user)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let caller = require_member(&state.db, id, uid).await?;
    if target_user != uid && !matches!(caller.role.as_str(), "owner" | "admin") {
        return Err(AppError::Unauthorized);
    }
    let m = require_member(&state.db, id, target_user).await?;
    membership::Entity::delete_by_id(m.id).exec(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}
