//! OpenAI-compatible streaming chat client.
//!
//! Works with DeepSeek, OpenAI, Moonshot, vLLM gateways — anything that
//! implements `/v1/chat/completions` with `stream: true` and SSE
//! `data: {...}` lines.
//!
//! Credentials are not held by the client — they are read from SQLite at
//! call time and passed in via [`LlmCallConfig`] so the user can change the
//! provider from the in-app Settings page without restarting.

use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug)]
pub enum StreamChunk {
    Delta(String),
    Done,
}

#[derive(Debug, Clone)]
pub struct LlmCallConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Default)]
pub struct OpenAiClient {
    http: reqwest::Client,
}

impl OpenAiClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent("ai-career-copilot/0.1")
                .build()
                .expect("reqwest client"),
        }
    }

    pub async fn chat_stream(
        &self,
        cfg: LlmCallConfig,
        messages: Vec<ChatMessage>,
    ) -> AppResult<impl Stream<Item = AppResult<StreamChunk>> + Send + 'static> {
        if cfg.api_key.trim().is_empty() {
            return Err(AppError::Llm(
                "尚未配置 LLM API Key，请在右上角「设置」里填入。".into(),
            ));
        }

        let body = serde_json::json!({
            "model": cfg.model,
            "messages": messages,
            "stream": true,
            "temperature": 0.6,
        });

        let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));

        let resp = self
            .http
            .post(&url)
            .bearer_auth(&cfg.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Llm(format!("connect upstream: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Llm(format!("upstream {status}: {text}")));
        }

        let byte_stream = resp.bytes_stream();
        Ok(parse_sse_stream(byte_stream))
    }

    /// Non-streaming probe used by the Settings page "Test connection" button.
    /// Sends a 1-token request and returns the raw assistant content.
    pub async fn chat_probe(&self, cfg: &LlmCallConfig) -> AppResult<String> {
        if cfg.api_key.trim().is_empty() {
            return Err(AppError::Llm("API Key 不能为空".into()));
        }

        let body = serde_json::json!({
            "model": cfg.model,
            "messages": [{"role": "user", "content": "ping"}],
            "stream": false,
            "max_tokens": 16,
        });

        let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));

        let resp = self
            .http
            .post(&url)
            .bearer_auth(&cfg.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Llm(format!("连接失败：{e}")))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(AppError::Llm(format!("HTTP {status}: {text}")));
        }

        let v: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| AppError::Llm(format!("响应非 JSON：{e}: {text}")))?;
        let content = v
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        Ok(content)
    }
}

fn parse_sse_stream<S>(byte_stream: S) -> impl Stream<Item = AppResult<StreamChunk>>
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>>,
{
    async_stream::try_stream! {
        let mut buf = String::new();
        let mut byte_stream = Box::pin(byte_stream);

        while let Some(chunk) = byte_stream.next().await {
            let bytes = chunk.map_err(|e| AppError::Llm(format!("stream read: {e}")))?;
            buf.push_str(std::str::from_utf8(&bytes).unwrap_or(""));

            while let Some((frame_end, rest_start)) = split_event(&buf) {
                let frame = buf[..frame_end].to_string();
                let rest = buf[rest_start..].to_string();
                buf = rest;

                for line in frame.lines() {
                    let line = line.trim();
                    if !line.starts_with("data:") {
                        continue;
                    }
                    let payload = line.trim_start_matches("data:").trim();
                    if payload == "[DONE]" {
                        yield StreamChunk::Done;
                        return;
                    }
                    let v: serde_json::Value = match serde_json::from_str(payload) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    if let Some(delta) = v
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        if !delta.is_empty() {
                            yield StreamChunk::Delta(delta.to_string());
                        }
                    }
                }
            }
        }

        yield StreamChunk::Done;
    }
}

/// Locate the next SSE event boundary. Returns `(frame_end, rest_start)` —
/// the first index is the end of the event body, the second is where the
/// remaining buffer continues.
fn split_event(buf: &str) -> Option<(usize, usize)> {
    if let Some(idx) = buf.find("\r\n\r\n") {
        return Some((idx, idx + 4));
    }
    if let Some(idx) = buf.find("\n\n") {
        return Some((idx, idx + 2));
    }
    None
}
