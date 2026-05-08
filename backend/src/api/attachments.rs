//! Resume attachment endpoints.
//!
//! - `POST   /api/sessions/{id}/attachments`       multipart upload
//! - `GET    /api/sessions/{id}/attachments`       list metadata
//! - `GET    /api/sessions/{id}/attachments/{aid}` metadata + extracted preview
//! - `GET    /api/sessions/{id}/attachments/{aid}/download` original bytes
//! - `DELETE /api/sessions/{id}/attachments/{aid}` remove

use std::path::PathBuf;

use axum::body::Body;
use axum::extract::{Multipart, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::extract::{self, AttachmentKind};
use crate::AppState;

/// Hard limits to keep a single SQLite DB happy and the LLM prompt bounded.
const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_ATTACHMENTS_PER_SESSION: i64 = 5;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AttachmentRow {
    pub id: String,
    pub session_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub storage_path: String,
    pub extract_status: String,
    pub extract_error: Option<String>,
    pub sha256: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct AttachmentSummary {
    pub id: String,
    pub session_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub kind: &'static str,
    pub extract_status: String,
    pub extract_error: Option<String>,
    pub text_chars: usize,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct AttachmentDetail {
    #[serde(flatten)]
    pub summary: AttachmentSummary,
    /// First ~1000 chars of the extracted text so the UI can show a preview
    /// without paying for the full payload. `text_chars` is the full count.
    pub text_preview: String,
}

fn kind_label(filename: &str, mime: &str) -> &'static str {
    match AttachmentKind::detect(filename, mime) {
        Some(k) => k.label(),
        None => "其他",
    }
}

fn summary(row: &AttachmentRow, text_chars: usize) -> AttachmentSummary {
    AttachmentSummary {
        id: row.id.clone(),
        session_id: row.session_id.clone(),
        filename: row.filename.clone(),
        mime_type: row.mime_type.clone(),
        size_bytes: row.size_bytes,
        kind: kind_label(&row.filename, &row.mime_type),
        extract_status: row.extract_status.clone(),
        extract_error: row.extract_error.clone(),
        text_chars,
        created_at: row.created_at.clone(),
    }
}

#[derive(Debug, sqlx::FromRow)]
struct AttachmentListRow {
    pub id: String,
    pub session_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub storage_path: String,
    pub extract_status: String,
    pub extract_error: Option<String>,
    pub sha256: String,
    pub created_at: String,
    pub text_chars: i64,
}

impl AttachmentListRow {
    fn into_summary(self) -> AttachmentSummary {
        let row = AttachmentRow {
            id: self.id,
            session_id: self.session_id,
            filename: self.filename,
            mime_type: self.mime_type,
            size_bytes: self.size_bytes,
            storage_path: self.storage_path,
            extract_status: self.extract_status,
            extract_error: self.extract_error,
            sha256: self.sha256,
            created_at: self.created_at,
        };
        summary(&row, self.text_chars.max(0) as usize)
    }
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(session_id): Path<String>,
) -> AppResult<Json<Vec<AttachmentSummary>>> {
    own_session(&state.db, &user.id, &session_id).await?;

    let rows = sqlx::query_as::<_, AttachmentListRow>(
        r#"
        SELECT id, session_id, filename, mime_type, size_bytes, storage_path,
               extract_status, extract_error, sha256, created_at,
               LENGTH(extracted_text) AS text_chars
          FROM resume_attachments
         WHERE session_id = ?1
         ORDER BY created_at DESC
        "#,
    )
    .bind(&session_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(AttachmentListRow::into_summary)
    .collect();

    Ok(Json(rows))
}

#[derive(Debug, sqlx::FromRow)]
struct AttachmentDetailRow {
    pub id: String,
    pub session_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub storage_path: String,
    pub extract_status: String,
    pub extract_error: Option<String>,
    pub sha256: String,
    pub created_at: String,
    pub extracted_text: String,
}

pub async fn get(
    State(state): State<AppState>,
    user: AuthUser,
    Path((session_id, attachment_id)): Path<(String, String)>,
) -> AppResult<Json<AttachmentDetail>> {
    own_session(&state.db, &user.id, &session_id).await?;

    let detail = sqlx::query_as::<_, AttachmentDetailRow>(
        r#"
        SELECT id, session_id, filename, mime_type, size_bytes, storage_path,
               extract_status, extract_error, sha256, created_at, extracted_text
          FROM resume_attachments
         WHERE session_id = ?1 AND id = ?2
        "#,
    )
    .bind(&session_id)
    .bind(&attachment_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let text_chars = detail.extracted_text.chars().count();
    let preview: String = detail.extracted_text.chars().take(1_000).collect();

    let row = AttachmentRow {
        id: detail.id,
        session_id: detail.session_id,
        filename: detail.filename,
        mime_type: detail.mime_type,
        size_bytes: detail.size_bytes,
        storage_path: detail.storage_path,
        extract_status: detail.extract_status,
        extract_error: detail.extract_error,
        sha256: detail.sha256,
        created_at: detail.created_at,
    };

    Ok(Json(AttachmentDetail {
        summary: summary(&row, text_chars),
        text_preview: preview,
    }))
}

pub async fn download(
    State(state): State<AppState>,
    user: AuthUser,
    Path((session_id, attachment_id)): Path<(String, String)>,
) -> AppResult<Response> {
    own_session(&state.db, &user.id, &session_id).await?;

    let row: AttachmentRow = sqlx::query_as(
        r#"
        SELECT id, session_id, filename, mime_type, size_bytes, storage_path,
               extract_status, extract_error, sha256, created_at
          FROM resume_attachments
         WHERE session_id = ?1 AND id = ?2
        "#,
    )
    .bind(&session_id)
    .bind(&attachment_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let bytes = tokio::fs::read(&row.storage_path)
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("read attachment file: {e}")))?;

    let disposition = format!(
        "attachment; filename*=UTF-8''{}",
        percent_encode(&row.filename)
    );

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, row.mime_type.clone()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        Body::from(bytes),
    )
        .into_response())
}

pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path((session_id, attachment_id)): Path<(String, String)>,
) -> AppResult<Json<serde_json::Value>> {
    own_session(&state.db, &user.id, &session_id).await?;

    let row: Option<AttachmentRow> = sqlx::query_as(
        r#"
        SELECT id, session_id, filename, mime_type, size_bytes, storage_path,
               extract_status, extract_error, sha256, created_at
          FROM resume_attachments
         WHERE session_id = ?1 AND id = ?2
        "#,
    )
    .bind(&session_id)
    .bind(&attachment_id)
    .fetch_optional(&state.db)
    .await?;

    let Some(row) = row else {
        return Err(AppError::NotFound);
    };

    sqlx::query("DELETE FROM resume_attachments WHERE id = ?1")
        .bind(&attachment_id)
        .execute(&state.db)
        .await?;

    // Best-effort cleanup; if it fails the record is already gone so the
    // file is just orphaned in /data/uploads.
    let _ = tokio::fs::remove_file(&row.storage_path).await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn upload(
    State(state): State<AppState>,
    user: AuthUser,
    Path(session_id): Path<String>,
    mut multipart: Multipart,
) -> AppResult<Json<AttachmentSummary>> {
    own_session(&state.db, &user.id, &session_id).await?;

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM resume_attachments WHERE session_id = ?1")
            .bind(&session_id)
            .fetch_one(&state.db)
            .await?;
    if count >= MAX_ATTACHMENTS_PER_SESSION {
        return Err(AppError::BadRequest(format!(
            "每个对话最多上传 {MAX_ATTACHMENTS_PER_SESSION} 个简历附件，请先删除旧的"
        )));
    }

    let mut filename: Option<String> = None;
    let mut mime: Option<String> = None;
    let mut bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("multipart 解析失败：{e}")))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name != "file" {
            continue;
        }

        let fname = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "resume".to_string());
        let ftype = field
            .content_type()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("读取上传文件失败：{e}")))?;

        if data.len() as u64 > MAX_FILE_BYTES {
            return Err(AppError::BadRequest(format!(
                "文件超过 {} MB 上限",
                MAX_FILE_BYTES / 1024 / 1024
            )));
        }
        if data.is_empty() {
            return Err(AppError::BadRequest("文件为空".into()));
        }

        filename = Some(fname);
        mime = Some(ftype);
        bytes = Some(data.to_vec());
        break;
    }

    let filename = filename.ok_or_else(|| AppError::BadRequest("缺少 file 字段".into()))?;
    let mime = mime.unwrap_or_else(|| "application/octet-stream".to_string());
    let bytes = bytes.ok_or_else(|| AppError::BadRequest("文件为空".into()))?;

    let kind = AttachmentKind::detect(&filename, &mime).ok_or_else(|| {
        AppError::BadRequest("仅支持 PDF / DOCX / TXT / Markdown 格式".into())
    })?;

    let outcome = extract::extract(kind, &bytes);

    let id = Uuid::new_v4().to_string();
    let sha = Sha256::digest(&bytes);
    let sha_hex: String = sha.iter().map(|b| format!("{b:02x}")).collect();

    let upload_dir: PathBuf = PathBuf::from(&state.cfg.data_dir)
        .join("uploads")
        .join(&session_id);
    tokio::fs::create_dir_all(&upload_dir)
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("create upload dir: {e}")))?;

    let storage_path = upload_dir.join(format!("{id}.bin"));
    tokio::fs::write(&storage_path, &bytes)
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("write upload: {e}")))?;
    let storage_path_str = storage_path.to_string_lossy().to_string();

    let size_bytes = bytes.len() as i64;
    let status_str = outcome.status.as_str();

    sqlx::query(
        r#"
        INSERT INTO resume_attachments (
            id, session_id, filename, mime_type, size_bytes,
            storage_path, extracted_text, extract_status, extract_error, sha256
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(&id)
    .bind(&session_id)
    .bind(&filename)
    .bind(&mime)
    .bind(size_bytes)
    .bind(&storage_path_str)
    .bind(&outcome.text)
    .bind(status_str)
    .bind(outcome.error.as_deref())
    .bind(&sha_hex)
    .execute(&state.db)
    .await?;

    sqlx::query("UPDATE sessions SET updated_at = datetime('now') WHERE id = ?1")
        .bind(&session_id)
        .execute(&state.db)
        .await?;

    let row: AttachmentRow = sqlx::query_as(
        r#"
        SELECT id, session_id, filename, mime_type, size_bytes, storage_path,
               extract_status, extract_error, sha256, created_at
          FROM resume_attachments
         WHERE id = ?1
        "#,
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(summary(&row, outcome.text.chars().count())))
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

fn percent_encode(input: &str) -> String {
    // RFC 5987 — keep ASCII alphanumerics and a few safe chars, percent-encode the rest.
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        let c = *byte;
        let safe = c.is_ascii_alphanumeric()
            || matches!(c, b'-' | b'_' | b'.' | b'~');
        if safe {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    out
}
