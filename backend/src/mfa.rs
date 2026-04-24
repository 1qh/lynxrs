use axum::{Json, extract::State, http::HeaderMap, http::StatusCode};
use axum_extra::extract::PrivateCookieJar;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use totp_rs::{Algorithm, Secret, TOTP};
use utoipa::ToSchema;

use crate::{entity::user, error::{AppError, Result}, state::AppState};

const ISSUER: &str = "Simu";

fn build_totp(secret_b32: &str, account: &str) -> Result<TOTP> {
    let secret = Secret::Encoded(secret_b32.to_string())
        .to_bytes()
        .map_err(|e| AppError::Other(anyhow::anyhow!(format!("totp secret: {e:?}"))))?;
    TOTP::new(Algorithm::SHA1, 6, 1, 30, secret, Some(ISSUER.to_string()), account.to_string())
        .map_err(|e| AppError::Other(anyhow::anyhow!(format!("totp build: {e:?}"))))
}

#[derive(Serialize, ToSchema)]
pub struct EnrollDto {
    pub secret: String,
    pub otpauth_url: String,
}

#[utoipa::path(post, path = "/mfa/enroll", responses((status = 200, body = EnrollDto), (status = 409)))]
pub async fn enroll(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<EnrollDto>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let u = user::Entity::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if u.totp_enabled {
        return Err(AppError::Conflict("MFA already enabled".into()));
    }
    let secret_b32 = Secret::generate_secret().to_encoded().to_string();
    let totp = build_totp(&secret_b32, &u.email)?;
    let url = totp.get_url();
    let mut am: user::ActiveModel = u.into();
    am.totp_secret = Set(Some(secret_b32.clone()));
    am.updated_at = Set(chrono::Utc::now());
    am.update(&state.db).await?;
    Ok(Json(EnrollDto { secret: secret_b32, otpauth_url: url }))
}

#[derive(Deserialize, ToSchema)]
pub struct MfaCodeInput {
    pub code: String,
}

#[utoipa::path(post, path = "/mfa/activate", request_body = MfaCodeInput,
    responses((status = 204), (status = 400), (status = 409)))]
pub async fn activate(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Json(input): Json<MfaCodeInput>,
) -> Result<StatusCode> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let u = user::Entity::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if u.totp_enabled {
        return Err(AppError::Conflict("MFA already active".into()));
    }
    let secret = u.totp_secret.clone().ok_or_else(|| AppError::BadRequest("call /mfa/enroll first".into()))?;
    let totp = build_totp(&secret, &u.email)?;
    if !totp.check_current(&input.code).map_err(|e| AppError::Other(anyhow::anyhow!(format!("totp: {e:?}"))))? {
        return Err(AppError::BadRequest("invalid code".into()));
    }
    let mut am: user::ActiveModel = u.into();
    am.totp_enabled = Set(true);
    am.updated_at = Set(chrono::Utc::now());
    am.update(&state.db).await?;
    crate::audit::record(&state.db, Some(uid), "mfa_activated", Some(&headers), serde_json::json!({})).await;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/mfa/disable", request_body = MfaCodeInput,
    responses((status = 204), (status = 400)))]
pub async fn disable(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Json(input): Json<MfaCodeInput>,
) -> Result<StatusCode> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let u = user::Entity::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if !u.totp_enabled {
        return Err(AppError::BadRequest("MFA not active".into()));
    }
    let secret = u.totp_secret.clone().ok_or_else(|| AppError::Other(anyhow::anyhow!("missing secret")))?;
    let totp = build_totp(&secret, &u.email)?;
    if !totp.check_current(&input.code).map_err(|e| AppError::Other(anyhow::anyhow!(format!("totp: {e:?}"))))? {
        return Err(AppError::BadRequest("invalid code".into()));
    }
    let mut am: user::ActiveModel = u.into();
    am.totp_enabled = Set(false);
    am.totp_secret = Set(None);
    am.updated_at = Set(chrono::Utc::now());
    am.update(&state.db).await?;
    crate::audit::record(&state.db, Some(uid), "mfa_disabled", Some(&headers), serde_json::json!({})).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Helper used by login: verify TOTP for a user.
pub fn verify_for(user: &user::Model, code: &str) -> Result<bool> {
    let Some(secret) = user.totp_secret.as_deref() else {
        return Ok(false);
    };
    let totp = build_totp(secret, &user.email)?;
    totp.check_current(code)
        .map_err(|e| AppError::Other(anyhow::anyhow!(format!("totp: {e:?}"))))
}
