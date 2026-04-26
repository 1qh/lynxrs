use axum::{Json, response::IntoResponse};

use crate::state::AppState;

pub async fn api_docs_html() -> axum::response::Response {
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

pub async fn health() -> axum::response::Response {
    (
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(serde_json::json!({ "status": "ok" })),
    )
        .into_response()
}

pub async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "rustc": option_env!("VERGEN_RUSTC_SEMVER").unwrap_or("unknown"),
    }))
}

pub async fn not_found(uri: axum::http::Uri) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "code": "not_found",
            "message": format!("no route for {}", uri.path()),
        })),
    )
}

/// Readiness check — verifies DB + S3 reachable. Returns 503 on failure.
/// Kubernetes/Docker can split liveness (/health) from readiness (/ready).
pub async fn ready(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::response::Response {
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
