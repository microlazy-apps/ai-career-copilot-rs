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
main.rs            wires AppState, routes, embedded Vue dist
config.rs          env → Config struct, including Lazycat OIDC vars
db.rs              sqlx pool + sqlx::migrate!() runner

auth/
  oidc.rs          minimal OIDC client over reqwest (no discovery doc)
  session.rs       AuthUser extractor (cookie JWT or x-hc-user-id header)

api/
  auth.rs          /auth/oidc/login, /auth/oidc/callback, /auth/me, /auth/logout
  sessions.rs      /api/sessions CRUD
  messages.rs      /api/sessions/{id}/messages — SSE streaming chat
  resume.rs        /api/sessions/{id}/resume — structured resume JSON

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

## Why one binary serves the frontend too

`rust-embed` walks `../frontend/dist` at compile time and bakes the files
into the binary. The fallback handler in `embed.rs` returns the asset for
any path that the API/auth routers haven't already matched, and falls back
to `index.html` so client-side router paths like `/c/:id` work after a
refresh.

This collapses the Lazycat manifest down to **one upstream service**, so
there's nothing to coordinate between Next.js + Go like the multica wrapper
has to.
