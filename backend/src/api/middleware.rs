use axum::http::HeaderName;

pub async fn inject_request_id_into_errors(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
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
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => {
            metrics::counter!("simu_err_body_rewrite_failed_total").increment(1);
            return axum::response::Response::from_parts(parts, axum::body::Body::empty());
        }
    };
    let mut val: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => return axum::response::Response::from_parts(parts, axum::body::Body::from(bytes)),
    };
    if let (Some(obj), Some(id)) = (val.as_object_mut(), req_id) {
        obj.entry("request_id")
            .or_insert(serde_json::Value::String(id));
    }
    let new_body = serde_json::to_vec(&val).unwrap_or_else(|_| bytes.to_vec());
    axum::response::Response::from_parts(parts, axum::body::Body::from(new_body))
}

pub async fn security_headers(
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

