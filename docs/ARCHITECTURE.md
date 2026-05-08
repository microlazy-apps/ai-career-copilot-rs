# Architecture notes

## Why Rust + SQLite (and why not a Python copy)

The Python reference (LangGraph + FastAPI + MySQL + Redis) is well-suited
for research and prototyping, but turns into a four-service stack the moment
you try to self-host it. For a **single-user / family-Lazycat deployment**
those moving parts are pure tax:

| Concern | Reference (Python) | This repo (Rust) |
|---|---|---|
| Web server | FastAPI / uvicorn | axum 0.8 |
| Agent orchestration | LangGraph multi-agent | Single chat agent + system prompt |
| Persistent state | MySQL + Redis | SQLite (WAL) |
| Container count | web + backend + mysql + redis | 1 |
| Cold start | ~6–10 s | ~150 ms |
| Image size | ~600 MB+ | ~70 MB |

The Lazycat package only needs to remember "who is logged in" and "what did
we say last time" — a single binary against a WAL-mode SQLite is more than
enough, and it makes the **persistent-history requirement** (the user's main
pain point) trivial to guarantee.

## Module map (`backend/src/`)

```
main.rs            wires AppState, routes, embedded Vue dist;
                   calls settings::seed_from_env() once on boot
config.rs          env → Config struct, including Lazycat OIDC vars
                   and EnvDefaults (seed values for app_settings)
db.rs              sqlx pool + sqlx::migrate!() runner
settings.rs        AppSettings load/update/seed_from_env over app_settings table

auth/
  oidc.rs          minimal OIDC client over reqwest (no discovery doc)
  session.rs       AuthUser extractor (cookie JWT or x-hc-user-id header)

api/
  auth.rs          /auth/oidc/login, /auth/oidc/callback, /auth/me, /auth/logout
  sessions.rs      /api/sessions CRUD
  messages.rs      /api/sessions/{id}/messages — SSE streaming chat
  resume.rs        /api/sessions/{id}/resume — structured resume JSON
  attachments.rs   /api/sessions/{id}/attachments — multipart upload + manage
  settings.rs      /api/settings (GET/PUT) + /api/settings/test — in-app LLM config

llm/
  mod.rs           system prompt + re-exports
  openai.rs        OpenAI-compatible streaming chat client

extract.rs         PDF / DOCX / TXT / Markdown text extraction (capped at 32k chars)
models.rs          sqlx::FromRow structs for users / sessions / messages / resume_contents
embed.rs           rust-embed of frontend/dist + SPA fallback
error.rs           AppError → IntoResponse
```

## Authentication flow

The Lazycat appstore manifest sets `application.oidc_redirect_path: /auth/oidc/callback`,
which makes Lazycat:

1. Issue a per-installation OAuth2 client (id + secret).
2. Inject `LAZYCAT_AUTH_OIDC_*` env vars at container start.
3. Reverse-proxy `/auth/oidc/callback` on our subdomain back into the container.

The flow is:

```
browser → GET /auth/oidc/login
         → 302 to LAZYCAT_AUTH_OIDC_AUTH_URI (Lazycat account portal)
         ← 302 back to /auth/oidc/callback?code=...&state=...
         → POST LAZYCAT_AUTH_OIDC_TOKEN_URI (form-urlencoded)
         → GET LAZYCAT_AUTH_OIDC_USERINFO_URI (Bearer token)
         → upsert into users table
         → set ac_session JWT cookie (HMAC-signed, 30d)
         → 302 to /
```

The same `AuthUser` extractor also accepts the `x-hc-user-id` header that
Lazycat injects on every request (when the box has done SSO at the proxy
layer), so even if our cookie is missing, requests still resolve to a user
identity.

## Refresh-safe chat history

Every user message is `INSERT`ed into `messages` **before** the LLM call
starts. The assistant message is created when the SSE stream completes
(or on first delta if you want partial drafts to survive crashes — easy
follow-up).

The frontend never relies on in-memory state for the message list:

- on page mount, the router parses `:id` from the URL,
- `selectSession(id)` calls `GET /api/sessions/:id/messages`,
- the `<MessageList>` component renders directly from the store.

This means a hard refresh in the middle of a streaming reply just shows the
partial message that has already been persisted (if any), and lets the user
continue the conversation in the next turn — no "first paint is empty"
regression like in the original FastAPI implementation.

## 应用内设置 (app_settings)

LLM 凭据（base URL / API key / 模型）**不**通过懒猫部署参数或环境变量做长期配置 — 那条路径会让用户每次想换模型都要 redeploy。我们把它放进数据库：

```sql
-- migrations/0002_app_settings.sql
CREATE TABLE app_settings (
    scope        TEXT PRIMARY KEY,                  -- 'global' (single row for now)
    llm_base_url TEXT NOT NULL DEFAULT 'https://api.deepseek.com/v1',
    llm_api_key  TEXT NOT NULL DEFAULT '',
    llm_model    TEXT NOT NULL DEFAULT 'deepseek-chat',
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Read / write surface

| Endpoint | Purpose | Notes |
|---|---|---|
| `GET /api/settings` | 读取设置给 UI | 返回 `AppSettingsView`：API key 只回 `****abcd` 末四位 hint，原值不出库 |
| `PUT /api/settings` | 增量更新 | `llm_base_url` / `llm_model` / `llm_api_key` 都是 `Option<String>`：`None` = 不变；`Some("")` = 清空 |
| `POST /api/settings/test` | 真实联调 | 用当前配置打一次 16-token chat completion；返回模型回复，前端做"测试连通"按钮 |

### Env 作为种子，不作为配置

`config.rs::EnvDefaults` 收集 `LLM_BASE_URL` / `LLM_API_KEY` / `LLM_MODEL` 三个 env，启动时调用 `settings::seed_from_env`：

- 如果 `app_settings` 里 `llm_api_key` 还是空、`llm_base_url` / `llm_model` 还是出厂默认 → 用 env 填进去（首次启动方便）
- 一旦 UI 改过 → 已经不是出厂默认 → env **不再覆盖** 数据库

这套机制让两类用户都能舒服：

- **lpk 安装** → env 全空 → 登录后到 UI 填 → 一切都在数据库里
- **`docker compose up`** → 想用 env 就在 `.env` 里填，开箱启动；想换模型就到 UI 里改，env 失效不影响

### 前端

`frontend/src/components/SettingsModal.vue` 是入口。Provider 预设 / 状态徽章 / 卡片分组 / 测试连通 / dirty-aware 保存按钮等设计细节，详见 [README#配置](../README.md#配置)。

## 简历附件 (resume_attachments)

用户可以为每个会话上传至多 5 份简历附件（PDF / DOCX / TXT / Markdown，单文件 ≤ 10 MB）。后端把原始字节落到 `<DATA_DIR>/uploads/<session_id>/<id>.bin`，把抽取出来的纯文本存进 SQLite。每轮 chat 之前都会读这张表，把所有附件文本拼成一条 system message 注入到 prompt 头部 — 不需要做向量检索，对单人/家庭场景是性价比最高的方案。

```sql
-- migrations/0004_resume_attachments.sql
CREATE TABLE resume_attachments (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    storage_path TEXT NOT NULL,
    extracted_text TEXT NOT NULL DEFAULT '',
    extract_status TEXT NOT NULL CHECK (extract_status IN ('ok','partial','failed')),
    extract_error TEXT,
    sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL
);
```

### Endpoints

| Endpoint | Purpose |
|---|---|
| `POST   /api/sessions/{id}/attachments` | multipart upload，字段名 `file` |
| `GET    /api/sessions/{id}/attachments` | 列出附件元信息（不含正文） |
| `GET    /api/sessions/{id}/attachments/{aid}` | 元信息 + 1000 字预览 |
| `GET    /api/sessions/{id}/attachments/{aid}/download` | 下载原始文件 |
| `DELETE /api/sessions/{id}/attachments/{aid}` | 删除（同时清理磁盘文件） |

### 抽取策略 (`extract.rs`)

- **TXT / MD**：UTF-8 → GBK fallback。
- **PDF**：`pdf-extract` crate；包了 `catch_unwind` 防御被异常 PDF 触发 panic（解析失败会落库 `extract_status='failed'`，UI 显示红色徽章但不会丢文件）。
- **DOCX**：解 zip → 读 `word/document.xml` → quick-xml 仅保留 `<w:t>` 文本节点，按 `<w:p>` 加换行。
- 统一规整化：去掉空白尾部、合并连续空行；超过 32000 字截断（`extract_status='partial'`）。

### Prompt 注入 (`api/messages.rs::build_resume_context`)

每轮 chat 时按上传时间排序拼接 `【附件 N】文件名 + 文本`，总预算 24000 字符。如果某份解析失败，仍然会把文件名写进 prompt 让 AI 知道用户上传了什么但需要追问。这条 system message 跟 JD system message 一起，在普通历史消息之前插入。

## Why one binary serves the frontend too

`rust-embed` walks `../frontend/dist` at compile time and bakes the files
into the binary. The fallback handler in `embed.rs` returns the asset for
any path that the API/auth routers haven't already matched, and falls back
to `index.html` so client-side router paths like `/c/:id` work after a
refresh.

This collapses the Lazycat manifest down to **one upstream service**, so
there's nothing to coordinate between Next.js + Go like the multica wrapper
has to.
