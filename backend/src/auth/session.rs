use axum::extract::{FromRef, FromRequestParts, OptionalFromRequestParts};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

pub const SESSION_COOKIE: &str = "ac_session";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionToken {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub exp: i64,
    pub iat: i64,
}

impl SessionToken {
    pub fn new(
        sub: String,
        email: Option<String>,
        name: Option<String>,
        avatar_url: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            sub,
            email,
            name,
            avatar_url,
            iat: now.timestamp(),
            exp: (now + Duration::days(30)).timestamp(),
        }
    }

    pub fn encode(&self, secret: &str) -> anyhow::Result<String> {
        let token = encode(
            &Header::default(),
            self,
            &EncodingKey::from_secret(secret.as_bytes()),
        )?;
        Ok(token)
    }

    pub fn decode(token: &str, secret: &str) -> anyhow::Result<Self> {
        let data = decode::<Self>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )?;
        Ok(data.claims)
    }
}

/// Authenticated user extractor. Reads either:
/// - the `ac_session` cookie (our own JWT issued after OIDC callback), or
/// - the `x-hc-user-id` header injected by the lazycat box for SSO requests.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    crate::AppState: FromRef<S>,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = <crate::AppState as FromRef<S>>::from_ref(state);

        // 1. Lazycat-injected SSO header (anonymous reverse-proxy auth).
        if let Some(value) = parts.headers.get("x-hc-user-id") {
            if let Ok(uid) = value.to_str() {
                if !uid.is_empty() {
                    return Ok(AuthUser {
                        id: uid.to_string(),
                        email: None,
                        name: None,
                        avatar_url: None,
                    });
                }
            }
        }

        // 2. Our own session cookie set after OIDC exchange.
        let jar = CookieJar::from_headers(&parts.headers);
        if let Some(cookie) = jar.get(SESSION_COOKIE) {
            if let Ok(claims) = SessionToken::decode(cookie.value(), &app_state.cfg.session_secret) {
                return Ok(AuthUser {
                    id: claims.sub,
                    email: claims.email,
                    name: claims.name,
                    avatar_url: claims.avatar_url,
                });
            }
        }

        Err((StatusCode::UNAUTHORIZED, "not authenticated").into_response())
    }
}

impl<S> OptionalFromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    crate::AppState: FromRef<S>,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        Ok(<AuthUser as FromRequestParts<S>>::from_request_parts(parts, state).await.ok())
    }
}
