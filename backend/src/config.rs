use std::env;

/// Runtime configuration assembled from env vars.
///
/// Lazycat injects the `LAZYCAT_AUTH_OIDC_*` and `LAZYCAT_APP_DOMAIN`
/// vars at install time when `application.oidc_redirect_path` is set
/// in the lpk manifest, so the app does not need user-supplied OIDC
/// credentials.
///
/// LLM credentials are *not* part of this struct — those are stored in
/// SQLite (`app_settings`) and edited from the in-app Settings page.
/// `EnvDefaults` only carries optional seed values that pre-populate the
/// settings row on first boot for convenience.
#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub data_dir: String,
    pub database_url: String,
    pub session_secret: String,
    pub oidc: Option<OidcConfig>,
    /// When true, expose `POST /auth/email/login` as a no-password
    /// login fallback for environments without Lazycat OIDC.
    pub email_login_enabled: bool,
    pub app_domain: String,
    pub env_defaults: EnvDefaults,
}

#[derive(Debug, Clone)]
pub struct OidcConfig {
    pub client_id: String,
    pub client_secret: String,
    pub auth_uri: String,
    pub token_uri: String,
    pub userinfo_uri: Option<String>,
    pub redirect_url: String,
}

#[derive(Debug, Clone, Default)]
pub struct EnvDefaults {
    pub llm_base_url: Option<String>,
    pub llm_api_key: Option<String>,
    pub llm_model: Option<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
        let data_dir = env::var("DATA_DIR").unwrap_or_else(|_| "./data".into());

        std::fs::create_dir_all(&data_dir).ok();

        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| format!("sqlite://{data_dir}/app.db?mode=rwc"));

        let session_secret = env::var("SESSION_SECRET")
            .unwrap_or_else(|_| "change-me-in-production-please".into());

        let app_domain = env::var("LAZYCAT_APP_DOMAIN")
            .or_else(|_| env::var("APP_DOMAIN"))
            .unwrap_or_else(|_| "localhost:8080".into());

        let oidc = oidc_from_env(&app_domain);

        // Email login is the fallback when Lazycat OIDC is absent: lpk
        // installs always have OIDC, so they never see it; everyone
        // else (docker compose / cargo run) gets it automatically.
        let email_login_enabled = oidc.is_none();

        let env_defaults = EnvDefaults {
            llm_base_url: env::var("LLM_BASE_URL").ok().filter(|s| !s.is_empty()),
            llm_api_key: env::var("LLM_API_KEY").ok().filter(|s| !s.is_empty()),
            llm_model: env::var("LLM_MODEL").ok().filter(|s| !s.is_empty()),
        };

        Ok(Self {
            bind_addr,
            data_dir,
            database_url,
            session_secret,
            oidc,
            email_login_enabled,
            app_domain,
            env_defaults,
        })
    }
}

fn oidc_from_env(app_domain: &str) -> Option<OidcConfig> {
    let client_id = env::var("LAZYCAT_AUTH_OIDC_CLIENT_ID").ok()?;
    let client_secret = env::var("LAZYCAT_AUTH_OIDC_CLIENT_SECRET").ok()?;
    let auth_uri = env::var("LAZYCAT_AUTH_OIDC_AUTH_URI").ok()?;
    let token_uri = env::var("LAZYCAT_AUTH_OIDC_TOKEN_URI").ok()?;

    if client_id.is_empty() || client_secret.is_empty() {
        return None;
    }

    let userinfo_uri = env::var("LAZYCAT_AUTH_OIDC_USERINFO_URI").ok();

    let redirect_url = env::var("LAZYCAT_AUTH_OIDC_REDIRECT_URL").unwrap_or_else(|_| {
        // Lazycat's `oidc_redirect_path` lands the user back here.
        format!("https://{app_domain}/auth/oidc/callback")
    });

    Some(OidcConfig {
        client_id,
        client_secret,
        auth_uri,
        token_uri,
        userinfo_uri,
        redirect_url,
    })
}
