use axum::extract::State;
use axum::Json;
use serde::Serialize;
use serde_json::json;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::llm::LlmCallConfig;
use crate::settings::{self, AppSettingsView, UpdateSettingsPayload};
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct GetSettingsResponse {
    #[serde(flatten)]
    pub view: AppSettingsView,
}

pub async fn get(
    State(state): State<AppState>,
    _user: AuthUser,
) -> AppResult<Json<GetSettingsResponse>> {
    let s = settings::load(&state.db).await?;
    Ok(Json(GetSettingsResponse { view: (&s).into() }))
}

pub async fn update(
    State(state): State<AppState>,
    _user: AuthUser,
    Json(payload): Json<UpdateSettingsPayload>,
) -> AppResult<Json<GetSettingsResponse>> {
    let s = settings::update(&state.db, payload).await?;
    Ok(Json(GetSettingsResponse { view: (&s).into() }))
}

pub async fn test_llm(
    State(state): State<AppState>,
    _user: AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let s = settings::load(&state.db).await?;
    if !s.llm_configured() {
        return Err(AppError::Llm("尚未配置 API Key".into()));
    }
    let cfg = LlmCallConfig {
        base_url: s.llm_base_url.clone(),
        api_key: s.llm_api_key.clone(),
        model: s.llm_model.clone(),
    };
    let reply = state.llm.chat_probe(&cfg).await?;
    Ok(Json(json!({
        "ok": true,
        "model": s.llm_model,
        "base_url": s.llm_base_url,
        "reply": reply,
    })))
}
