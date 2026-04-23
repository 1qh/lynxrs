use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use axum_extra::extract::{
    PrivateCookieJar,
    cookie::{Cookie, SameSite},
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    entity::{email_verification, password_reset, user},
    error::{AppError, Result},
    state::AppState,
};

pub const SESSION_COOKIE: &str = "simu_session";
const SESSION_TTL_DAYS: i64 = 30;

#[derive(Deserialize, ToSchema, Validate)]
pub struct SignupInput {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8, max = 128))]
    pub password: String,
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct LoginInput {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1, max = 128))]
    pub password: String,
}

#[derive(Serialize, ToSchema)]
pub struct UserDto {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub email_verified: bool,
}

impl From<user::Model> for UserDto {
    fn from(m: user::Model) -> Self {
        Self {
            id: m.id,
            email: m.email,
            role: m.role,
            email_verified: m.email_verified_at.is_some(),
        }
    }
}

async fn hash_password(password: String) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| AppError::Other(anyhow::anyhow!("hash: {e}")))
    })
    .await
    .map_err(|e| AppError::Other(anyhow::anyhow!("spawn_blocking: {e}")))?
}

async fn verify_password(password: String, phc: String) -> Result<bool> {
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&phc)
            .map_err(|e| AppError::Other(anyhow::anyhow!("parse phc: {e}")))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|e| AppError::Other(anyhow::anyhow!("spawn_blocking: {e}")))?
}

fn issue_cookie(user_id: Uuid) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, user_id.to_string()))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(false) // TODO: true under HTTPS in prod
        .path("/")
        .max_age(time::Duration::days(SESSION_TTL_DAYS))
        .build()
}

#[utoipa::path(post, path = "/auth/signup", request_body = SignupInput,
    responses((status = 201, body = UserDto), (status = 409)))]
pub async fn signup(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Json(input): Json<SignupInput>,
) -> Result<impl IntoResponse> {
    input
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let email = input.email.trim().to_lowercase();
    let existing = user::Entity::find()
        .filter(user::Column::Email.eq(&email))
        .one(&state.db)
        .await?;
    if existing.is_some() {
        return Err(AppError::Conflict("email already registered".into()));
    }

    let now = chrono::Utc::now();
    let id = Uuid::now_v7();
    let password_hash = hash_password(input.password).await?;
    let u = user::ActiveModel {
        id: Set(id),
        email: Set(email),
        password_hash: Set(password_hash),
        role: Set("user".to_string()),
        email_verified_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&state.db)
    .await?;

    // Fire-and-forget email verification — skip failures to keep signup snappy.
    if let Err(e) = enqueue_email_verification(&state, u.id, &u.email).await {
        tracing::warn!(error=%e, "verification email enqueue failed");
    }

    let jar = jar.add(issue_cookie(u.id));
    Ok((StatusCode::CREATED, jar, Json(UserDto::from(u))))
}

async fn enqueue_email_verification(
    state: &AppState,
    user_id: Uuid,
    email: &str,
) -> anyhow::Result<()> {
    let raw = random_url_token();
    let token_hash = sha256_hex(&raw);
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(48);

    email_verification::ActiveModel {
        id: Set(Uuid::now_v7()),
        user_id: Set(user_id),
        token_hash: Set(token_hash),
        expires_at: Set(expires_at),
        used_at: Set(None),
        created_at: Set(chrono::Utc::now()),
    }
    .insert(&state.db)
    .await?;

    let url = format!(
        "{}/verify-email?token={raw}",
        state.public_base_url.trim_end_matches('/')
    );
    state.mailer.send_email_verification(email, &url).await?;
    Ok(())
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct VerifyEmailInput {
    #[validate(length(min = 16, max = 256))]
    pub token: String,
}

#[utoipa::path(
    post,
    path = "/auth/email/verify",
    request_body = VerifyEmailInput,
    responses((status = 204), (status = 400))
)]
pub async fn verify_email(
    State(state): State<AppState>,
    Json(input): Json<VerifyEmailInput>,
) -> Result<StatusCode> {
    input
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let token_hash = sha256_hex(&input.token);

    let row = email_verification::Entity::find()
        .filter(email_verification::Column::TokenHash.eq(&token_hash))
        .one(&state.db)
        .await?
        .ok_or(AppError::BadRequest("invalid token".into()))?;

    if row.used_at.is_some() {
        return Err(AppError::BadRequest("token already used".into()));
    }
    if row.expires_at < chrono::Utc::now() {
        return Err(AppError::BadRequest("token expired".into()));
    }

    let u = user::Entity::find_by_id(row.user_id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut u_am: user::ActiveModel = u.into();
    u_am.email_verified_at = Set(Some(chrono::Utc::now()));
    u_am.updated_at = Set(chrono::Utc::now());
    u_am.update(&state.db).await?;

    let mut used: email_verification::ActiveModel = row.into();
    used.used_at = Set(Some(chrono::Utc::now()));
    used.update(&state.db).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/auth/email/resend",
    responses((status = 202), (status = 401))
)]
pub async fn resend_verification(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> Result<StatusCode> {
    let uid = current_user_id(&jar)?;
    let u = user::Entity::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if u.email_verified_at.is_some() {
        return Ok(StatusCode::ACCEPTED);
    }
    if let Err(e) = enqueue_email_verification(&state, u.id, &u.email).await {
        tracing::warn!(error=%e, "verification email resend failed");
    }
    Ok(StatusCode::ACCEPTED)
}

#[utoipa::path(post, path = "/auth/login", request_body = LoginInput,
    responses((status = 200, body = UserDto), (status = 401)))]
pub async fn login(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Json(input): Json<LoginInput>,
) -> Result<impl IntoResponse> {
    input
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let email = input.email.trim().to_lowercase();
    let u = user::Entity::find()
        .filter(user::Column::Email.eq(&email))
        .one(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !verify_password(input.password, u.password_hash.clone()).await? {
        return Err(AppError::Unauthorized);
    }

    let jar = jar.add(issue_cookie(u.id));
    Ok((jar, Json(UserDto::from(u))))
}

#[utoipa::path(post, path = "/auth/logout", responses((status = 204)))]
pub async fn logout(jar: PrivateCookieJar) -> Result<impl IntoResponse> {
    let jar = jar.remove(Cookie::build(SESSION_COOKIE).path("/").build());
    Ok((StatusCode::NO_CONTENT, jar))
}

#[utoipa::path(get, path = "/auth/me", responses((status = 200, body = UserDto), (status = 401)))]
pub async fn me(State(state): State<AppState>, jar: PrivateCookieJar) -> Result<Json<UserDto>> {
    let uid = current_user_id(&jar)?;
    let u = user::Entity::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(Json(UserDto::from(u)))
}

#[utoipa::path(delete, path = "/auth/me", responses((status = 204), (status = 401)))]
pub async fn delete_me(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> Result<impl IntoResponse> {
    let uid = current_user_id(&jar)?;
    // CASCADE FKs drop file_objects, password_resets, email_verifications.
    user::Entity::delete_by_id(uid).exec(&state.db).await?;
    let jar = jar.remove(Cookie::build(SESSION_COOKIE).path("/").build());
    Ok((StatusCode::NO_CONTENT, jar))
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct ChangePasswordInput {
    #[validate(length(min = 1, max = 128))]
    pub current_password: String,
    #[validate(length(min = 8, max = 128))]
    pub new_password: String,
}

#[utoipa::path(
    post,
    path = "/auth/password/change",
    request_body = ChangePasswordInput,
    responses((status = 204), (status = 401), (status = 400))
)]
pub async fn change_password(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Json(input): Json<ChangePasswordInput>,
) -> Result<impl IntoResponse> {
    input
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let uid = current_user_id(&jar)?;
    let u = user::Entity::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !verify_password(input.current_password, u.password_hash.clone()).await? {
        return Err(AppError::Unauthorized);
    }
    let new_hash = hash_password(input.new_password).await?;

    let mut active: user::ActiveModel = u.into();
    active.password_hash = Set(new_hash);
    active.updated_at = Set(chrono::Utc::now());
    active.update(&state.db).await?;

    // Refresh cookie so the session stays valid post-rotation.
    let jar = jar.add(issue_cookie(uid));
    Ok((StatusCode::NO_CONTENT, jar))
}

pub fn current_user_id(jar: &PrivateCookieJar) -> Result<Uuid> {
    jar.get(SESSION_COOKIE)
        .and_then(|c| Uuid::parse_str(c.value()).ok())
        .ok_or(AppError::Unauthorized)
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct ForgotPasswordInput {
    #[validate(email)]
    pub email: String,
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct ResetPasswordInput {
    #[validate(length(min = 16, max = 256))]
    pub token: String,
    #[validate(length(min = 8, max = 128))]
    pub new_password: String,
}

fn sha256_hex(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex::encode(digest)
}

fn random_url_token() -> String {
    // 32 random bytes → 43-char base64url without padding.
    use argon2::password_hash::rand_core::RngCore;
    let mut buf = [0u8; 32];
    OsRng.fill_bytes(&mut buf);
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    URL_SAFE_NO_PAD.encode(buf)
}

#[utoipa::path(
    post,
    path = "/auth/password/forgot",
    request_body = ForgotPasswordInput,
    responses((status = 202, description = "If the email is registered, a reset link is sent"))
)]
pub async fn forgot_password(
    State(state): State<AppState>,
    Json(input): Json<ForgotPasswordInput>,
) -> Result<StatusCode> {
    input
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let email = input.email.trim().to_lowercase();
    // Always return 202 to avoid email enumeration.
    if let Some(u) = user::Entity::find()
        .filter(user::Column::Email.eq(&email))
        .one(&state.db)
        .await?
    {
        let raw_token = random_url_token();
        let token_hash = sha256_hex(&raw_token);
        let expires_at = chrono::Utc::now() + chrono::Duration::minutes(60);

        password_reset::ActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(u.id),
            token_hash: Set(token_hash),
            expires_at: Set(expires_at),
            used_at: Set(None),
            created_at: Set(chrono::Utc::now()),
        }
        .insert(&state.db)
        .await?;

        let url = format!(
            "{}/reset?token={raw_token}",
            state.public_base_url.trim_end_matches('/')
        );
        if let Err(e) = state.mailer.send_password_reset(&u.email, &url).await {
            tracing::warn!(error=%e, email=%u.email, "password reset mail failed");
        }
    }

    Ok(StatusCode::ACCEPTED)
}

#[utoipa::path(
    post,
    path = "/auth/password/reset",
    request_body = ResetPasswordInput,
    responses((status = 204), (status = 400))
)]
pub async fn reset_password(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Json(input): Json<ResetPasswordInput>,
) -> Result<impl IntoResponse> {
    input
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let token_hash = sha256_hex(&input.token);

    let row = password_reset::Entity::find()
        .filter(password_reset::Column::TokenHash.eq(&token_hash))
        .one(&state.db)
        .await?
        .ok_or(AppError::BadRequest("invalid token".into()))?;

    if row.used_at.is_some() {
        return Err(AppError::BadRequest("token already used".into()));
    }
    if row.expires_at < chrono::Utc::now() {
        return Err(AppError::BadRequest("token expired".into()));
    }

    let new_hash = hash_password(input.new_password).await?;

    let user_id = row.user_id;
    let u = user::Entity::find_by_id(user_id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut u: user::ActiveModel = u.into();
    u.password_hash = Set(new_hash);
    u.updated_at = Set(chrono::Utc::now());
    u.update(&state.db).await?;

    let mut used: password_reset::ActiveModel = row.into();
    used.used_at = Set(Some(chrono::Utc::now()));
    used.update(&state.db).await?;

    let jar = jar.add(issue_cookie(user_id));
    Ok((StatusCode::NO_CONTENT, jar))
}
