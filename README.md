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
- 流式返回 (SSE)：AI 回答逐 token 渲染，体感与 ChatGPT 一致；中途断网也能落库恢复
- 懒猫微服 OIDC 一键登录：通过 `application.oidc_redirect_path` 自动接入懒猫账号体系
- **应用内设置页**：API Base URL / 模型 / API Key 全部在右上角「⚙ 设置」里配置，支持「测试连通」按钮一键验证；安装时不需要先准备凭据
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
cp .env.example .env       # SESSION_SECRET 必填；LLM_* 可留空，启动后到「⚙ 设置」里填
cargo run -p ai-career-copilot
```

打开 http://localhost:5173 。本地开发没有懒猫 OIDC 时，登录页会自动显示**邮箱登录**：输入任意邮箱即可作为身份进入（不发验证邮件、不验密码——纯做"现阶段认证意义不大"的弱认证）。也可以用浏览器扩展手动注入 `x-hc-user-id` 头任意调试。

> **LLM 凭据怎么填都行**：`.env` 里的 `LLM_BASE_URL / LLM_API_KEY / LLM_MODEL` 只在数据库还是默认值时
> 作为首次启动的"种子"写入 `app_settings` 表。一旦你在 UI 里改过设置，env 不再覆盖数据库里的值。
> 推荐做法：留空 env，登录后到右上角「⚙ 设置」配置，刷新或重启都不丢。

## 一键部署 (Docker Compose)

适合自托管在任意一台有 Docker 的机器上。镜像在本地构建（多阶段：Vue 产物 + Rust 静态二进制），SQLite 通过命名卷持久化，重启 / 升级不会丢数据。

```bash
# 1. 克隆仓库
git clone https://github.com/microlazy-apps/ai-career-copilot-rs.git
cd ai-career-copilot-rs

# 2. 准备环境变量
#    必填：SESSION_SECRET（compose 启动会校验）
#    可选：LLM_BASE_URL / LLM_API_KEY / LLM_MODEL —— 不填也能启动，
#         首次登录后到「⚙ 设置」配置即可。
cp .env.example .env
sed -i "s/^SESSION_SECRET=.*/SESSION_SECRET=$(openssl rand -hex 32)/" .env

# 3. 启动（首次会构建镜像，5–10 分钟）
docker compose up -d --build

# 4. 验证
curl -fsS http://localhost:${APP_PORT:-8080}/healthz   # => ok
```

打开 http://localhost:8080 即可使用，**到右上角「⚙ 设置」填入 LLM 凭据**（DeepSeek / OpenAI / Moonshot 等
OpenAI 兼容协议）后即可对话。无 OIDC 凭证时，本地可通过浏览器扩展注入 `x-hc-user-id` 头进行调试，详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

常用命令：

```bash
docker compose logs -f app          # 跟踪日志
docker compose restart app          # 重启
docker compose down                 # 停止并移除容器（卷保留 → 数据不丢）
docker compose down -v              # 停止并清理卷（数据会被删除）
docker volume ls | grep ai-career   # 查看持久化卷
```

数据卷映射：`ai-career-copilot_app-data` → 容器内 `/data`，含 `app.db` 主库与 WAL 文件。

## 一键部署到懒猫微服

仓库自带 `lazycat/{package,lzc-manifest}.template.yml` —— 安装时不需要任何参数（凭据从应用内「⚙ 设置」配置）。
配套两条 GitHub Actions 流水线（基于 [microlazy-apps/lazycat-ci](https://github.com/microlazy-apps/lazycat-ci) 的 reusable workflows）：

| Workflow | 触发 | 作用 |
| --- | --- | --- |
| `.github/workflows/release.yml` | `git push origin v*` | Docker → ghcr → lzc-cli copy-image → lpk artifact，附加到 GH Release，并推送到懒猫应用市场 |
| `.github/workflows/bootstrap-app.yml` | manual `workflow_dispatch` | 一次性首次提交：注册 app + 上传 lpk + 提交审核（之后由 release.yml 接管） |

仓库需要在 **Repository secrets** 里配置：

- `LAZYCAT_USERNAME` / `LAZYCAT_PASSWORD`：懒猫开发者中心账号

发布流程：

```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions 跑完后会自动产出 `.lpk`，附加到对应的 GitHub Release，并推送到懒猫应用市场。
**安装无需输入任何参数**（OIDC、子域名、HTTPS 由懒猫微服自动接管）—— 第一次登录后到右上角「⚙ 设置」填入 LLM `API Base URL` / `模型` / `API Key`，点「测试连通」验证即可。所有配置写入 `/data/app.db`，重启 / 升级不丢。

> 首次提交（`bootstrap-app.yml`）需要先把截图放到 `lazycat/screenshots/` 下，
> 文件名要与 `lazycat/appstore.yml` 的 `screenshots.pc` 对齐。

## 登录方式

| 部署形态 | 默认登录方式 | 说明 |
| --- | --- | --- |
| 懒猫微服 lpk | 懒猫 OIDC | 由 lpk manifest 的 `oidc_redirect_path` 自动接入；用户点 **使用懒猫账号登录** 即可 |
| docker compose / 本地 `cargo run` | **邮箱登录**（默认开） | 没有 OIDC 时，登录页直接显示邮箱表单，输入任意邮箱即可进入 |

**邮箱登录是"弱认证"**，对齐 issue MIC-5 里"现阶段认证意义不大"的判断：

- 不发验证邮件、不要密码、不依赖任何外部服务
- 邮箱地址（小写化后）即用户身份：`POST /auth/email/login {email}` → 在 `users` 表里以 `email:<addr>` 为主键 upsert，下发 30 天 `ac_session` JWT cookie
- 前端登录页根据 `GET /auth/me` 返回的 `methods.{oidc,email}` 自动渲染对应入口；OIDC 与邮箱可同时启用，互不影响

切换开关：

| 场景 | 推荐做法 |
| --- | --- |
| 懒猫 lpk 安装 | 不用动；OIDC 起来后邮箱登录默认关闭 |
| docker compose 自托管 | 不用动；没 OIDC 时邮箱登录默认开 |
| 既要 OIDC 又要邮箱兜底 | `EMAIL_LOGIN=1` 同时打开 |
| 强制只允许 OIDC | `EMAIL_LOGIN=0`（OIDC 没起来时会变成"无法登录"，慎用） |

## 配置

应用所有运行期可调的 LLM 设置都放在**应用内**，不需要重启容器：

| 设置项 | 入口 | 默认值 | 说明 |
| --- | --- | --- | --- |
| API Base URL | 右上角「⚙ 设置」→「服务端点」 | `https://api.deepseek.com/v1` | OpenAI 兼容协议 |
| 模型 | 同上 | `deepseek-chat` | 例如 `gpt-4o-mini`、`moonshot-v1-8k` |
| API Key | 同上 →「访问凭据」 | _(空)_ | 写入数据库后只回 `****abcd` 末四位 hint，原值不再返回 |

设置写入 `app_settings` 表（见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#应用内设置-app_settings)），
随 `/data/app.db` 一起持久化，重启、升级、`docker compose down && up` 都不丢。设置页内置了：

- **Provider 预设**：DeepSeek / OpenAI / Moonshot 一键填入
- **测试连通**：调用一次 16-token 的 chat completion，把模型回复回显
- **状态徽章 / Key hint**：随时知道当前是否已配置

#### 环境变量是"种子"，不是配置

`.env` / `docker-compose.yml` 里的 `LLM_BASE_URL` / `LLM_API_KEY` / `LLM_MODEL` 仅在数据库还是默认值时
作为首次启动的种子写入 `app_settings`。一旦在 UI 里改过设置，env 就不再覆盖（避免"我明明改了，怎么重启又变回去"）。

| 场景 | 推荐做法 |
| --- | --- |
| 懒猫 lpk 安装 | env 全部不传，登录后到「⚙ 设置」填 |
| docker compose 自托管 | env 全部不传，登录后到「⚙ 设置」填；或在 `.env` 里填一份方便首次启动 |
| 本地 `cargo run` | 同上 |

API（带 OIDC cookie 或 `x-hc-user-id` 头）：

```http
GET  /api/settings           → { llm_base_url, llm_model, llm_api_key_hint, llm_configured, updated_at }
PUT  /api/settings           # 增量更新；省略字段=不变；llm_api_key=""=清空
POST /api/settings/test      # 用当前配置打一次 16-token chat completion，回显模型回复
```

## 数据存放

| 路径 (容器内)     | 说明                                  |
| ----------------- | ------------------------------------- |
| `/data/app.db`    | SQLite 主库 (WAL 开启)                |
| `/data/app.db-wal` / `app.db-shm` | WAL & shared memory 文件 |

懒猫部署时通过 `binds: /lzcapp/var/data:/data` 挂出宿主目录，重启 / 升级都不会丢数据。

## 鸣谢

需求与多 Agent 流程参考自 [Programmergyt/ai-career-copilot](https://github.com/Programmergyt/ai-career-copilot)（FastAPI + LangGraph + Vue3，原作者 [Programmergyt](https://github.com/Programmergyt)），本仓库为 Rust 单体重写版。
