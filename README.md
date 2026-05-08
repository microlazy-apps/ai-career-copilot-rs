# AI 求职助手 (ai-career-copilot-rs)

[![Rust](https://img.shields.io/badge/backend-Rust%201.92-orange?logo=rust)]()
[![Vue](https://img.shields.io/badge/frontend-Vue%203-42B883?logo=vuedotjs&logoColor=white)]()
[![SQLite](https://img.shields.io/badge/store-SQLite-003B57?logo=sqlite&logoColor=white)]()
[![Lazycat](https://img.shields.io/badge/deploy-Lazycat%20MicroServer-7c8cff)]()

一份多轮对话式的 AI 求职辅导应用。后端使用 **Rust + axum + SQLite + sqlx**，前端使用 **Vue 3 + Pinia + Vue Router**，
打包成单二进制 + 单镜像，原生集成**懒猫微服 OIDC** 一键登录，并把每一次对话持久化到 SQLite — **刷新页面不会丢消息，会话历史在左侧侧边栏可随时切换**。

> 设计目标是 [Programmergyt/ai-career-copilot](https://github.com/Programmergyt/ai-career-copilot) 的轻量、可独立部署版本：用一个 ~9 MB 的 Rust 二进制取代原来的 FastAPI + LangGraph + MySQL + Redis 全套依赖，专注于「**对话 + 历史 + 一键登录**」三件最影响日常体验的事。

## 功能

- 多轮对话求职辅导：JD 解析、简历内容生成、能力缺口分析、模拟面试问答
- 持久化的会话历史：每条消息写入 SQLite，刷新或重启后从左侧侧边栏选择即可继续
- 流式返回 (SSE)：AI 回答逐 token 渲染，体感与 ChatGPT 一致
- 懒猫微服 OIDC 一键登录：通过 `application.oidc_redirect_path` 自动接入懒猫账号体系
- OpenAI 兼容的 LLM 接入：默认 DeepSeek，可改成 OpenAI / Moonshot / 自托管 vLLM
- 单二进制部署：Vue 产物通过 `rust-embed` 打进二进制，运行时只需要一个 SQLite 文件目录

## 架构概览

```
┌────────────────────────────────────────────────────────┐
│  Lazycat reverse-proxy  (subdomain: ai-career-copilot) │
│   - injects x-hc-user-id for SSO / OIDC redirect path  │
└───────────────────────┬────────────────────────────────┘
                        │
                ┌───────▼─────────┐
                │ Rust binary :8080│
                │  axum + sqlx     │
                │                  │
                │ /              → embed (Vue dist)     │
                │ /auth/oidc/*   → Lazycat OIDC client  │
                │ /api/sessions  → CRUD chat threads    │
                │ /api/.../msg   → SSE chat completions │
                │                  │
                │     ┌───── SQLite (WAL) at /data ─────┐│
                │     │ users, sessions, messages,      ││
                │     │ resume_contents                 ││
                │     └─────────────────────────────────┘│
                └──────────────────────────────────────┬─┘
                                                       │
                                       OpenAI-compat   ▼
                                       /v1/chat/completions
                                       (DeepSeek / OpenAI / ...)
```

详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

## 本地开发

需要：
- Rust 1.90+
- Node 20+ 与 pnpm 9+

```bash
# 1. 启前端 (vite dev server, 5173 端口，把 /api、/auth 代理到 :8080)
cd frontend
pnpm install
pnpm dev

# 2. 启后端 (默认监听 0.0.0.0:8080)
cd ..
cp .env.example .env       # 填入 LLM_API_KEY 等
cargo run -p ai-career-copilot
```

打开 http://localhost:5173 ，点 **使用懒猫账号登录**。本地开发时如果没有真实的 OIDC，
后端 `/auth/oidc/login` 会返回 503，可以临时把 `x-hc-user-id` 头加在浏览器扩展里用任意 user id 调试。

## 一键部署到懒猫微服

仓库自带 `lazycat/lzc-manifest.template.yml` 与 `lazycat/lzc-deploy-params.yml`。
推荐用 [microlazy-apps/lazycat-ci](https://github.com/microlazy-apps/lazycat-ci) 工作流：

```yaml
# .github/workflows/lpk.yml
jobs:
  publish:
    uses: microlazy-apps/lazycat-ci/.github/workflows/lpk-build.yml@main
    with:
      package_id: cloud.lazycat.app.microlazy-apps.ai-career-copilot
      manifest_template: lazycat/lzc-manifest.template.yml
      deploy_params: lazycat/lzc-deploy-params.yml
      dockerfile: Dockerfile
    secrets: inherit
```

安装时填入 `LLM_API_KEY` 即可，OIDC、子域名、HTTPS 由懒猫微服自动接管。

## 数据存放

| 路径 (容器内)     | 说明                                  |
| ----------------- | ------------------------------------- |
| `/data/app.db`    | SQLite 主库 (WAL 开启)                |
| `/data/app.db-wal` / `app.db-shm` | WAL & shared memory 文件 |

懒猫部署时通过 `binds: /lzcapp/var/data:/data` 挂出宿主目录，重启 / 升级都不会丢数据。

## 鸣谢

需求与多 Agent 流程参考自 [Programmergyt/ai-career-copilot](https://github.com/Programmergyt/ai-career-copilot)（FastAPI + LangGraph + Vue3，原作者 [Programmergyt](https://github.com/Programmergyt)），本仓库为 Rust 单体重写版。
