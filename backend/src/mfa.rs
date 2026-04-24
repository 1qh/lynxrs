use axum::{Json, extract::State, http::HeaderMap, http::StatusCode};
use axum_extra::extract::PrivateCookieJar;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use totp_rs::{Algorithm, Secret, TOTP};
use utoipa::ToSchema;

use crate::{entity::{mfa_recovery, user}, error::{AppError, Result}, state::AppState};
use rand::Rng;
use sea_orm::{ColumnTrait, QueryFilter};
use sha2::Digest;

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
    metrics::counter!("simu_mfa_activated_total").increment(1);
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

fn hash_code(c: &str) -> String {
    hex::encode(sha2::Sha256::digest(c.as_bytes()))
}

fn gen_recovery_code() -> String {
    let mut rng = rand::rng();
    // 10 chars, alphanumeric upper.
    (0..10)
        .map(|_| {
            let n: u32 = rng.random_range(0..36);
            if n < 10 { (b'0' + n as u8) as char } else { (b'A' + (n - 10) as u8) as char }
        })
        .collect()
}

#[derive(Serialize, ToSchema)]
pub struct RecoveryCodesDto {
    pub codes: Vec<String>,
}

#[utoipa::path(post, path = "/mfa/recovery-codes",
    responses((status = 200, body = RecoveryCodesDto), (status = 400)))]
pub async fn generate_recovery_codes(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<RecoveryCodesDto>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let u = user::Entity::find_by_id(uid).one(&state.db).await?.ok_or(AppError::Unauthorized)?;
    if !u.totp_enabled {
        return Err(AppError::BadRequest("activate MFA first".into()));
    }
    // Invalidate old codes.
    mfa_recovery::Entity::delete_many()
        .filter(mfa_recovery::Column::UserId.eq(uid))
        .exec(&state.db)
        .await?;
    let mut codes = Vec::with_capacity(10);
    for _ in 0..10 {
        let code = gen_recovery_code();
        mfa_recovery::ActiveModel {
            id: Set(uuid::Uuid::now_v7()),
            user_id: Set(uid),
            code_hash: Set(hash_code(&code)),
            used_at: Set(None),
            created_at: Set(chrono::Utc::now()),
        }
        .insert(&state.db)
        .await?;
        codes.push(code);
    }
    crate::audit::record(&state.db, Some(uid), "mfa_recovery_generated", Some(&headers), serde_json::json!({})).await;
    Ok(Json(RecoveryCodesDto { codes }))
}

/// Consume a recovery code. Returns true if matched an unused code (and marked it used).
pub async fn consume_recovery(
    db: &sea_orm::DatabaseConnection,
    uid: uuid::Uuid,
    code: &str,
) -> Result<bool> {
    let hash = hash_code(code);
    let Some(row) = mfa_recovery::Entity::find()
        .filter(mfa_recovery::Column::UserId.eq(uid))
        .filter(mfa_recovery::Column::CodeHash.eq(hash))
        .filter(mfa_recovery::Column::UsedAt.is_null())
        .one(db)
        .await?
    else {
        return Ok(false);
    };
    let mut am: mfa_recovery::ActiveModel = row.into();
    am.used_at = Set(Some(chrono::Utc::now()));
    am.update(db).await?;
    Ok(true)
}
