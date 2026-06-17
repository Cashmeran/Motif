use async_trait::async_trait;
use crate::types::{LLMResponse, Message, ToolDefinition};
use crate::error::Error;

/// LLM Provider abstraction. Implementations handle API-specific details
/// (auth headers, base URLs, response parsing).
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Send messages and tool definitions to the LLM, returning its response.
    async fn call(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> crate::Result<LLMResponse>;
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
            client: Client::new(),
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
        }
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
        let choice = &json["choices"][0];
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
                                arguments: tc["function"]["arguments"].to_string(),
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
