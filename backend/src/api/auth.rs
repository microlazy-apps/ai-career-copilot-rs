use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;
use serde_json::json;

use crate::auth::oidc::random_state;
use crate::auth::session::{AuthUser, SessionToken, SESSION_COOKIE};
use crate::error::{AppError, AppResult};
use crate::AppState;

const STATE_COOKIE: &str = "ac_oidc_state";

pub async fn me(
    State(state): State<AppState>,
    user: Option<AuthUser>,
) -> Json<serde_json::Value> {
    let methods = json!({
        "oidc": state.oidc.is_some(),
        "email": state.cfg.email_login_enabled,
    });
    match user {
        Some(u) => Json(json!({
            "authenticated": true,
            "methods": methods,
            "user": {
                "id": u.id,
                "email": u.email,
                "name": u.name,
                "avatar_url": u.avatar_url,
            }
        })),
        None => Json(json!({
            "authenticated": false,
            "methods": methods,
        })),
    }
}

pub async fn login(State(state): State<AppState>, jar: CookieJar) -> AppResult<Response> {
    let oidc = state
        .oidc
        .as_ref()
        .ok_or(AppError::OidcNotConfigured)?;

    let csrf = random_state();
    let url = oidc.authorize_url(&csrf);

    let cookie = Cookie::build((STATE_COOKIE, csrf))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::minutes(10))
        .build();

    Ok((jar.add(cookie), Redirect::to(&url)).into_response())
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

pub async fn callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<CallbackQuery>,
) -> AppResult<Response> {
    if let Some(err) = q.error {
        let desc = q.error_description.unwrap_or_default();
        return Err(AppError::BadRequest(format!("oidc error: {err} {desc}")));
    }

    let oidc = state
        .oidc
        .as_ref()
        .ok_or(AppError::OidcNotConfigured)?;

    let code = q
        .code
        .ok_or_else(|| AppError::BadRequest("missing code".into()))?;
    let returned_state = q
        .state
        .ok_or_else(|| AppError::BadRequest("missing state".into()))?;

    let expected = jar
        .get(STATE_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or_else(|| AppError::BadRequest("missing state cookie".into()))?;
    if expected != returned_state {
        return Err(AppError::BadRequest("state mismatch".into()));
    }

    let token = oidc.exchange_code(&code).await?;
    let user_info = oidc.fetch_userinfo(&token.access_token).await?;

    upsert_user(&state.db, &user_info).await?;

    let session = SessionToken::new(
        user_info.sub.clone(),
        user_info.email.clone(),
        user_info.display_name(),
        user_info.avatar_url(),
    );

    let session_token = session
        .encode(&state.cfg.session_secret)
        .map_err(AppError::Other)?;

    let session_cookie = Cookie::build((SESSION_COOKIE, session_token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::days(30))
        .build();

    let cleared_state = Cookie::build((STATE_COOKIE, ""))
        .path("/")
        .max_age(time::Duration::ZERO)
        .build();

    let updated_jar = jar.add(session_cookie).add(cleared_state);
    Ok((updated_jar, Redirect::to("/")).into_response())
}

pub async fn logout(jar: CookieJar) -> Response {
    let cleared = Cookie::build((SESSION_COOKIE, ""))
        .path("/")
        .max_age(time::Duration::ZERO)
        .build();
    (jar.add(cleared), Redirect::to("/login")).into_response()
}

#[derive(Debug, Deserialize)]
pub struct EmailLoginPayload {
    pub email: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// Password-less email login for non-Lazycat deployments.
///
/// "现阶段认证意义不大" — we do format validation only, no email
/// verification. The submitted address becomes the stable user
/// identity (`email:<lowercased>`), distinct from any OIDC `sub`,
/// and is upserted into the same `users` table as OIDC accounts.
pub async fn email_login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(payload): Json<EmailLoginPayload>,
) -> AppResult<Response> {
    if !state.cfg.email_login_enabled {
        return Err(AppError::EmailLoginDisabled);
    }

    let email = normalize_email(&payload.email)
        .ok_or_else(|| AppError::BadRequest("invalid email address".into()))?;

    let user_id = format!("email:{email}");
    let display_name = payload
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| email.split('@').next().map(str::to_string));

    sqlx::query(
        r#"
        INSERT INTO users (id, email, name, avatar_url)
        VALUES (?1, ?2, ?3, NULL)
        ON CONFLICT(id) DO UPDATE SET
            email = excluded.email,
            name = COALESCE(excluded.name, users.name),
            updated_at = datetime('now')
        "#,
    )
    .bind(&user_id)
    .bind(&email)
    .bind(&display_name)
    .execute(&state.db)
    .await?;

    let session = SessionToken::new(
        user_id.clone(),
        Some(email.clone()),
        display_name.clone(),
        None,
    );
    let session_token = session
        .encode(&state.cfg.session_secret)
        .map_err(AppError::Other)?;

    let session_cookie = Cookie::build((SESSION_COOKIE, session_token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::days(30))
        .build();

    let body = Json(json!({
        "ok": true,
        "user": {
            "id": user_id,
            "email": email,
            "name": display_name,
            "avatar_url": null,
        }
    }));
    Ok((jar.add(session_cookie), body).into_response())
}

fn normalize_email(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.len() < 3 || trimmed.len() > 254 {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    let (local, domain) = lower.split_once('@')?;
    if local.is_empty() || domain.is_empty() || !domain.contains('.') {
        return None;
    }
    if domain.starts_with('.') || domain.ends_with('.') || domain.contains("..") {
        return None;
    }
    let valid = lower.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(c, '@' | '.' | '_' | '-' | '+')
    });
    valid.then_some(lower)
}

async fn upsert_user(
    db: &sqlx::SqlitePool,
    info: &crate::auth::oidc::UserInfo,
) -> AppResult<()> {
    let name = info.display_name();
    let avatar = info.avatar_url();

    sqlx::query(
        r#"
        INSERT INTO users (id, email, name, avatar_url)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(id) DO UPDATE SET
            email = excluded.email,
            name = excluded.name,
            avatar_url = excluded.avatar_url,
            updated_at = datetime('now')
        "#,
    )
    .bind(&info.sub)
    .bind(&info.email)
    .bind(&name)
    .bind(&avatar)
    .execute(db)
    .await?;

    Ok(())
}
