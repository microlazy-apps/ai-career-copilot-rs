//! Lazycat OIDC client.
//!
//! Lazycat injects four URIs at install time
//! (`LAZYCAT_AUTH_OIDC_AUTH_URI`, `_TOKEN_URI`, `_USERINFO_URI`, plus
//! a client_id/secret pair) and reverse-proxies the redirect path
//! configured under `application.oidc_redirect_path` in the manifest.
//! There is no OIDC discovery doc — we talk to the three endpoints
//! directly with reqwest.

use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::config::OidcConfig;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct OidcClient {
    cfg: OidcConfig,
    http: reqwest::Client,
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub expires_in: Option<i64>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserInfo {
    pub sub: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub preferred_username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
}

impl UserInfo {
    pub fn display_name(&self) -> Option<String> {
        self.name
            .clone()
            .or_else(|| self.preferred_username.clone())
    }

    pub fn avatar_url(&self) -> Option<String> {
        self.picture.clone().or_else(|| self.avatar.clone())
    }
}

impl OidcClient {
    pub fn new(cfg: OidcConfig) -> Self {
        Self {
            cfg,
            http: reqwest::Client::new(),
        }
    }

    pub fn redirect_url(&self) -> &str {
        &self.cfg.redirect_url
    }

    pub fn authorize_url(&self, state: &str) -> String {
        let scope = "openid profile email";
        let mut url = url::Url::parse(&self.cfg.auth_uri).expect("valid auth_uri");
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.cfg.client_id)
            .append_pair("redirect_uri", &self.cfg.redirect_url)
            .append_pair("scope", scope)
            .append_pair("state", state);
        url.to_string()
    }

    pub async fn exchange_code(&self, code: &str) -> AppResult<TokenResponse> {
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", self.cfg.redirect_url.as_str()),
            ("client_id", self.cfg.client_id.as_str()),
            ("client_secret", self.cfg.client_secret.as_str()),
        ];

        let resp = self
            .http
            .post(&self.cfg.token_uri)
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Other(anyhow::anyhow!("token request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Other(anyhow::anyhow!(
                "token endpoint returned {status}: {body}"
            )));
        }

        resp.json::<TokenResponse>()
            .await
            .map_err(|e| AppError::Other(anyhow::anyhow!("decode token response: {e}")))
    }

    pub async fn fetch_userinfo(&self, access_token: &str) -> AppResult<UserInfo> {
        let url = self
            .cfg
            .userinfo_uri
            .as_deref()
            .ok_or_else(|| AppError::Other(anyhow::anyhow!("userinfo_uri not configured")))?;

        let resp = self
            .http
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| AppError::Other(anyhow::anyhow!("userinfo request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Other(anyhow::anyhow!(
                "userinfo endpoint returned {status}: {body}"
            )));
        }

        resp.json::<UserInfo>()
            .await
            .map_err(|e| AppError::Other(anyhow::anyhow!("decode userinfo: {e}")))
    }
}

pub fn random_state() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}
