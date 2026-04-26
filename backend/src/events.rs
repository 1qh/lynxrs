use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};
use axum_extra::extract::PrivateCookieJar;
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::Result, state::AppState};

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventMsg {
    FileCreated {
        file_id: Uuid,
        owner_id: Uuid,
        filename: String,
    },
    FileDeleted {
        file_id: Uuid,
        owner_id: Uuid,
    },
    Ping {
        at_ms: i64,
    },
}

/// Broadcast channel handle shared across handlers.
pub type EventBus = broadcast::Sender<EventMsg>;

pub fn new_bus(capacity: usize) -> EventBus {
    broadcast::channel(capacity).0
}

/// If `NATS_URL` is set, bridge the in-process broadcast bus to a NATS subject
/// so every instance sees every other instance's events. The bridge runs
/// forever on a background tokio task; if NATS is down we log and quietly
/// fall back to local-only.
pub fn spawn_nats_bridge(bus: EventBus) {
    let url = match std::env::var("NATS_URL") {
        Ok(v) => v,
        Err(_) => return,
    };
    let subject = std::env::var("NATS_SUBJECT").unwrap_or_else(|_| "simu.events".into());
    let instance_id = uuid::Uuid::now_v7().to_string();
    tokio::spawn(async move {
        let client = match async_nats::connect(&url).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error=%e, url=%url, "nats connect failed; running local-only");
                return;
            }
        };
        tracing::info!(url=%url, subject=%subject, "nats event bridge online");

        // Outbound: every local broadcast → NATS publish (with origin tag)
        let mut rx = bus.subscribe();
        let out_client = client.clone();
        let out_subject = subject.clone();
        let out_instance = instance_id.clone();
        tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                let envelope = serde_json::json!({
                    "origin": out_instance,
                    "msg": msg,
                });
                if let Ok(bytes) = serde_json::to_vec(&envelope) {
                    let _ = out_client.publish(out_subject.clone(), bytes.into()).await;
                }
            }
        });

        // Inbound: NATS subject → local broadcast (drop our own echoes)
        let mut sub = match client.subscribe(subject.clone()).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error=%e, "nats subscribe failed");
                return;
            }
        };
        while let Some(m) = sub.next().await {
            let Ok(env) = serde_json::from_slice::<serde_json::Value>(&m.payload) else {
                continue;
            };
            if env["origin"].as_str() == Some(&instance_id) {
                continue;
            }
            if let Ok(msg) = serde_json::from_value::<EventMsg>(env["msg"].clone()) {
                let _ = bus.send(msg);
            }
        }
    });
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    // Auth: must be logged-in user
    let _uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let bus = state.bus.clone();
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, bus)))
}

async fn handle_socket(socket: WebSocket, bus: EventBus) {
    metrics::gauge!("simu_ws_connections").increment(1.0);
    let (mut tx, mut rx) = socket.split();
    let mut sub = bus.subscribe();

    // Initial hello
    let hello = serde_json::to_string(&EventMsg::Ping {
        at_ms: chrono::Utc::now().timestamp_millis(),
    })
    .unwrap_or_default();
    let _ = tx.send(Message::Text(hello.into())).await;

    // Forward broadcast → client. Handle Lagged separately so a slow client
    // doesn't silently lose its forwarder; we just skip the gap and keep
    // delivering future events. Closed = bus dropped → real disconnect.
    let fwd = tokio::spawn(async move {
        use tokio::sync::broadcast::error::RecvError;
        loop {
            match sub.recv().await {
                Ok(ev) => {
                    let Ok(body) = serde_json::to_string(&ev) else {
                        continue;
                    };
                    if tx.send(Message::Text(body.into())).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(n)) => {
                    metrics::counter!("simu_ws_lagged_total").increment(n);
                    tracing::warn!(skipped = n, "ws subscriber lagged");
                }
                Err(RecvError::Closed) => break,
            }
        }
    });

    // Drain incoming until close. We don't use client→server messages yet.
    while let Some(msg) = rx.next().await {
        match msg {
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }

    fwd.abort();
    metrics::gauge!("simu_ws_connections").decrement(1.0);
}

pub async fn sse_handler(
    headers: axum::http::HeaderMap,
    jar: PrivateCookieJar,
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let _uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let mut sub = state.bus.subscribe();
    let stream = async_stream::stream! {
        // Initial ping
        yield Ok::<_, std::convert::Infallible>(axum::response::sse::Event::default()
            .event("ping")
            .data(chrono::Utc::now().timestamp_millis().to_string()));
        while let Ok(ev) = sub.recv().await {
            if let Ok(body) = serde_json::to_string(&ev) {
                yield Ok(axum::response::sse::Event::default().data(body));
            }
        }
    };
    Ok(axum::response::Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default()))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/events/ws", get(ws_handler))
        .route("/events/sse", get(sse_handler))
}
