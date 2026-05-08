use std::sync::Arc;

use axum::extract::FromRef;
use axum::routing::{get, post};
use axum::Router;
use sqlx::SqlitePool;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

mod api;
mod auth;
mod config;
mod db;
mod embed;
mod error;
mod extract;
mod llm;
mod models;
mod settings;

use auth::oidc::OidcClient;
use config::Config;
use llm::OpenAiClient;

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub db: SqlitePool,
    pub oidc: Option<Arc<OidcClient>>,
    pub llm: Arc<OpenAiClient>,
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    init_tracing();

    let cfg = Config::from_env()?;
    tracing::info!(
        bind = %cfg.bind_addr,
        data_dir = %cfg.data_dir,
        oidc_configured = cfg.oidc.is_some(),
        "starting ai-career-copilot",
    );

    let db = db::connect(&cfg.database_url).await?;
    settings::seed_from_env(&db, &cfg.env_defaults).await?;

    let oidc = cfg.oidc.clone().map(|c| Arc::new(OidcClient::new(c)));
    let llm = Arc::new(OpenAiClient::new());

    let state = AppState {
        cfg: Arc::new(cfg.clone()),
        db,
        oidc,
        llm,
    };

    let api_routes = Router::new()
        .route("/sessions", get(api::sessions::list).post(api::sessions::create))
        .route(
            "/sessions/{id}",
            get(api::sessions::get)
                .patch(api::sessions::update)
                .delete(api::sessions::delete),
        )
        .route(
            "/sessions/{id}/messages",
            get(api::messages::list).post(api::messages::stream),
        )
        .route(
            "/sessions/{id}/resume",
            get(api::resume::get).put(api::resume::upsert),
        )
        .route(
            "/sessions/{id}/attachments",
            get(api::attachments::list).post(api::attachments::upload),
        )
        .route(
            "/sessions/{id}/attachments/{aid}",
            get(api::attachments::get).delete(api::attachments::delete),
        )
        .route(
            "/sessions/{id}/attachments/{aid}/download",
            get(api::attachments::download),
        )
        .route(
            "/settings",
            get(api::settings::get).put(api::settings::update),
        )
        .route("/settings/test", post(api::settings::test_llm))
        .layer(axum::extract::DefaultBodyLimit::max(12 * 1024 * 1024));

    let auth_routes = Router::new()
        .route("/oidc/login", get(api::auth::login))
        .route("/oidc/callback", get(api::auth::callback))
        .route("/email/login", post(api::auth::email_login))
        .route("/me", get(api::auth::me))
        .route("/logout", post(api::auth::logout));

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .nest("/api", api_routes)
        .nest("/auth", auth_routes)
        .fallback(embed::serve)
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    tracing::info!("listening on http://{}", cfg.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}
