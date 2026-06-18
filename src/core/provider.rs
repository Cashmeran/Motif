use crate::core::error::Error;
use crate::core::types::{FinishReason, LLMResponse, LLMStream, Message, StreamEvent, ToolDefinition};
use async_trait::async_trait;
use serde::Deserialize;

/// LLM Provider abstraction with streaming support.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Non-streaming call — used for internal agent decision-making.
    async fn call(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> crate::Result<LLMResponse>;

    /// Streaming call — for UI rendering. Default falls back to `call()`.
    async fn call_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> crate::Result<LLMStream> {
        let response = self.call(messages, tools).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(2);
        let content = response.message.content.clone();
        let reason = response.finish_reason.clone();
        tokio::spawn(async move {
            let _ = tx.send(StreamEvent::Content(content)).await;
            let _ = tx.send(StreamEvent::Finish(reason)).await;
        });
        Ok(LLMStream { receiver: rx })
    }
}

// --- API response types ---

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}
#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
    finish_reason: Option<String>,
}
#[derive(Deserialize)]
struct ChatMessage {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChatToolCall>>,
}
#[derive(Deserialize)]
struct ChatToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: ChatFunctionCall,
}
#[derive(Deserialize)]
struct ChatFunctionCall {
    name: String,
    arguments: String,
}
#[derive(Deserialize)]
struct ChatUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

// --- OpenAI-compatible implementation ---

use reqwest::Client;
use serde_json::Value;

pub struct OpenAIProvider {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
    extra_body: serde_json::Map<String, serde_json::Value>,
    max_retries: usize,
    retry_delay_ms: u64,
}

impl OpenAIProvider {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .expect("Failed to build HTTP client"),
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
            extra_body: Default::default(),
            max_retries: 2,
            retry_delay_ms: 1000,
        }
    }

    /// Configure a custom HTTP client (e.g., with proxy, custom timeouts).
    pub fn with_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }

    /// Add extra body fields (e.g., `temperature`, `top_p`) to every request.
    pub fn with_body(mut self, key: &str, value: impl Into<serde_json::Value>) -> Self {
        self.extra_body.insert(key.to_string(), value.into());
        self
    }

    /// Set max retry attempts for transient errors (default 2).
    pub fn with_retry(mut self, max: usize) -> Self {
        self.max_retries = max;
        self
    }

    /// Send a POST with retry. On success returns the reqwest Response for parsing.
    async fn post_with_retry(&self, body: &serde_json::Value) -> crate::Result<reqwest::Response> {
        let url = format!("{}/chat/completions", self.base_url);
        let mut last_err = None;
        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(
                    self.retry_delay_ms * attempt as u64,
                ))
                .await;
            }
            let response = match self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    last_err = Some(e.into());
                    continue;
                }
            };
            let status = response.status();
            if !status.is_success() {
                let code = status.as_u16();
                let body = response.text().await.unwrap_or_default();
                if code == 429 || code >= 500 {
                    last_err = Some(Error::ApiError { status: code, body });
                    continue;
                }
                return Err(Error::ApiError { status: code, body });
            }
            return Ok(response);
        }
        Err(last_err.unwrap_or_else(|| Error::Custom("max retries exhausted".into())))
    }

    fn parse_response(json: &serde_json::Value) -> crate::Result<LLMResponse> {
        let resp: ChatResponse = serde_json::from_value(json.clone())?;
        let choice = resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| Error::ApiError {
                status: 200,
                body: "No choices in response".into(),
            })?;
        Ok(LLMResponse {
            message: crate::core::types::AssistantMessage {
                content: choice.message.content.unwrap_or_default(),
                tool_calls: choice.message.tool_calls.map(|arr| {
                    arr.into_iter()
                        .map(|tc| crate::core::types::ToolCall {
                            id: tc.id,
                            call_type: tc.call_type,
                            function: crate::core::types::FunctionCall {
                                name: tc.function.name,
                                arguments: tc.function.arguments,
                            },
                        })
                        .collect()
                }),
            },
            finish_reason: match choice.finish_reason.as_deref() {
                Some("stop") => FinishReason::Stop,
                Some("length") => FinishReason::Length,
                Some("tool_calls") => FinishReason::ToolCalls,
                Some("content_filter") => FinishReason::ContentFilter,
                Some(o) => FinishReason::Custom(o.to_string()),
                None => FinishReason::Stop,
            },
            usage: resp.usage.map(|u| crate::core::types::TokenUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
        })
    }

    fn build_request_body(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        stream: bool,
    ) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
        });
        if stream {
            body["stream"] = serde_json::Value::Bool(true);
        }
        if !tools.is_empty() {
            body["tools"] = serde_json::to_value(tools).unwrap();
        }
        for (k, v) in &self.extra_body {
            body[k] = v.clone();
        }
        body
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn call(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> crate::Result<LLMResponse> {
        let body = self.build_request_body(messages, tools, false);
        let response = self.post_with_retry(&body).await?;
        let json: Value = response.json().await?;
        Self::parse_response(&json)
    }

    async fn call_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> crate::Result<LLMStream> {
        let body = self.build_request_body(messages, tools, true);
        let response = self.post_with_retry(&body).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(64);

        tokio::spawn(async move {
            use futures::StreamExt;
            let mut stream = response.bytes_stream();
            let mut buf: Vec<u8> = Vec::new();

            loop {
                match stream.next().await {
                    Some(Ok(bytes)) => buf.extend_from_slice(&bytes),
                    Some(Err(e)) => {
                        tracing::warn!("SSE stream error: {}", e);
                        let _ = tx
                            .send(StreamEvent::Finish(crate::core::types::FinishReason::Custom(
                                "stream_error".into(),
                            )))
                            .await;
                        return;
                    }
                    None => break,
                }
                while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                    let line = std::str::from_utf8(&buf[..pos])
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    buf = buf[pos + 1..].to_vec();
                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }
                    if line == "data: [DONE]" {
                        let _ = tx
                            .send(StreamEvent::Finish(crate::core::types::FinishReason::Stop))
                            .await;
                        return;
                    }
                    if let Some(data) = line.strip_prefix("data: ") {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(c) = json["choices"][0]["delta"]["content"].as_str() {
                                if !c.is_empty()
                                    && tx.send(StreamEvent::Content(c.to_string())).await.is_err()
                                {
                                    return;
                                }
                            }
                            if let Some(r) = json["choices"][0]["finish_reason"].as_str() {
                                let fr = match r {
                                    "stop" => crate::core::types::FinishReason::Stop,
                                    "length" => FinishReason::Length,
                                    "tool_calls" => FinishReason::ToolCalls,
                                    _ => FinishReason::Stop,
                                };
                                let _ = tx.send(StreamEvent::Finish(fr)).await;
                                return;
                            }
                        }
                    }
                }
            }
            let _ = tx
                .send(StreamEvent::Finish(crate::core::types::FinishReason::Stop))
                .await;
        });
        Ok(LLMStream { receiver: rx })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_provider_construction() {
        let p = OpenAIProvider::new("https://api.openai.com/v1", "sk-test", "gpt-4");
        assert_eq!(p.base_url, "https://api.openai.com/v1");
    }
}
