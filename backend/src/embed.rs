//! Serve the Vue dist bundle. The frontend ships a single SPA, so any
//! unknown path falls back to `index.html` — but only AFTER the API
//! and OIDC routes have already been matched.

use axum::body::Body;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../frontend/dist/"]
struct Assets;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if !path.is_empty() {
        if let Some(file) = Assets::get(path) {
            return build(path, file);
        }
    }

    if let Some(index) = Assets::get("index.html") {
        return build("index.html", index);
    }

    (
        StatusCode::NOT_FOUND,
        "frontend bundle is missing — run `pnpm --dir frontend build` first",
    )
        .into_response()
}

fn build(path: &str, file: rust_embed::EmbeddedFile) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let bytes = file.data.into_owned();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(Body::from(bytes))
        .unwrap()
}
