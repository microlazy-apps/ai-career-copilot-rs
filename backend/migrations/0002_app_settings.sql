-- Per-instance app settings configured from the in-app Settings page.
-- One row per settings scope. The default scope is 'global' (single-user
-- box). user-scoped overrides can be added later.
CREATE TABLE IF NOT EXISTS app_settings (
    scope TEXT PRIMARY KEY,
    llm_base_url TEXT NOT NULL DEFAULT 'https://api.deepseek.com/v1',
    llm_api_key TEXT NOT NULL DEFAULT '',
    llm_model TEXT NOT NULL DEFAULT 'deepseek-chat',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO app_settings (scope) VALUES ('global')
ON CONFLICT(scope) DO NOTHING;
