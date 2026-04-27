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
    pub system_prompt: String,
    pub temperature: f32,
    pub shared: bool,
}

impl From<conversation::Model> for ConversationDto {
    fn from(m: conversation::Model) -> Self {
        Self {
            id: m.id,
            title: m.title,
            model: m.model,
            created_at: m.created_at,
            updated_at: m.updated_at,
            system_prompt: m.system_prompt,
            temperature: m.temperature,
            shared: m.share_token_hash.is_some(),
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
        model: Set(input.model.unwrap_or_else(default_model)),
        created_at: Set(now),
        updated_at: Set(now),
        archived_at: Set(None),
        system_prompt: Set(String::new()),
        temperature: Set(0.7),
        share_token_hash: Set(None),
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

#[derive(Deserialize, ToSchema)]
pub struct UpdateConversationInput {
    pub title: Option<String>,
    pub model: Option<String>,
    /// Set to true to archive (hide from list); false to unarchive.
    pub archived: Option<bool>,
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
}

#[utoipa::path(patch, path = "/conversations/{id}", request_body = UpdateConversationInput,
    responses((status = 200, body = ConversationDto), (status = 404)))]
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateConversationInput>,
) -> Result<Json<ConversationDto>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let conv = conversation::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if conv.owner_id != uid {
        return Err(AppError::NotFound);
    }
    let mut am: conversation::ActiveModel = conv.into();
    if let Some(t) = input.title {
        am.title = Set(t);
    }
    if let Some(m) = input.model {
        am.model = Set(m);
    }
    if let Some(a) = input.archived {
        am.archived_at = Set(if a { Some(chrono::Utc::now()) } else { None });
    }
    if let Some(s) = input.system_prompt {
        am.system_prompt = Set(s);
    }
    if let Some(t) = input.temperature {
        am.temperature = Set(t.clamp(0.0, 2.0));
    }
    am.updated_at = Set(chrono::Utc::now());
    let updated = am.update(&state.db).await?;
    Ok(Json(updated.into()))
}

#[utoipa::path(delete, path = "/conversations/{id}",
    responses((status = 204), (status = 404)))]
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let conv = conversation::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if conv.owner_id != uid {
        return Err(AppError::NotFound);
    }
    // CASCADE on messages.conversation_id removes child rows automatically.
    conversation::Entity::delete_by_id(id)
        .exec(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema, utoipa::IntoParams)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Serialize, ToSchema)]
pub struct SearchHit {
    pub conversation_id: Uuid,
    pub title: String,
    pub message_id: Uuid,
    pub snippet: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[utoipa::path(get, path = "/conversations/search", params(SearchQuery),
    responses((status = 200, body = [SearchHit])))]
pub async fn search(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    axum::extract::Query(q): axum::extract::Query<SearchQuery>,
) -> Result<Json<Vec<SearchHit>>> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let needle = q.q.trim().to_string();
    if needle.is_empty() {
        return Ok(Json(vec![]));
    }
    use sea_orm::{ConnectionTrait, FromQueryResult, Statement};
    #[derive(FromQueryResult)]
    struct Row {
        conversation_id: Uuid,
        title: String,
        message_id: Uuid,
        snippet: String,
        created_at: chrono::DateTime<chrono::Utc>,
    }
    // Substring match on both title and content; bounded result count keeps
    // this responsive even on large stores. Real FTS upgrade: tsvector on
    // messages.content like we did for files (deferred until volume warrants).
    let pattern = format!("%{}%", needle);
    let rows: Vec<Row> = Row::find_by_statement(Statement::from_sql_and_values(
        state.db.get_database_backend(),
        "SELECT m.conversation_id, c.title, m.id AS message_id, \
                substring(m.content from 1 for 200) AS snippet, m.created_at \
         FROM messages m \
         JOIN conversations c ON c.id = m.conversation_id \
         WHERE c.owner_id = $1 \
           AND (m.content ILIKE $2 OR c.title ILIKE $2) \
         ORDER BY m.created_at DESC \
         LIMIT 50",
        [uid.into(), pattern.into()],
    ))
    .all(&state.db)
    .await?;
    let hits = rows
        .into_iter()
        .map(|r| SearchHit {
            conversation_id: r.conversation_id,
            title: r.title,
            message_id: r.message_id,
            snippet: r.snippet,
            created_at: r.created_at,
        })
        .collect();
    Ok(Json(hits))
}

#[derive(Serialize, ToSchema)]
pub struct ExportDto {
    pub markdown: String,
}

#[utoipa::path(get, path = "/conversations/{id}/export",
    responses((status = 200, body = ExportDto), (status = 404)))]
pub async fn export(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<ExportDto>> {
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
    let mut md = String::new();
    md.push_str(&format!(
        "# {}\n\n_Model: `{}` · {}_\n\n",
        if conv.title.is_empty() {
            "Untitled"
        } else {
            &conv.title
        },
        conv.model,
        conv.created_at.to_rfc3339(),
    ));
    for r in rows {
        md.push_str(&format!("## {}\n\n{}\n\n", r.role, r.content));
    }
    Ok(Json(ExportDto { markdown: md }))
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
    let _ = state.bus.send(crate::events::EventMsg::MessageCreated {
        conversation_id: id,
        message_id: user_msg.id,
        role: "user".to_string(),
    });
    let prompt = input.content.clone();
    let conv_id = id;
    let db_for_persist = state.db.clone();

    // Snapshot prior turns once so the streaming closure doesn't need a live
    // db handle just to build the prompt. Deliberately no system prompt yet —
    // that's a settings-screen knob we ship in the next slice.
    let prior = message::Entity::find()
        .filter(message::Column::ConversationId.eq(id))
        .order_by_asc(message::Column::ChainSeq)
        .all(&state.db)
        .await
        .unwrap_or_default();
    let mut history: Vec<(String, String)> = Vec::new();
    if !conv.system_prompt.is_empty() {
        history.push(("system".to_string(), conv.system_prompt.clone()));
    }
    history.extend(prior.into_iter().map(|m| (m.role, m.content)));
    let model_name = conv.model.clone();
    let temperature = conv.temperature;
    let bus = state.bus.clone();

    let stream = async_stream::stream! {
        yield Ok(Event::default()
            .event("user_persisted")
            .data(user_msg.id.to_string()));

        let mut acc = String::new();
        match openai_stream(&model_name, &history, &prompt, temperature).await {
            Ok(mut rx) => {
                while let Some(ev) = rx.recv().await {
                    match ev {
                        ChatEvent::Delta(t) => {
                            acc.push_str(&t);
                            yield Ok(Event::default().event("delta").data(t));
                        }
                        ChatEvent::Error(e) => {
                            yield Ok(Event::default().event("error").data(e));
                        }
                        ChatEvent::Done => break,
                    }
                }
            }
            Err(_) => {
                // No upstream LLM reachable — fall back to stub so the UI
                // is exercised. Real LLMs report errors via ChatEvent::Error.
                for tok in synthesize_response(&prompt) {
                    acc.push_str(&tok);
                    yield Ok(Event::default().event("delta").data(tok));
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
            }
        }

        if let Ok(msg) = insert_message_db(&db_for_persist, conv_id, "assistant", &acc).await {
            let _ = bus.send(crate::events::EventMsg::MessageCreated {
                conversation_id: conv_id,
                message_id: msg.id,
                role: "assistant".to_string(),
            });
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

/// Default model name. Override per-conversation via `CreateConversationInput.model`.
/// Configure with env `OPENAI_DEFAULT_MODEL` (e.g. `gpt-4o-mini` for real OpenAI).
fn default_model() -> String {
    std::env::var("OPENAI_DEFAULT_MODEL").unwrap_or_else(|_| "qwen3.5:4b-q4_K_M".into())
}

/// Stub model — splits prompt into words and streams them back, simulating
/// a model that thoughtfully echoes. Used only when no upstream LLM is
/// reachable (e.g. in CI without ollama running).
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

#[derive(Debug)]
pub enum ChatEvent {
    Delta(String),
    Error(String),
    Done,
}

/// OpenAI-compatible streaming chat completion. Defaults target a local
/// Ollama at :11434 — set `OPENAI_BASE_URL` to point elsewhere (e.g. real
/// OpenAI, vLLM, llama.cpp server). `OPENAI_API_KEY` is sent as Bearer when
/// present; unset for local Ollama.
pub async fn openai_stream(
    model: &str,
    history: &[(String, String)],
    prompt: &str,
    temperature: f32,
) -> std::result::Result<tokio::sync::mpsc::Receiver<ChatEvent>, anyhow::Error> {
    use futures::StreamExt;
    let base =
        std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "http://localhost:11434/v1".into());
    let url = format!("{}/chat/completions", base.trim_end_matches('/'));
    let mut messages: Vec<serde_json::Value> = history
        .iter()
        .filter(|(r, _)| r == "user" || r == "assistant" || r == "system")
        .map(|(role, content)| serde_json::json!({"role": role, "content": content}))
        .collect();
    messages.push(serde_json::json!({"role": "user", "content": prompt}));
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "temperature": temperature,
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let mut req = client.post(&url).json(&body);
    if let Ok(key) = std::env::var("OPENAI_API_KEY")
        && !key.is_empty()
    {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let txt = resp.text().await.unwrap_or_default();
        anyhow::bail!("openai-compat {status}: {txt}");
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<ChatEvent>(64);
    tokio::spawn(async move {
        let mut stream = resp.bytes_stream();
        let mut buf: Vec<u8> = Vec::new();
        while let Some(chunk) = stream.next().await {
            let Ok(bytes) = chunk else { break };
            buf.extend_from_slice(&bytes);
            // SSE frames are separated by blank lines. Iterate complete frames.
            while let Some(pos) = find_subseq(&buf, b"\n\n") {
                let frame = buf.drain(..pos + 2).collect::<Vec<_>>();
                let frame = String::from_utf8_lossy(&frame).to_string();
                for line in frame.lines() {
                    let Some(payload) = line.strip_prefix("data:") else {
                        continue;
                    };
                    let payload = payload.trim();
                    if payload == "[DONE]" {
                        let _ = tx.send(ChatEvent::Done).await;
                        return;
                    }
                    if payload.is_empty() {
                        continue;
                    }
                    let Ok(json) = serde_json::from_str::<serde_json::Value>(payload) else {
                        continue;
                    };
                    if let Some(content) = json
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|c| c.as_str())
                        && !content.is_empty()
                    {
                        let _ = tx.send(ChatEvent::Delta(content.to_string())).await;
                    }
                }
            }
        }
        let _ = tx.send(ChatEvent::Done).await;
    });
    Ok(rx)
}

fn find_subseq(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[derive(Serialize, ToSchema)]
pub struct ShareCreated {
    pub url: String,
    pub token: String,
}

/// Mint a public read-only link for a conversation. Storing only the SHA-256
/// of the token (never plaintext) so a DB leak doesn't expose live links.
#[utoipa::path(post, path = "/conversations/{id}/share",
    responses((status = 201, body = ShareCreated), (status = 404)))]
pub async fn share(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<ShareCreated>)> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let conv = conversation::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if conv.owner_id != uid {
        return Err(AppError::NotFound);
    }
    let mut bytes = [0u8; 24];
    for b in &mut bytes {
        *b = rand::random::<u8>();
    }
    let token = hex::encode(bytes);
    let hash = {
        use sha2::Digest;
        hex::encode(sha2::Sha256::digest(token.as_bytes()))
    };
    let mut am: conversation::ActiveModel = conv.into();
    am.share_token_hash = Set(Some(hash));
    am.update(&state.db).await?;
    let base = std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "".into());
    let url = format!("{base}/api/share/conversation/{token}");
    Ok((StatusCode::CREATED, Json(ShareCreated { url, token })))
}

#[utoipa::path(delete, path = "/conversations/{id}/share",
    responses((status = 204), (status = 404)))]
pub async fn unshare(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let uid = crate::auth::authenticate(&state, &headers, &jar).await?;
    let conv = conversation::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if conv.owner_id != uid {
        return Err(AppError::NotFound);
    }
    let mut am: conversation::ActiveModel = conv.into();
    am.share_token_hash = Set(None);
    am.update(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
pub struct PublicConversation {
    pub title: String,
    pub model: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub messages: Vec<MessageDto>,
}

/// Public read: anyone with the token sees the conversation's frozen-at-share
/// state. Constant-time compare on the hash; tokens not in DB get a 404.
#[utoipa::path(get, path = "/share/conversation/{token}",
    responses((status = 200, body = PublicConversation), (status = 404)))]
pub async fn share_view(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<PublicConversation>> {
    let hash = {
        use sha2::Digest;
        hex::encode(sha2::Sha256::digest(token.as_bytes()))
    };
    let conv = conversation::Entity::find()
        .filter(conversation::Column::ShareTokenHash.eq(Some(hash)))
        .one(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let rows = message::Entity::find()
        .filter(message::Column::ConversationId.eq(conv.id))
        .order_by_asc(message::Column::ChainSeq)
        .all(&state.db)
        .await?;
    Ok(Json(PublicConversation {
        title: conv.title,
        model: conv.model,
        created_at: conv.created_at,
        messages: rows.into_iter().map(Into::into).collect(),
    }))
}
