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
    #[serde(default)]
    pub totp_code: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct UserDto {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub email_verified: bool,
    pub totp_enabled: bool,
}

impl From<user::Model> for UserDto {
    fn from(m: user::Model) -> Self {
        Self {
            id: m.id,
            email: m.email,
            role: m.role,
            email_verified: m.email_verified_at.is_some(),
            totp_enabled: m.totp_enabled,
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

fn issue_cookie(user_id: Uuid, session_version: i32) -> Cookie<'static> {
    let value = format!("{user_id}:{session_version}");
    Cookie::build((SESSION_COOKIE, value))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(false) // TODO: true under HTTPS in prod
        .path("/")
        .max_age(time::Duration::days(SESSION_TTL_DAYS))
        .build()
}

fn parse_session_cookie(jar: &PrivateCookieJar) -> Option<(Uuid, i32)> {
    let cookie = jar.get(SESSION_COOKIE)?;
    let value = cookie.value();
    let (uid, v) = value.split_once(':')?;
    Some((Uuid::parse_str(uid).ok()?, v.parse().ok()?))
}

#[utoipa::path(post, path = "/auth/signup", request_body = SignupInput,
    responses((status = 201, body = UserDto), (status = 409)))]
pub async fn signup(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
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
        session_version: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        totp_secret: Set(None),
        totp_enabled: Set(false),
        failed_login_count: Set(0),
        locked_until: Set(None),
    }
    .insert(&state.db)
    .await?;

    // Fire-and-forget email verification — skip failures to keep signup snappy.
    if let Err(e) = enqueue_email_verification(&state, u.id, &u.email).await {
        tracing::warn!(error=%e, "verification email enqueue failed");
    }

    crate::audit::record(&state.db, Some(u.id), "signup", Some(&headers), serde_json::json!({})).await;
    let jar = jar.add(issue_cookie(u.id, u.session_version));
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
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<StatusCode> {
    let uid = authenticate(&state, &headers, &jar).await?;
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
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Json(input): Json<LoginInput>,
) -> Result<impl IntoResponse> {
    input
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let email = input.email.trim().to_lowercase();
    let u = match user::Entity::find()
        .filter(user::Column::Email.eq(&email))
        .one(&state.db)
        .await?
    {
        Some(u) => u,
        None => {
            crate::audit::record(&state.db, None, "login_failed", Some(&headers), serde_json::json!({"email": email})).await;
            return Err(AppError::Unauthorized);
        }
    };

    if let Some(until) = u.locked_until
        && until > chrono::Utc::now()
    {
        crate::audit::record(&state.db, Some(u.id), "login_locked", Some(&headers), serde_json::json!({"until": until})).await;
        return Err(AppError::Unauthorized);
    }

    let password_ok = verify_password(input.password, u.password_hash.clone()).await?;
    let mfa_ok = if !password_ok {
        false
    } else if u.totp_enabled {
        let code = input.totp_code.as_deref().unwrap_or("");
        if code.is_empty() {
            false
        } else if crate::mfa::verify_for(&u, code)? {
            true
        } else {
            crate::mfa::consume_recovery(&state.db, u.id, code).await?
        }
    } else {
        true
    };

    if !password_ok || !mfa_ok {
        let new_count = u.failed_login_count + 1;
        let lock = if new_count >= 5 {
            Some(chrono::Utc::now() + chrono::Duration::minutes(15))
        } else {
            u.locked_until
        };
        let uid_for_audit = u.id;
        let mut am: user::ActiveModel = u.into();
        am.failed_login_count = Set(new_count);
        am.locked_until = Set(lock);
        am.updated_at = Set(chrono::Utc::now());
        let _ = am.update(&state.db).await;
        let action = if !password_ok { "login_failed" } else { "login_failed_mfa" };
        crate::audit::record(&state.db, Some(uid_for_audit), action, Some(&headers), serde_json::json!({"count": new_count})).await;
        return Err(AppError::Unauthorized);
    }

    // Success: reset counters.
    if u.failed_login_count != 0 || u.locked_until.is_some() {
        let uid_copy = u.id;
        let sv = u.session_version;
        let mut am: user::ActiveModel = u.clone().into();
        am.failed_login_count = Set(0);
        am.locked_until = Set(None);
        am.updated_at = Set(chrono::Utc::now());
        let _ = am.update(&state.db).await;
        crate::audit::record(&state.db, Some(uid_copy), "login", Some(&headers), serde_json::json!({})).await;
        let jar = jar.add(issue_cookie(uid_copy, sv));
        return Ok((jar, Json(UserDto::from(u))));
    }

    crate::audit::record(&state.db, Some(u.id), "login", Some(&headers), serde_json::json!({})).await;
    let jar = jar.add(issue_cookie(u.id, u.session_version));
    Ok((jar, Json(UserDto::from(u))))
}

#[utoipa::path(post, path = "/auth/logout", responses((status = 204)))]
pub async fn logout(jar: PrivateCookieJar) -> Result<impl IntoResponse> {
    let jar = jar.remove(Cookie::build(SESSION_COOKIE).path("/").build());
    Ok((StatusCode::NO_CONTENT, jar))
}

#[utoipa::path(get, path = "/auth/me", responses((status = 200, body = UserDto), (status = 401)))]
pub async fn me(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<UserDto>> {
    let uid = authenticate(&state, &headers, &jar).await?;
    let u = user::Entity::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(Json(UserDto::from(u)))
}

#[utoipa::path(delete, path = "/auth/me", responses((status = 204), (status = 401)))]
pub async fn delete_me(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<impl IntoResponse> {
    let uid = authenticate(&state, &headers, &jar).await?;
    // CASCADE FKs drop file_objects, password_resets, email_verifications.
    crate::audit::record(&state.db, Some(uid), "delete_account", Some(&headers), serde_json::json!({})).await;
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
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    Json(input): Json<ChangePasswordInput>,
) -> Result<impl IntoResponse> {
    input
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let uid = authenticate(&state, &headers, &jar).await?;
    let u = user::Entity::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !verify_password(input.current_password, u.password_hash.clone()).await? {
        return Err(AppError::Unauthorized);
    }
    let new_hash = hash_password(input.new_password).await?;
    let new_version = u.session_version + 1;

    let mut active: user::ActiveModel = u.into();
    active.password_hash = Set(new_hash);
    active.session_version = Set(new_version);
    active.updated_at = Set(chrono::Utc::now());
    active.update(&state.db).await?;

    crate::audit::record(&state.db, Some(uid), "password_change", Some(&headers), serde_json::json!({})).await;
    // Refresh cookie with bumped version — old cookies instantly invalid.
    let jar = jar.add(issue_cookie(uid, new_version));
    Ok((StatusCode::NO_CONTENT, jar))
}

#[utoipa::path(post, path = "/auth/logout-all", responses((status = 204), (status = 401)))]
pub async fn logout_all(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
) -> Result<impl IntoResponse> {
    let uid = authenticate(&state, &headers, &jar).await?;
    let u = user::Entity::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let mut am: user::ActiveModel = u.into();
    am.session_version = Set(am.session_version.unwrap() + 1);
    am.updated_at = Set(chrono::Utc::now());
    am.update(&state.db).await?;
    crate::audit::record(&state.db, Some(uid), "logout_all", Some(&headers), serde_json::json!({})).await;
    let jar = jar.remove(Cookie::build(SESSION_COOKIE).path("/").build());
    Ok((StatusCode::NO_CONTENT, jar))
}

/// Verify the session cookie: parse `user_id:version`, look up the user, compare versions.
/// Returns the authoritative user_id on success.
pub async fn current_user_id(state: &AppState, jar: &PrivateCookieJar) -> Result<Uuid> {
    let (uid, v) = parse_session_cookie(jar).ok_or(AppError::Unauthorized)?;
    let u = user::Entity::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if u.session_version != v {
        return Err(AppError::Unauthorized);
    }
    Ok(uid)
}

/// Accept cookie session OR `Authorization: Bearer simu_<plaintext>` API token.
/// Updates the token's last_used_at on each call.
pub async fn authenticate(
    state: &AppState,
    headers: &axum::http::HeaderMap,
    jar: &PrivateCookieJar,
) -> Result<Uuid> {
    // 1) Bearer token
    if let Some(hdr) = headers.get(axum::http::header::AUTHORIZATION)
        && let Ok(hdr_s) = hdr.to_str()
        && let Some(token) = hdr_s.strip_prefix("Bearer ")
    {
        let token_hash = {
            let digest = sha2::Sha256::digest(token.as_bytes());
            hex::encode(digest)
        };
        use crate::entity::api_token;
        if let Some(row) = api_token::Entity::find()
            .filter(api_token::Column::TokenHash.eq(&token_hash))
            .one(&state.db)
            .await?
        {
            if row.revoked_at.is_some() {
                return Err(AppError::Unauthorized);
            }
            // Fire-and-forget last_used_at update
            let uid = row.user_id;
            let mut am: api_token::ActiveModel = row.into();
            am.last_used_at = Set(Some(chrono::Utc::now()));
            let _ = am.update(&state.db).await;
            return Ok(uid);
        }
    }
    // 2) Cookie session
    current_user_id(state, jar).await
}

/// Cookie-only parse — used rarely (e.g., WS upgrade before DB access). Does NOT verify version.
pub fn current_user_id_unchecked(jar: &PrivateCookieJar) -> Result<Uuid> {
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
    _headers: axum::http::HeaderMap,
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
    let new_version = u.session_version + 1;
    let mut u_am: user::ActiveModel = u.into();
    u_am.password_hash = Set(new_hash);
    u_am.session_version = Set(new_version);
    u_am.updated_at = Set(chrono::Utc::now());
    u_am.update(&state.db).await?;

    let mut used: password_reset::ActiveModel = row.into();
    used.used_at = Set(Some(chrono::Utc::now()));
    used.update(&state.db).await?;

    let jar = jar.add(issue_cookie(user_id, new_version));
    Ok((StatusCode::NO_CONTENT, jar))
}
