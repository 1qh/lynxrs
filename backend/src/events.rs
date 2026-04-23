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

use crate::{auth::current_user_id, error::Result, state::AppState};

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventMsg {
    FileCreated {
        file_id: Uuid,
        owner_id: Uuid,
        filename: String,
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

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    jar: PrivateCookieJar,
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    // Auth: must be logged-in user
    let _uid = current_user_id(&jar)?;
    let bus = state.bus.clone();
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, bus)))
}

async fn handle_socket(socket: WebSocket, bus: EventBus) {
    let (mut tx, mut rx) = socket.split();
    let mut sub = bus.subscribe();

    // Initial hello
    let hello = serde_json::to_string(&EventMsg::Ping {
        at_ms: chrono::Utc::now().timestamp_millis(),
    })
    .unwrap_or_default();
    let _ = tx.send(Message::Text(hello.into())).await;

    // Forward broadcast → client.
    let fwd = tokio::spawn(async move {
        while let Ok(ev) = sub.recv().await {
            let Ok(body) = serde_json::to_string(&ev) else {
                continue;
            };
            if tx.send(Message::Text(body.into())).await.is_err() {
                break;
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
}

pub fn router() -> Router<AppState> {
    Router::new().route("/events/ws", get(ws_handler))
}
