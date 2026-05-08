-- Resume attachments uploaded by the user (PDF / DOCX / TXT / MD).
-- The original file is persisted to disk under <data_dir>/uploads/<session_id>/<id>.bin;
-- we keep an extracted plain-text copy in `extracted_text` so the LLM
-- prompt can be enriched without re-parsing on every chat turn.
CREATE TABLE IF NOT EXISTS resume_attachments (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    storage_path TEXT NOT NULL,
    extracted_text TEXT NOT NULL DEFAULT '',
    extract_status TEXT NOT NULL DEFAULT 'ok' CHECK (extract_status IN ('ok', 'partial', 'failed')),
    extract_error TEXT,
    sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_resume_attachments_session_created
    ON resume_attachments(session_id, created_at DESC);
