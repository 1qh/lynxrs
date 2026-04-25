//! Public factory: build an Axum Router + OpenAPI spec from an AppState.
//! Shared by the bin entry point and the integration test harness.

use std::sync::Arc;

use axum::{Json, Router, http::HeaderName, routing::get};
use axum_prometheus::PrometheusMetricLayerBuilder;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};

async fn inject_request_id_into_errors(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    // Capture request id from request extensions/headers set by SetRequestIdLayer.
    let req_id = req
        .extensions()
        .get::<tower_http::request_id::RequestId>()
        .and_then(|id| id.header_value().to_str().ok().map(String::from))
        .or_else(|| {
            req.headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .map(String::from)
        });
    let res = next.run(req).await;
    let status = res.status();
    let is_json = res
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.starts_with("application/json"))
        .unwrap_or(false);
    if !status.is_client_error() && !status.is_server_error() || !is_json {
        return res;
    }
    let (parts, body) = res.into_parts();
    // Error JSON is always small; buffer without a ceiling. If we ever exceed
    // something absurd, surface as an empty body and bump a counter.
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => {
            metrics::counter!("simu_err_body_rewrite_failed_total").increment(1);
            return axum::response::Response::from_parts(parts, axum::body::Body::empty());
        }
    };
    let mut val: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            return axum::response::Response::from_parts(parts, axum::body::Body::from(bytes));
        }
    };
    if let (Some(obj), Some(id)) = (val.as_object_mut(), req_id) {
        obj.entry("request_id")
            .or_insert(serde_json::Value::String(id));
    }
    let new_body = serde_json::to_vec(&val).unwrap_or_else(|_| bytes.to_vec());
    axum::response::Response::from_parts(parts, axum::body::Body::from(new_body))
}

async fn security_headers(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut res = next.run(req).await;
    let is_html = res
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.starts_with("text/html"))
        .unwrap_or(false);
    let h = res.headers_mut();
    h.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        axum::http::HeaderValue::from_static("nosniff"),
    );
    h.insert(
        axum::http::header::X_FRAME_OPTIONS,
        axum::http::HeaderValue::from_static("DENY"),
    );
    h.insert(
        axum::http::header::REFERRER_POLICY,
        axum::http::HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        axum::http::header::STRICT_TRANSPORT_SECURITY,
        axum::http::HeaderValue::from_static("max-age=63072000; includeSubDomains"),
    );
    h.insert(
        HeaderName::from_static("permissions-policy"),
        axum::http::HeaderValue::from_static("interest-cohort=(), geolocation=()"),
    );
    let csp = if is_html {
        // /docs pulls Scalar from cdn.jsdelivr.net; tighten everything else.
        "default-src 'self'; \
         script-src 'self' https://cdn.jsdelivr.net 'unsafe-inline'; \
         style-src 'self' 'unsafe-inline'; \
         img-src 'self' data: blob:; \
         font-src 'self' data:; \
         connect-src 'self'; \
         object-src 'none'; \
         base-uri 'self'; \
         frame-ancestors 'none'"
    } else {
        // API responses (JSON, octet-stream, etc.) — strictest possible.
        "default-src 'none'; frame-ancestors 'none'"
    };
    h.insert(
        HeaderName::from_static("content-security-policy"),
        axum::http::HeaderValue::from_str(csp).unwrap(),
    );
    res
}
use tracing::Level;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    admin, audit, auth, error::ErrorBody, events, files, mfa, oauth, orgs, state::AppState, tokens,
    webhooks,
};

const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

#[derive(OpenApi)]
#[openapi(
    components(schemas(
        ErrorBody,
        auth::SignupInput,
        auth::LoginInput,
        auth::UserDto,
        auth::ForgotPasswordInput,
        auth::ResetPasswordInput,
        auth::ChangePasswordInput,
        auth::ProfileInput,
        auth::VerifyEmailInput,
        files::FileDto,
        files::FileList,
        files::FileShareDto,
        files::QuotaDto,
        files::MeStatsDto,
        files::RenameInput,
        files::VerifyDto,
        files::TagInput,
        files::BulkAction,
        files::BulkResult,
        files::CommentDto,
        files::CommentInput,
        files::ZipDownloadInput,
        files::MoveInput,
        files::DescribeInput,
        files::PresignedDto,
        files::PresignUploadInput,
        files::PresignUploadDto,
        files::VersionDto,
        files::CreateShareInput,
        files::Base64UploadInput,
        events::EventMsg,
        admin::AdminStats,
        admin::UserSummary,
        tokens::ApiTokenDto,
        tokens::ApiTokenCreated,
        tokens::CreateTokenInput,
        audit::AuditDto,
        orgs::OrgDto,
        orgs::CreateOrgInput,
        orgs::MemberDto,
        orgs::AddMemberInput,
        orgs::InviteInput,
        orgs::InviteCreated,
        orgs::InvitePreview,
        orgs::OrgStatsDto,
        admin::SetRoleInput,
        mfa::EnrollDto,
        mfa::MfaCodeInput,
        mfa::RecoveryCodesDto,
        webhooks::WebhookDto,
        webhooks::WebhookCreated,
        webhooks::CreateWebhookInput,
        webhooks::DeliveryDto,
        webhooks::TestResult,
    )),
    tags(
        (name = "auth", description = "Authentication, MFA, sessions"),
        (name = "files", description = "File CRUD, upload, download, versions, tags"),
        (name = "shares", description = "Public share links"),
        (name = "webhooks", description = "Outbound webhooks + deliveries"),
        (name = "orgs", description = "Organizations, memberships, invites"),
        (name = "admin", description = "Admin-only: users, audit, backup"),
        (name = "events", description = "WebSocket / SSE event streams"),
        (name = "me", description = "Current user — profile, quota, stats, export"),
    )
)]
pub struct ApiDoc;

#[derive(Clone, Copy, Debug)]
pub struct BuildOpts {
    pub rate_limit_rps: u64,
    pub rate_limit_burst: u32,
    /// If false, skip the governor + metrics + trace layers (useful for tests).
    pub production_layers: bool,
}

impl Default for BuildOpts {
    fn default() -> Self {
        Self {
            rate_limit_rps: 10,
            rate_limit_burst: 30,
            production_layers: true,
        }
    }
}

async fn api_docs_html() -> axum::response::Response {
    use axum::response::IntoResponse;
    let html = r#"<!doctype html>
<html>
<head>
  <title>Simu API</title>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
</head>
<body>
  <script
    id="api-reference"
    data-url="/api-docs/openapi.json"
    data-configuration='{"theme":"default","hideDownloadButton":false}'
  ></script>
  <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
</body>
</html>"#;
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}

async fn health() -> axum::response::Response {
    use axum::response::IntoResponse;
    (
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(serde_json::json!({ "status": "ok" })),
    )
        .into_response()
}

async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "rustc": option_env!("VERGEN_RUSTC_SEMVER").unwrap_or("unknown"),
    }))
}

async fn not_found(uri: axum::http::Uri) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "code": "not_found",
            "message": format!("no route for {}", uri.path()),
        })),
    )
}

/// Readiness check — verifies the DB is reachable. Returns 503 on failure.
/// Kubernetes/Docker can split liveness (/health) from readiness (/ready).
async fn ready(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    use sea_orm::ConnectionTrait;
    let db_ok = state
        .db
        .query_one(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT 1".to_string(),
        ))
        .await
        .is_ok();
    let s3_ok = {
        use futures::StreamExt;
        let mut stream = state
            .storage
            .list(Some(&object_store::path::Path::from("__healthcheck__")));
        match stream.next().await {
            Some(Ok(_)) | None => true,
            Some(Err(_)) => false,
        }
    };
    let smtp_ok = {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let host = std::env::var("SMTP_HOST").unwrap_or_else(|_| "localhost".into());
        let port: u16 = std::env::var("SMTP_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(25);
        let probe = async {
            let mut s = tokio::net::TcpStream::connect((host.as_str(), port))
                .await
                .ok()?;
            let mut buf = [0u8; 256];
            let n = s.read(&mut buf).await.ok()?;
            if !buf[..n].starts_with(b"220") {
                return None;
            }
            s.write_all(b"EHLO simu.local\r\n").await.ok()?;
            let n = s.read(&mut buf).await.ok()?;
            if !buf[..n].starts_with(b"250") {
                return None;
            }
            let _ = s.write_all(b"QUIT\r\n").await;
            Some(())
        };
        tokio::time::timeout(std::time::Duration::from_millis(1000), probe)
            .await
            .ok()
            .flatten()
            .is_some()
    };
    let all_ok = db_ok && s3_ok;
    let body = serde_json::json!({
        "db": if db_ok { "ok" } else { "fail" },
        "s3": if s3_ok { "ok" } else { "fail" },
        "smtp": if smtp_ok { "ok" } else { "fail" },
    });
    (
        if all_ok {
            axum::http::StatusCode::OK
        } else {
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        },
        Json(body),
    )
        .into_response()
}

pub fn build(state: AppState, opts: BuildOpts) -> Router {
    let (api_router, openapi) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(auth::signup))
        .routes(routes!(auth::login))
        .routes(routes!(auth::logout))
        .routes(routes!(auth::logout_all))
        .routes(routes!(auth::me, auth::delete_me, auth::update_me))
        .routes(routes!(auth::forgot_password))
        .routes(routes!(auth::reset_password))
        .routes(routes!(auth::change_password))
        .routes(routes!(auth::verify_email))
        .routes(routes!(auth::resend_verification))
        .routes(routes!(files::upload))
        .routes(routes!(files::upload_json))
        .routes(routes!(files::list))
        .routes(routes!(files::quota))
        .routes(routes!(files::me_stats))
        .routes(routes!(files::me_export))
        .routes(routes!(files::me_import))
        .routes(routes!(files::verify))
        .routes(routes!(files::presign))
        .routes(routes!(files::presign_upload))
        .routes(routes!(files::confirm_upload))
        .routes(routes!(files::list_versions, files::create_version))
        .routes(routes!(files::restore_version))
        .routes(routes!(files::thumbnail))
        .routes(routes!(files::list_trash, files::empty_trash))
        .routes(routes!(files::restore))
        .routes(routes!(files::purge))
        .routes(routes!(files::add_tag))
        .routes(routes!(files::remove_tag))
        .routes(routes!(files::bulk))
        .routes(routes!(files::list_comments, files::add_comment))
        .routes(routes!(files::delete_comment))
        .routes(routes!(files::download_zip))
        .routes(routes!(files::star_file, files::unstar_file))
        .routes(routes!(files::list_starred))
        .routes(routes!(files::move_file))
        .routes(routes!(files::describe))
        .routes(routes!(files::download, files::rename, files::head_file))
        .routes(routes!(files::delete))
        .routes(routes!(files::create_share, files::list_shares))
        .routes(routes!(files::download_share))
        .routes(routes!(files::revoke_share))
        .routes(routes!(admin::stats))
        .routes(routes!(admin::list_users))
        .routes(routes!(admin::audit_all))
        .routes(routes!(admin::audit_csv))
        .routes(routes!(admin::set_role))
        .routes(routes!(admin::delete_user))
        .routes(routes!(admin::lock_user))
        .routes(routes!(admin::unlock_user))
        .routes(routes!(admin::impersonate))
        .routes(routes!(admin::backup))
        .routes(routes!(admin::list_all_orgs))
        .routes(routes!(admin::list_all_webhooks))
        .routes(routes!(admin::user_detail))
        .routes(routes!(tokens::create_token, tokens::list_tokens))
        .routes(routes!(tokens::revoke_token))
        .routes(routes!(audit::list_mine))
        .routes(routes!(audit::list_sessions))
        .routes(routes!(audit::verify_chain))
        .routes(routes!(orgs::create_org, orgs::list_orgs))
        .routes(routes!(orgs::list_members, orgs::add_member))
        .routes(routes!(orgs::remove_member))
        .routes(routes!(orgs::create_invite))
        .routes(routes!(orgs::preview_invite))
        .routes(routes!(orgs::accept_invite))
        .routes(routes!(orgs::org_stats))
        .routes(routes!(mfa::enroll))
        .routes(routes!(mfa::activate))
        .routes(routes!(mfa::disable))
        .routes(routes!(mfa::generate_recovery_codes))
        .routes(routes!(webhooks::create_webhook, webhooks::list_webhooks))
        .routes(routes!(webhooks::revoke_webhook))
        .routes(routes!(webhooks::list_deliveries))
        .routes(routes!(webhooks::test_webhook))
        .routes(routes!(webhooks::enable_webhook))
        .with_state(state.clone())
        .split_for_parts();

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_credentials(true)
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
            axum::http::header::AUTHORIZATION,
            axum::http::header::COOKIE,
            HeaderName::from_static("x-request-id"),
            HeaderName::from_static("x-csrf-token"),
        ])
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::PATCH,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .expose_headers([
            axum::http::header::SET_COOKIE,
            HeaderName::from_static("x-request-id"),
        ]);

    if opts.production_layers {
        let governor_conf = Arc::new(
            GovernorConfigBuilder::default()
                .per_second(opts.rate_limit_rps)
                .burst_size(opts.rate_limit_burst)
                .finish()
                .expect("governor config"),
        );

        let (prometheus_layer, metric_handle) = PrometheusMetricLayerBuilder::new()
            .with_prefix("simu")
            .with_default_metrics()
            .build_pair();

        let ready_state = state.clone();
        let scope_state = state.clone();
        let rate_state = auth::build_rate_state(state.db.clone(), state.cookie_key.clone());
        let limited = Router::new()
            .nest("/api", api_router)
            .nest("/api", oauth::router().with_state(state.clone()))
            .merge(events::router().with_state(state))
            .layer(axum::middleware::from_fn_with_state(
                rate_state,
                auth::per_user_rate_limit,
            ))
            .layer(axum::middleware::from_fn(inject_request_id_into_errors))
            .layer(axum::middleware::from_fn(auth::csrf_enforce))
            .layer(axum::middleware::from_fn_with_state(
                scope_state,
                auth::token_scope_enforce,
            ))
            .layer(GovernorLayer::new(governor_conf).error_handler(|err| {
                use axum::response::IntoResponse;
                let (status, code) = match err {
                    tower_governor::GovernorError::TooManyRequests { .. } => {
                        (axum::http::StatusCode::TOO_MANY_REQUESTS, "rate_limited")
                    }
                    _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal"),
                };
                (
                    status,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    Json(serde_json::json!({"code": code, "message": err.to_string()})),
                )
                    .into_response()
            }));

        Router::new()
            .route(
                "/metrics",
                get(move || {
                    let h = metric_handle.clone();
                    async move { h.render() }
                }),
            )
            .route("/health", get(health))
            .route("/version", get(version))
            .route("/ready", get(ready).with_state(ready_state))
            .merge(limited)
            .route(
                "/api-docs/openapi.json",
                get(move || {
                    let doc = openapi.clone();
                    async move { Json(doc) }
                }),
            )
            .route("/docs", get(api_docs_html))
            .fallback(not_found)
            .layer(prometheus_layer)
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(
                        DefaultMakeSpan::new()
                            .level(Level::INFO)
                            .include_headers(false),
                    )
                    .on_response(DefaultOnResponse::new().level(Level::INFO)),
            )
            .layer(PropagateRequestIdLayer::new(X_REQUEST_ID))
            .layer(SetRequestIdLayer::new(X_REQUEST_ID, MakeRequestUuid))
            .layer(axum::middleware::from_fn(security_headers))
            .layer(CompressionLayer::new())
            .layer(RequestBodyLimitLayer::new(5 * 1024 * 1024 * 1024))
            .layer(axum::extract::DefaultBodyLimit::disable())
            .layer(TimeoutLayer::with_status_code(
                axum::http::StatusCode::REQUEST_TIMEOUT,
                std::time::Duration::from_secs(30),
            ))
            .layer(cors)
    } else {
        Router::new()
            .route("/health", get(health))
            .nest("/api", api_router)
            .nest("/api", oauth::router().with_state(state.clone()))
            .merge(events::router().with_state(state))
            .route(
                "/api-docs/openapi.json",
                get(move || {
                    let doc = openapi.clone();
                    async move { Json(doc) }
                }),
            )
            .layer(cors)
    }
}
