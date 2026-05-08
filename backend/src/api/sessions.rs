use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::Session;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateSessionPayload {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub target_jd: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SessionView {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub target_jd: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<Vec<SessionView>>> {
    let rows = sqlx::query_as::<_, SessionView>(
        r#"
        SELECT
            s.id,
            s.user_id,
            s.title,
            s.target_jd,
            s.created_at,
            s.updated_at,
            COALESCE((SELECT COUNT(*) FROM messages m WHERE m.session_id = s.id), 0) AS message_count
        FROM sessions s
        WHERE s.user_id = ?1
        ORDER BY s.updated_at DESC
        "#,
    )
    .bind(&user.id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows))
}

pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Json(payload): Json<CreateSessionPayload>,
) -> AppResult<Json<Session>> {
    ensure_user_row(&state.db, &user).await?;

    let id = Uuid::new_v4().to_string();
    let title = payload
        .title
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(default_session_title);

    sqlx::query(
        r#"
        INSERT INTO sessions (id, user_id, title, target_jd)
        VALUES (?1, ?2, ?3, ?4)
        "#,
    )
    .bind(&id)
    .bind(&user.id)
    .bind(&title)
    .bind(&payload.target_jd)
    .execute(&state.db)
    .await?;

    let session = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = ?1")
        .bind(&id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(session))
}

pub async fn get(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<Session>> {
    let session = sqlx::query_as::<_, Session>(
        "SELECT * FROM sessions WHERE id = ?1 AND user_id = ?2",
    )
    .bind(&id)
    .bind(&user.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(session))
}

#[derive(Debug, Deserialize)]
pub struct UpdateSessionPayload {
    pub title: Option<String>,
    pub target_jd: Option<String>,
}

pub async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(payload): Json<UpdateSessionPayload>,
) -> AppResult<Json<Session>> {
    let _ = own_session(&state.db, &user.id, &id).await?;

    if let Some(title) = payload.title {
        sqlx::query(
            "UPDATE sessions SET title = ?1, updated_at = datetime('now') WHERE id = ?2 AND user_id = ?3",
        )
        .bind(&title)
        .bind(&id)
        .bind(&user.id)
        .execute(&state.db)
        .await?;
    }

    if let Some(jd) = payload.target_jd {
        sqlx::query(
            "UPDATE sessions SET target_jd = ?1, updated_at = datetime('now') WHERE id = ?2 AND user_id = ?3",
        )
        .bind(&jd)
        .bind(&id)
        .bind(&user.id)
        .execute(&state.db)
        .await?;
    }

    let session = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = ?1")
        .bind(&id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(session))
}

pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let res = sqlx::query("DELETE FROM sessions WHERE id = ?1 AND user_id = ?2")
        .bind(&id)
        .bind(&user.id)
        .execute(&state.db)
        .await?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
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

async fn ensure_user_row(db: &sqlx::SqlitePool, user: &AuthUser) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO users (id, email, name, avatar_url)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(id) DO NOTHING
        "#,
    )
    .bind(&user.id)
    .bind(&user.email)
    .bind(&user.name)
    .bind(&user.avatar_url)
    .execute(db)
    .await?;
    Ok(())
}

fn default_session_title() -> String {
    let now = chrono::Local::now();
    format!("新对话 · {}", now.format("%m-%d %H:%M"))
}
