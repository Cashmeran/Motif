use async_trait::async_trait;
use crate::types::{LLMResponse, LLMStream, Message, StreamEvent, ToolDefinition};
use crate::error::Error;

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

// --- OpenAI-compatible implementation ---

use reqwest::Client;
use serde_json::Value;

pub struct OpenAIProvider {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAIProvider {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .expect("Failed to build HTTP client"),
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    /// Configure a custom HTTP client (e.g., with proxy, custom timeouts).
    pub fn with_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn call(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> crate::Result<LLMResponse> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::to_value(tools)?;
        }

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(Error::ApiError {
                status: status.as_u16(),
                body: body_text,
            });
        }

        let json: Value = response.json().await?;
        let choice = json["choices"]
            .as_array()
            .and_then(|a| a.first())
            .ok_or_else(|| Error::ApiError {
                status: status.as_u16(),
                body: format!("No choices in response: {}", json),
            })?;
        let msg = &choice["message"];

        let content = msg["content"].as_str().unwrap_or("").to_string();

        let tool_calls = if let Some(tc_array) = msg["tool_calls"].as_array() {
            Some(
                tc_array
                    .iter()
                    .map(|tc| {
                        Ok(crate::types::ToolCall {
                            id: tc["id"].as_str().unwrap_or("").to_string(),
                            call_type: tc["type"].as_str().unwrap_or("function").to_string(),
                            function: crate::types::FunctionCall {
                                name: tc["function"]["name"].as_str().unwrap_or("").to_string(),
                                arguments: tc["function"]["arguments"]
                                    .as_str()
                                    .unwrap_or("{}")
                                    .to_string(),
                            },
                        })
                    })
                    .collect::<crate::Result<Vec<_>>>()?,
            )
        } else {
            None
        };

        let finish_reason = match choice["finish_reason"].as_str() {
            Some("stop") => crate::types::FinishReason::Stop,
            Some("length") => crate::types::FinishReason::Length,
            Some("tool_calls") => crate::types::FinishReason::ToolCalls,
            Some("content_filter") => crate::types::FinishReason::ContentFilter,
            Some(other) => crate::types::FinishReason::Custom(other.to_string()),
            None => crate::types::FinishReason::Stop,
        };

        Ok(LLMResponse {
            message: crate::types::AssistantMessage {
                content,
                tool_calls,
            },
            finish_reason,
        })
    }

    async fn call_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> crate::Result<LLMStream> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::to_value(tools)?;
        }

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(Error::ApiError {
                status: status.as_u16(),
                body: body_text,
            });
        }

        let (tx, rx) = tokio::sync::mpsc::channel(64);

        tokio::spawn(async move {
            use futures::StreamExt;
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(item) = stream.next().await {
                let chunk = match item {
                    Ok(bytes) => bytes,
                    Err(_) => break,
                };
                let chunk_str = String::from_utf8_lossy(&chunk);
                buffer.push_str(&chunk_str);

                // Process complete SSE lines
                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }
                    if line == "data: [DONE]" {
                        let _ = tx.send(StreamEvent::Finish(crate::types::FinishReason::Stop)).await;
                        return;
                    }
                    if let Some(data) = line.strip_prefix("data: ") {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                            let choice = &json["choices"][0];
                            let delta = &choice["delta"];

                            if let Some(content) = delta["content"].as_str() {
                                if !content.is_empty() {
                                    let _ = tx.send(StreamEvent::Content(content.to_string())).await;
                                }
                            }
                            if let Some(reason) = choice["finish_reason"].as_str() {
                                let fr = match reason {
                                    "stop" => crate::types::FinishReason::Stop,
                                    "length" => crate::types::FinishReason::Length,
                                    "tool_calls" => crate::types::FinishReason::ToolCalls,
                                    _ => crate::types::FinishReason::Stop,
                                };
                                let _ = tx.send(StreamEvent::Finish(fr)).await;
                                return;
                            }
                        }
                    }
                }
            }
            let _ = tx.send(StreamEvent::Finish(crate::types::FinishReason::Stop)).await;
        });

        Ok(LLMStream { receiver: rx })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_provider_construction() {
        let p = OpenAIProvider::new(
            "https://api.openai.com/v1",
            "sk-test",
            "gpt-4",
        );
        assert_eq!(p.base_url, "https://api.openai.com/v1");
    }
}
