//! Generic OAuth 2.0 "authorization code" flow, provider-agnostic.
//! Configured via env so tests can point at a local mock without patching code.
//!
//! Env:
//!   OAUTH_CLIENT_ID, OAUTH_CLIENT_SECRET, OAUTH_REDIRECT_URL,
//!   OAUTH_AUTHORIZE_URL, OAUTH_TOKEN_URL, OAUTH_USERINFO_URL,
//!   OAUTH_SCOPE (default: "openid email profile")

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::{
    PrivateCookieJar,
    cookie::{Cookie, SameSite},
};
use rand::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use uuid::Uuid;

use crate::{entity::user, error::{AppError, Result}, state::AppState};

#[derive(Clone)]
struct Config {
    client_id: String,
    client_secret: String,
    redirect: String,
    authorize: String,
    token: String,
    userinfo: String,
    scope: String,
}

fn load_config() -> Option<Config> {
    Some(Config {
        client_id: std::env::var("OAUTH_CLIENT_ID").ok()?,
        client_secret: std::env::var("OAUTH_CLIENT_SECRET").ok()?,
        redirect: std::env::var("OAUTH_REDIRECT_URL").ok()?,
        authorize: std::env::var("OAUTH_AUTHORIZE_URL")
            .unwrap_or_else(|_| "https://accounts.google.com/o/oauth2/v2/auth".into()),
        token: std::env::var("OAUTH_TOKEN_URL")
            .unwrap_or_else(|_| "https://oauth2.googleapis.com/token".into()),
        userinfo: std::env::var("OAUTH_USERINFO_URL")
            .unwrap_or_else(|_| "https://www.googleapis.com/oauth2/v2/userinfo".into()),
        scope: std::env::var("OAUTH_SCOPE").unwrap_or_else(|_| "openid email profile".into()),
    })
}

fn random_state() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 16] = rng.random();
    hex::encode(bytes)
}

const OAUTH_STATE_COOKIE: &str = "simu_oauth_state";

pub async fn start(jar: PrivateCookieJar) -> Result<(PrivateCookieJar, Redirect)> {
    let cfg = load_config().ok_or_else(|| AppError::BadRequest("OAuth not configured".into()))?;
    let state = random_state();
    let mut parsed = reqwest::Url::parse(&cfg.authorize)
        .map_err(|e| AppError::Other(anyhow::anyhow!("authorize url: {e}")))?;
    parsed
        .query_pairs_mut()
        .append_pair("client_id", &cfg.client_id)
        .append_pair("redirect_uri", &cfg.redirect)
        .append_pair("response_type", "code")
        .append_pair("scope", &cfg.scope)
        .append_pair("state", &state);
    let url = parsed.to_string();
    let cookie = Cookie::build((OAUTH_STATE_COOKIE, state))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(false)
        .path("/")
        .max_age(time::Duration::minutes(10))
        .build();
    Ok((jar.add(cookie), Redirect::temporary(&url)))
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
}

#[derive(Deserialize)]
struct TokenResp {
    access_token: String,
}

#[derive(Deserialize)]
struct UserInfo {
    email: String,
}

pub async fn callback(
    State(app): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Query(q): Query<CallbackQuery>,
) -> Result<Response> {
    let cfg = load_config().ok_or_else(|| AppError::BadRequest("OAuth not configured".into()))?;
    let saved = jar
        .get(OAUTH_STATE_COOKIE)
        .ok_or_else(|| AppError::BadRequest("missing state cookie".into()))?;
    if saved.value() != q.state {
        return Err(AppError::BadRequest("state mismatch".into()));
    }

    let client = reqwest::Client::new();
    let token: TokenResp = client
        .post(&cfg.token)
        .form(&[
            ("code", q.code.as_str()),
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("redirect_uri", cfg.redirect.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("token exchange: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::Other(anyhow::anyhow!("token non-2xx: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("token parse: {e}")))?;

    let info: UserInfo = client
        .get(&cfg.userinfo)
        .bearer_auth(&token.access_token)
        .send()
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("userinfo: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::Other(anyhow::anyhow!("userinfo non-2xx: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("userinfo parse: {e}")))?;

    let email = info.email.trim().to_lowercase();
    let now = chrono::Utc::now();

    let u = if let Some(existing) = user::Entity::find()
        .filter(user::Column::Email.eq(&email))
        .one(&app.db)
        .await?
    {
        existing
    } else {
        // Create account; random-hashed placeholder password (user can reset).
        let placeholder_hash = crate::auth::hash_password_sync("oauth_placeholder_please_reset".into()).await?;
        user::ActiveModel {
            id: Set(Uuid::now_v7()),
            email: Set(email.clone()),
            password_hash: Set(placeholder_hash),
            role: Set("user".into()),
            email_verified_at: Set(Some(now)), // OAuth provider already verified email
            session_version: Set(0),
            created_at: Set(now),
            updated_at: Set(now),
            totp_secret: Set(None),
            totp_enabled: Set(false),
            failed_login_count: Set(0),
            locked_until: Set(None),
            display_name: Set(None),
            avatar_url: Set(None),
        }
        .insert(&app.db)
        .await?
    };

    crate::audit::record(&app.db, Some(u.id), "login_oauth", Some(&headers), serde_json::json!({"email": email})).await;
    metrics::counter!("simu_login_success_total").increment(1);

    let jar = jar
        .remove(Cookie::build(OAUTH_STATE_COOKIE).path("/").build())
        .add(crate::auth::issue_cookie_public(u.id, u.session_version));

    // Redirect to frontend root.
    let base = app.public_base_url.trim_end_matches('/').to_string();
    let redir = Redirect::temporary(&base);
    Ok((jar, redir).into_response())
}

pub fn router() -> axum::Router<AppState> {
    use axum::routing::get;
    axum::Router::new()
        .route("/auth/oauth/google/start", get(start))
        .route("/auth/oauth/google/callback", get(callback))
        .route("/auth/oauth/status", get(config_status))
}

pub async fn config_status() -> impl IntoResponse {
    let configured = load_config().is_some();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::json!({ "google": configured }).to_string(),
    )
}
