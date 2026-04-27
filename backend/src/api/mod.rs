//! Public factory: build an Axum Router + OpenAPI spec from an AppState.
//! Shared by the bin entry point and the integration test harness.

mod builtin;
mod middleware;

use std::sync::Arc;

use axum::{Json, Router, http::HeaderName, routing::get};

use axum_prometheus::PrometheusMetricLayerBuilder;
use builtin::{api_docs_html, health, not_found, ready, version};
use middleware::{inject_request_id_into_errors, security_headers};
use tower_governor::{
    GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor,
};
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::Level;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    admin, audit, auth, chat, error::ErrorBody, events, files, mfa, oauth, orgs, state::AppState,
    tokens, webhooks,
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
        chat::ConversationDto,
        chat::CreateConversationInput,
        chat::UpdateConversationInput,
        chat::MessageDto,
        chat::SendMessageInput,
        chat::SearchHit,
        chat::ExportDto,
        chat::ShareCreated,
        chat::PublicConversation,
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
        .routes(routes!(chat::create, chat::list))
        .routes(routes!(chat::search))
        .routes(routes!(chat::update, chat::delete))
        .routes(routes!(chat::export))
        .routes(routes!(chat::share, chat::unshare))
        .routes(routes!(chat::share_view))
        .routes(routes!(chat::messages, chat::send_and_stream))
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
        // SmartIpKeyExtractor reads X-Forwarded-For / X-Real-IP first so
        // running behind a reverse proxy doesn't collapse every client
        // onto the proxy's loopback IP.
        let governor_conf = Arc::new(
            GovernorConfigBuilder::default()
                .per_second(opts.rate_limit_rps)
                .burst_size(opts.rate_limit_burst)
                .key_extractor(SmartIpKeyExtractor)
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
