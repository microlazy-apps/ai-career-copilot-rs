# syntax=docker/dockerfile:1.7

# --- Stage 1: build the Vue frontend bundle.
FROM node:22-alpine AS frontend
WORKDIR /app/frontend

RUN corepack enable && corepack prepare pnpm@10.33.0 --activate

COPY frontend/package.json frontend/pnpm-lock.yaml* ./
RUN pnpm install --frozen-lockfile=false

COPY frontend/ ./
RUN pnpm build


# --- Stage 2: build the Rust backend with the frontend dist embedded.
FROM rust:1.92-slim-trixie AS backend
WORKDIR /app

RUN apt-get update && \
    apt-get install -y --no-install-recommends pkg-config && \
    rm -rf /var/lib/apt/lists/*

COPY Cargo.toml ./
COPY backend/Cargo.toml ./backend/
COPY backend/src ./backend/src
COPY backend/migrations ./backend/migrations

# rust-embed reads ../frontend/dist relative to the backend crate.
COPY --from=frontend /app/frontend/dist ./frontend/dist

RUN cargo build --release --bin ai-career-copilot
RUN strip target/release/ai-career-copilot


# --- Stage 3: minimal runtime image.
FROM debian:trixie-slim
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates wget && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=backend /app/target/release/ai-career-copilot /usr/local/bin/ai-career-copilot

ENV BIND_ADDR=0.0.0.0:8080 \
    DATA_DIR=/data \
    TZ=Asia/Shanghai

VOLUME ["/data"]

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD wget -q --spider -T 5 http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/ai-career-copilot"]
