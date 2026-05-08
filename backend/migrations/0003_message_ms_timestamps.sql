-- Use millisecond-precision timestamps for messages so user / assistant
-- pairs created within the same second still sort in insertion order.
-- (created_at defaults are baked into the row at INSERT time, so existing
-- rows are left untouched — the new format only applies to future writes.)
CREATE TABLE IF NOT EXISTS _messages_new (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant', 'tool')),
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO _messages_new (id, session_id, role, content, created_at)
SELECT id, session_id, role, content, created_at FROM messages;

DROP TABLE messages;
ALTER TABLE _messages_new RENAME TO messages;

CREATE INDEX IF NOT EXISTS idx_messages_session_created
    ON messages(session_id, created_at);
