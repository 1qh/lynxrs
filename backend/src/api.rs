//! Public factory: build an Axum Router + OpenAPI spec from an AppState.
//! Shared by the bin entry point and the integration test harness.

use std::sync::Arc;

use axum::{Json, Router, http::HeaderName, routing::get};
use axum_prometheus::PrometheusMetricLayerBuilder;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};

async fn security_headers(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut res = next.run(req).await;
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
    res
}
use tracing::Level;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{admin, auth, error::ErrorBody, events, files, state::AppState};

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
        files::FileDto,
        files::FileList,
        files::Base64UploadInput,
        events::EventMsg,
        admin::AdminStats,
        admin::UserSummary,
    )),
    tags((name = "simu", description = "Simu SaaS API"))
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

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

pub fn build(state: AppState, opts: BuildOpts) -> Router {
    let (api_router, openapi) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(auth::signup))
        .routes(routes!(auth::login))
        .routes(routes!(auth::logout))
        .routes(routes!(auth::me))
        .routes(routes!(auth::forgot_password))
        .routes(routes!(auth::reset_password))
        .routes(routes!(auth::change_password))
        .routes(routes!(files::upload))
        .routes(routes!(files::upload_json))
        .routes(routes!(files::list))
        .routes(routes!(files::download))
        .routes(routes!(files::delete))
        .routes(routes!(admin::stats))
        .routes(routes!(admin::list_users))
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

        let limited = Router::new()
            .nest("/api", api_router)
            .merge(events::router().with_state(state))
            .layer(GovernorLayer::new(governor_conf));

        Router::new()
            .route(
                "/metrics",
                get(move || {
                    let h = metric_handle.clone();
                    async move { h.render() }
                }),
            )
            .route("/health", get(health))
            .merge(limited)
            .route(
                "/api-docs/openapi.json",
                get(move || {
                    let doc = openapi.clone();
                    async move { Json(doc) }
                }),
            )
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
            .layer(cors)
    } else {
        Router::new()
            .route("/health", get(health))
            .nest("/api", api_router)
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
