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
  auth.rs          /auth/oidc/{login,callback}, /auth/email/login,
                   /auth/me (with `methods` advert), /auth/logout
  sessions.rs     /api/sessions CRUD
  messages.rs     /api/sessions/{id}/messages — SSE streaming chat
  resume.rs       /api/sessions/{id}/resume — structured resume JSON
  settings.rs     /api/settings (GET/PUT) + /api/settings/test — in-app LLM config

llm/
  mod.rs           system prompt + re-exports
  openai.rs        OpenAI-compatible streaming chat client

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

### Email login (non-Lazycat fallback)

Outside Lazycat the `LAZYCAT_AUTH_OIDC_*` vars are absent, so OIDC is
disabled. To keep the app usable in `docker compose` / `cargo run`
deployments, `Config::from_env` flips on `email_login_enabled` whenever
no OIDC is configured (override with `EMAIL_LOGIN=1/0`).

The endpoint is intentionally minimal — issue MIC-5 explicitly framed
this as "现阶段认证意义不大":

```
browser → POST /auth/email/login {email}
         → format-validate + lowercase
         → upsert users row with id = "email:<addr>"
         → set ac_session JWT (same cookie as OIDC)
         → 200 { ok: true, user }
```

There is **no password, no verification email, no rate limit**. The
email is the identity. The `email:` prefix on the user id keeps the
namespace disjoint from any future OIDC `sub` for the same address, so
the two methods can coexist without merging accounts.

`GET /auth/me` advertises which methods are live:

```json
{ "authenticated": false, "methods": { "oidc": true, "email": false } }
```

The frontend `LoginView` renders the Lazycat button, the email form, or
both, based on this payload.

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

## Why one binary serves the frontend too

`rust-embed` walks `../frontend/dist` at compile time and bakes the files
into the binary. The fallback handler in `embed.rs` returns the asset for
any path that the API/auth routers haven't already matched, and falls back
to `index.html` so client-side router paths like `/c/:id` work after a
refresh.

This collapses the Lazycat manifest down to **one upstream service**, so
there's nothing to coordinate between Next.js + Go like the multica wrapper
has to.
