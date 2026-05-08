use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::llm::ChatMessage;
use crate::models::{Message, Session};
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

    // 4. Open the upstream stream and pipe deltas back to the browser.
    let llm = state.llm.clone();
    let db = state.db.clone();
    let model = state.cfg.llm.model.clone();
    let assistant_id = Uuid::new_v4().to_string();
    let assistant_id_for_stream = assistant_id.clone();
    let session_id_for_stream = session_id.clone();
    let buffer = Arc::new(tokio::sync::Mutex::new(String::new()));
    let buffer_for_stream = buffer.clone();

    let upstream = llm.chat_stream(&model, messages).await?;
    let mut upstream = Box::pin(upstream);

    let stream = async_stream::stream! {
        use futures_util::StreamExt;

        // tell the client which message id will hold the final content
        let init = serde_json::json!({ "message_id": assistant_id_for_stream });
        yield Ok::<_, Infallible>(Event::default().event("init").data(init.to_string()));

        while let Some(chunk) = upstream.next().await {
            match chunk {
                Ok(crate::llm::StreamChunk::Delta(text)) => {
                    {
                        let mut buf = buffer_for_stream.lock().await;
                        buf.push_str(&text);
                    }
                    let payload = serde_json::json!({ "delta": text });
                    yield Ok(Event::default().event("delta").data(payload.to_string()));
                }
                Ok(crate::llm::StreamChunk::Done) => break,
                Err(e) => {
                    let payload = serde_json::json!({ "error": e.to_string() });
                    yield Ok(Event::default().event("error").data(payload.to_string()));
                    break;
                }
            }
        }

        let final_text = buffer_for_stream.lock().await.clone();
        let _ = sqlx::query(
            "INSERT INTO messages (id, session_id, role, content) VALUES (?1, ?2, 'assistant', ?3)",
        )
        .bind(&assistant_id_for_stream)
        .bind(&session_id_for_stream)
        .bind(&final_text)
        .execute(&db)
        .await;

        let _ = sqlx::query("UPDATE sessions SET updated_at = datetime('now') WHERE id = ?1")
            .bind(&session_id_for_stream)
            .execute(&db)
            .await;

        yield Ok(Event::default().event("done").data("{}"));
    };

    let boxed = Box::pin(stream);
    Ok(Sse::new(boxed).keep_alive(KeepAlive::default()).into_response())
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
