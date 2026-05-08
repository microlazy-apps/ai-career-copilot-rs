use std::convert::Infallible;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::llm::{ChatMessage, LlmCallConfig};
use crate::models::{Message, Session};
use crate::settings;
use crate::AppState;

const MAX_HISTORY_TURNS: usize = 30;

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(session_id): Path<String>,
) -> AppResult<Json<Vec<Message>>> {
    own_session(&state.db, &user.id, &session_id).await?;

    let rows = sqlx::query_as::<_, Message>(
        "SELECT * FROM messages WHERE session_id = ?1 ORDER BY created_at ASC, id ASC",
    )
    .bind(&session_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
pub struct PostMessagePayload {
    pub content: String,
}

/// Stream the assistant reply as SSE so the UI can render incrementally.
/// We persist both the user message (immediately) and the assistant
/// message (after the stream completes) to SQLite.
pub async fn stream(
    State(state): State<AppState>,
    user: AuthUser,
    Path(session_id): Path<String>,
    Json(payload): Json<PostMessagePayload>,
) -> AppResult<Response> {
    let trimmed = payload.content.trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("empty message".into()));
    }

    let session = own_session(&state.db, &user.id, &session_id).await?;

    // 1. Persist the user message right away.
    let user_msg_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content) VALUES (?1, ?2, 'user', ?3)",
    )
    .bind(&user_msg_id)
    .bind(&session_id)
    .bind(&trimmed)
    .execute(&state.db)
    .await?;

    bump_session_updated(&state.db, &session_id).await?;

    // 2. Auto-title the session from the first user message if it's still default.
    if session.title.starts_with("新对话") {
        let title = trimmed.chars().take(24).collect::<String>();
        if !title.is_empty() {
            sqlx::query("UPDATE sessions SET title = ?1 WHERE id = ?2")
                .bind(&title)
                .bind(&session_id)
                .execute(&state.db)
                .await?;
        }
    }

    // 3. Build the prompt history for the LLM.
    let history = recent_history(&state.db, &session_id).await?;
    let mut messages = Vec::with_capacity(history.len() + 1);
    messages.push(ChatMessage {
        role: "system".into(),
        content: crate::llm::SYSTEM_PROMPT.into(),
    });
    if let Some(jd) = session.target_jd.as_ref() {
        if !jd.trim().is_empty() {
            messages.push(ChatMessage {
                role: "system".into(),
                content: format!("当前目标岗位 JD：\n{jd}"),
            });
        }
    }
    for m in history {
        messages.push(ChatMessage {
            role: m.role,
            content: m.content,
        });
    }

    // 4. Resolve LLM credentials from per-instance settings (in-app config).
    let app_settings = settings::load(&state.db).await?;
    if !app_settings.llm_configured() {
        return Err(AppError::Llm(
            "尚未配置 LLM API Key，请在右上角「设置」里填入。".into(),
        ));
    }
    let call_cfg = LlmCallConfig {
        base_url: app_settings.llm_base_url.clone(),
        api_key: app_settings.llm_api_key.clone(),
        model: app_settings.llm_model.clone(),
    };

    // 5. Insert an empty assistant row immediately so a mid-stream client
    //    disconnect still leaves a recoverable partial reply on refresh.
    let assistant_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content) VALUES (?1, ?2, 'assistant', '')",
    )
    .bind(&assistant_id)
    .bind(&session_id)
    .execute(&state.db)
    .await?;

    // 6. Spawn the upstream consumer in a detached task so it keeps writing
    //    deltas to SQLite even if the browser closes the SSE connection.
    let upstream = state.llm.chat_stream(call_cfg, messages).await?;
    let (tx, mut rx) =
        tokio::sync::mpsc::channel::<StreamEvent>(64);

    let db_task = state.db.clone();
    let assistant_id_task = assistant_id.clone();
    let session_id_task = session_id.clone();
    tokio::spawn(async move {
        consume_upstream(upstream, tx, db_task, assistant_id_task, session_id_task).await;
    });

    // 7. Forward channel events to the SSE response.
    let assistant_id_for_init = assistant_id.clone();
    let stream = async_stream::stream! {
        let init = serde_json::json!({ "message_id": assistant_id_for_init });
        yield Ok::<_, Infallible>(Event::default().event("init").data(init.to_string()));

        while let Some(ev) = rx.recv().await {
            match ev {
                StreamEvent::Delta(text) => {
                    let payload = serde_json::json!({ "delta": text });
                    yield Ok(Event::default().event("delta").data(payload.to_string()));
                }
                StreamEvent::Error(msg) => {
                    let payload = serde_json::json!({ "error": msg });
                    yield Ok(Event::default().event("error").data(payload.to_string()));
                    break;
                }
                StreamEvent::Done => {
                    yield Ok(Event::default().event("done").data("{}"));
                    break;
                }
            }
        }
    };

    let boxed = Box::pin(stream);
    Ok(Sse::new(boxed).keep_alive(KeepAlive::default()).into_response())
}

#[derive(Debug)]
enum StreamEvent {
    Delta(String),
    Error(String),
    Done,
}

async fn consume_upstream<S>(
    upstream: S,
    tx: tokio::sync::mpsc::Sender<StreamEvent>,
    db: sqlx::SqlitePool,
    assistant_id: String,
    session_id: String,
) where
    S: futures_util::Stream<Item = AppResult<crate::llm::StreamChunk>> + Send + 'static,
{
    use futures_util::StreamExt;

    let mut upstream = Box::pin(upstream);
    let mut buffer = String::new();
    let mut since_flush = 0usize;

    while let Some(chunk) = upstream.next().await {
        match chunk {
            Ok(crate::llm::StreamChunk::Delta(text)) => {
                buffer.push_str(&text);
                since_flush += text.len();
                if since_flush >= 64 {
                    flush_assistant(&db, &assistant_id, &buffer).await;
                    since_flush = 0;
                }
                // Send to client; ignore Err — the client may have disconnected
                // and we still want to finish persisting the response.
                let _ = tx.send(StreamEvent::Delta(text)).await;
            }
            Ok(crate::llm::StreamChunk::Done) => break,
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(e.to_string())).await;
                break;
            }
        }
    }

    // Final write — capture whatever we have, even on partial failures.
    flush_assistant(&db, &assistant_id, &buffer).await;
    let _ = sqlx::query("UPDATE sessions SET updated_at = datetime('now') WHERE id = ?1")
        .bind(&session_id)
        .execute(&db)
        .await;
    let _ = tx.send(StreamEvent::Done).await;
}

async fn flush_assistant(db: &sqlx::SqlitePool, assistant_id: &str, content: &str) {
    let _ = sqlx::query("UPDATE messages SET content = ?1 WHERE id = ?2")
        .bind(content)
        .bind(assistant_id)
        .execute(db)
        .await;
}

async fn own_session(
    db: &sqlx::SqlitePool,
    user_id: &str,
    session_id: &str,
) -> AppResult<Session> {
    sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = ?1 AND user_id = ?2")
        .bind(session_id)
        .bind(user_id)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound)
}

async fn bump_session_updated(db: &sqlx::SqlitePool, session_id: &str) -> AppResult<()> {
    sqlx::query("UPDATE sessions SET updated_at = datetime('now') WHERE id = ?1")
        .bind(session_id)
        .execute(db)
        .await?;
    Ok(())
}

async fn recent_history(
    db: &sqlx::SqlitePool,
    session_id: &str,
) -> AppResult<Vec<Message>> {
    let mut rows = sqlx::query_as::<_, Message>(
        "SELECT * FROM messages WHERE session_id = ?1 ORDER BY created_at DESC, id DESC LIMIT ?2",
    )
    .bind(session_id)
    .bind(MAX_HISTORY_TURNS as i64)
    .fetch_all(db)
    .await?;

    rows.reverse();
    Ok(rows)
}
