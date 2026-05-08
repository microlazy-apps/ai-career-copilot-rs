//! Per-instance application settings persisted in SQLite.
//!
//! The user configures the LLM provider from inside the running app
//! (Settings page), so the values must live in the database rather than
//! in environment variables. Environment values seed a fresh database
//! on first run for convenience (local dev / lpk one-shot install).

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppResult;

pub const GLOBAL_SCOPE: &str = "global";

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AppSettings {
    pub scope: String,
    pub llm_base_url: String,
    pub llm_api_key: String,
    pub llm_model: String,
    pub updated_at: String,
}

impl AppSettings {
    pub fn llm_configured(&self) -> bool {
        !self.llm_api_key.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AppSettingsView {
    pub llm_base_url: String,
    pub llm_model: String,
    /// Last 4 chars of the API key, padded with `*`. The full key is never
    /// returned over the wire after it is saved.
    pub llm_api_key_hint: String,
    pub llm_configured: bool,
    pub updated_at: String,
}

impl From<&AppSettings> for AppSettingsView {
    fn from(s: &AppSettings) -> Self {
        let key = s.llm_api_key.trim();
        let hint = if key.is_empty() {
            String::new()
        } else if key.len() <= 4 {
            "****".to_string()
        } else {
            format!("****{}", &key[key.len() - 4..])
        };

        Self {
            llm_base_url: s.llm_base_url.clone(),
            llm_model: s.llm_model.clone(),
            llm_api_key_hint: hint,
            llm_configured: s.llm_configured(),
            updated_at: s.updated_at.clone(),
        }
    }
}

pub async fn load(db: &SqlitePool) -> AppResult<AppSettings> {
    let row =
        sqlx::query_as::<_, AppSettings>("SELECT * FROM app_settings WHERE scope = ?1")
            .bind(GLOBAL_SCOPE)
            .fetch_one(db)
            .await?;
    Ok(row)
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateSettingsPayload {
    pub llm_base_url: Option<String>,
    pub llm_model: Option<String>,
    /// `None` = leave existing key untouched. `Some("")` = clear the key.
    pub llm_api_key: Option<String>,
}

pub async fn update(db: &SqlitePool, payload: UpdateSettingsPayload) -> AppResult<AppSettings> {
    let current = load(db).await?;

    let next_url = payload
        .llm_base_url
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(current.llm_base_url);
    let next_model = payload
        .llm_model
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(current.llm_model);
    let next_key = payload.llm_api_key.unwrap_or(current.llm_api_key);

    sqlx::query(
        r#"
        UPDATE app_settings
           SET llm_base_url = ?1,
               llm_model    = ?2,
               llm_api_key  = ?3,
               updated_at   = datetime('now')
         WHERE scope = ?4
        "#,
    )
    .bind(&next_url)
    .bind(&next_model)
    .bind(&next_key)
    .bind(GLOBAL_SCOPE)
    .execute(db)
    .await?;

    load(db).await
}

/// Seed settings from env vars on startup. Only fills in values that are
/// still at their default — never overwrites user-supplied configuration.
pub async fn seed_from_env(db: &SqlitePool, env: &crate::config::EnvDefaults) -> AppResult<()> {
    let current = load(db).await?;

    let mut next_url = current.llm_base_url.clone();
    let mut next_model = current.llm_model.clone();
    let mut next_key = current.llm_api_key.clone();
    let mut changed = false;

    if next_url == "https://api.deepseek.com/v1" {
        if let Some(v) = env.llm_base_url.as_ref() {
            if !v.trim().is_empty() && v != &next_url {
                next_url = v.clone();
                changed = true;
            }
        }
    }
    if next_model == "deepseek-chat" {
        if let Some(v) = env.llm_model.as_ref() {
            if !v.trim().is_empty() && v != &next_model {
                next_model = v.clone();
                changed = true;
            }
        }
    }
    if next_key.is_empty() {
        if let Some(v) = env.llm_api_key.as_ref() {
            if !v.trim().is_empty() {
                next_key = v.clone();
                changed = true;
            }
        }
    }

    if changed {
        sqlx::query(
            r#"
            UPDATE app_settings
               SET llm_base_url = ?1, llm_model = ?2, llm_api_key = ?3,
                   updated_at = datetime('now')
             WHERE scope = ?4
            "#,
        )
        .bind(&next_url)
        .bind(&next_model)
        .bind(&next_key)
        .bind(GLOBAL_SCOPE)
        .execute(db)
        .await?;
    }

    Ok(())
}
