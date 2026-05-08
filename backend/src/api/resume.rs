use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::ResumeContent;
use crate::AppState;

pub async fn get(
    State(state): State<AppState>,
    user: AuthUser,
    Path(session_id): Path<String>,
) -> AppResult<Json<Value>> {
    own_session(&state.db, &user.id, &session_id).await?;

    let row = sqlx::query_as::<_, ResumeContent>(
        "SELECT * FROM resume_contents WHERE session_id = ?1",
    )
    .bind(&session_id)
    .fetch_optional(&state.db)
    .await?;

    match row {
        Some(r) => {
            let content: Value = serde_json::from_str(&r.content_json).unwrap_or(Value::Null);
            Ok(Json(json!({
                "version": r.version,
                "updated_at": r.updated_at,
                "content": content,
            })))
        }
        None => Ok(Json(json!({ "version": 0, "content": null }))),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpsertPayload {
    pub content: Value,
}

pub async fn upsert(
    State(state): State<AppState>,
    user: AuthUser,
    Path(session_id): Path<String>,
    Json(payload): Json<UpsertPayload>,
) -> AppResult<Json<Value>> {
    own_session(&state.db, &user.id, &session_id).await?;

    let content_json = serde_json::to_string(&payload.content)
        .map_err(|e| AppError::BadRequest(format!("invalid json: {e}")))?;

    sqlx::query(
        r#"
        INSERT INTO resume_contents (session_id, version, content_json)
        VALUES (?1, 1, ?2)
        ON CONFLICT(session_id) DO UPDATE SET
            version = resume_contents.version + 1,
            content_json = excluded.content_json,
            updated_at = datetime('now')
        "#,
    )
    .bind(&session_id)
    .bind(&content_json)
    .execute(&state.db)
    .await?;

    let row = sqlx::query_as::<_, ResumeContent>(
        "SELECT * FROM resume_contents WHERE session_id = ?1",
    )
    .bind(&session_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(json!({
        "version": row.version,
        "updated_at": row.updated_at,
    })))
}

async fn own_session(
    db: &sqlx::SqlitePool,
    user_id: &str,
    session_id: &str,
) -> AppResult<()> {
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sessions WHERE id = ?1 AND user_id = ?2",
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_one(db)
    .await?;

    if exists == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}
