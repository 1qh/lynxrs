//! Chat: conversations + messages + streaming response endpoint.
//!
//! The streaming endpoint emits Server-Sent Events. Today the "model" is a
//! deterministic stub (echoes the user prompt token-by-token) so the entire
//! UI surface — bubble rendering, optimistic insert, "stop generating",
//! token counter — can be built and tested without an upstream LLM key.
//! Swap `synthesize_response` for an Anthropic/OpenAI streaming call when
//! credentials are available; the on-the-wire SSE shape stays the same.
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
};
use axum_extra::extract::PrivateCookieJar;
use futures::Stream;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    entity::{conversation, message},
    error::{AppError, Result},
    state::AppState,
};

#[derive(Serialize, ToSchema)]
pub struct ConversationDto {
    pub id: Uuid,
    pub title: String,
    pub model: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<conversation::Model> for ConversationDto {
    fn from(m: conversation::Model) -> Self {
        Self {
            id: m.id,
            title: m.title,
            model: m.model,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct MessageDto {
    pub id: Uuid,
    pub role: String,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<message::Model> for MessageDto {
    fn from(m: message::Model) -> Self {
        Self {
            id: m.id,
            role: m.role,
            content: m.content,
            created_at: m.created_at,
        }
    }
}

#[derive(Deserialize, ToSchema)]
pub struct CreateConversationInput {
    pub title: Option<String>,
    pub model: Option<String>,
}

#[utoipa::path(post, path = "/conversations", request_body = CreateConversationInput,
    responses((status = 201, body = ConversationDto)))]
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Json(input): Json<CreateConversationInput>,
) -> Result<(StatusCode, Json<ConversationDto>)> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let now = chrono::Utc::now();
    let row = conversation::ActiveModel {
        id: Set(Uuid::now_v7()),
        owner_id: Set(uid),
        title: Set(input.title.unwrap_or_default()),
        model: Set(input.model.unwrap_or_else(|| "claude-opus-4-7".into())),
        created_at: Set(now),
        updated_at: Set(now),
        archived_at: Set(None),
    }
    .insert(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(row.into())))
}

#[utoipa::path(get, path = "/conversations",
    responses((status = 200, body = [ConversationDto])))]
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
) -> Result<Json<Vec<ConversationDto>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let rows = conversation::Entity::find()
        .filter(conversation::Column::OwnerId.eq(uid))
        .filter(conversation::Column::ArchivedAt.is_null())
        .order_by_desc(conversation::Column::UpdatedAt)
        .limit(200)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

#[utoipa::path(get, path = "/conversations/{id}/messages",
    responses((status = 200, body = [MessageDto]), (status = 404)))]
pub async fn messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<MessageDto>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let conv = conversation::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if conv.owner_id != uid {
        return Err(AppError::NotFound);
    }
    let rows = message::Entity::find()
        .filter(message::Column::ConversationId.eq(id))
        .order_by_asc(message::Column::ChainSeq)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

#[derive(Deserialize, ToSchema)]
pub struct SendMessageInput {
    pub content: String,
}

#[utoipa::path(post, path = "/conversations/{id}/messages", request_body = SendMessageInput,
    responses((status = 200), (status = 404)))]
pub async fn send_and_stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
    Json(input): Json<SendMessageInput>,
) -> Result<Sse<impl Stream<Item = std::result::Result<Event, Infallible>>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let conv = conversation::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if conv.owner_id != uid {
        return Err(AppError::NotFound);
    }

    // Persist the user turn synchronously so optimistic UI replay matches the
    // server-of-record on refresh.
    let user_msg = insert_message(&state, id, "user", &input.content).await?;
    let prompt = input.content.clone();
    let conv_id = id;
    let db_for_persist = state.db.clone();

    let stream = async_stream::stream! {
        // Emit the persisted user message id first so the client can swap its
        // optimistic local row for the canonical one.
        yield Ok(Event::default()
            .event("user_persisted")
            .data(user_msg.id.to_string()));

        // Stub model: stream the prompt back word-by-word so every chunk path
        // through the wire is exercised. Replace with anthropic SDK call when
        // ANTHROPIC_API_KEY is set; same SSE shape.
        let mut acc = String::new();
        for tok in synthesize_response(&prompt) {
            acc.push_str(&tok);
            yield Ok(Event::default().event("delta").data(tok));
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }

        // Persist the assistant turn at end-of-stream.
        if let Ok(msg) = insert_message_db(&db_for_persist, conv_id, "assistant", &acc).await {
            yield Ok(Event::default()
                .event("assistant_persisted")
                .data(msg.id.to_string()));
        }
        yield Ok(Event::default().event("done").data(""));
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn insert_message(
    state: &AppState,
    conv_id: Uuid,
    role: &str,
    content: &str,
) -> Result<message::Model> {
    insert_message_db(&state.db, conv_id, role, content).await
}

async fn insert_message_db(
    db: &sea_orm::DatabaseConnection,
    conv_id: Uuid,
    role: &str,
    content: &str,
) -> Result<message::Model> {
    use sea_orm::{ConnectionTrait, Statement};
    let id = Uuid::now_v7();
    let now = chrono::Utc::now();
    db.execute(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO messages \
         (id, conversation_id, role, content, attachments, created_at) \
         VALUES ($1, $2, $3, $4, '[]'::jsonb, $5)",
        [
            id.into(),
            conv_id.into(),
            role.into(),
            content.into(),
            now.into(),
        ],
    ))
    .await?;
    // Bump conversation.updated_at so the list view reorders by recency.
    db.execute(Statement::from_sql_and_values(
        db.get_database_backend(),
        "UPDATE conversations SET updated_at = $1 WHERE id = $2",
        [now.into(), conv_id.into()],
    ))
    .await?;
    let row = message::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(row)
}

/// Stub model — splits prompt into words and streams them back, simulating
/// a model that thoughtfully echoes. Replace with a real provider call when
/// credentials exist; the SSE shape (`delta` events + `done`) stays.
fn synthesize_response(prompt: &str) -> Vec<String> {
    let words: Vec<&str> = prompt.split_whitespace().collect();
    if words.is_empty() {
        return vec!["(empty prompt)".into()];
    }
    let mut out = vec!["You said: ".to_string()];
    for w in words {
        out.push(format!("{w} "));
    }
    out
}
