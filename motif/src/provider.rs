use crate::error::Error;
use crate::types::{FinishReason, LLMResponse, LLMStream, Message, StreamEvent, ToolDefinition};
use async_trait::async_trait;
use serde::Deserialize;

/// LLM Provider abstraction with streaming support.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn call(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> crate::Result<LLMResponse>;

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

// --- API format ---

/// Which API protocol the provider uses. DeepSeek supports both.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApiFormat {
    /// OpenAI `/v1/chat/completions` (default)
    OpenAI,
    /// Anthropic `/messages` — DeepSeek at `/anthropic/messages`
    Anthropic,
}

// --- API response types ---

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}

// Anthropic response types
#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    stop_reason: Option<String>,
    usage: Option<AnthropicUsage>,
}
#[derive(Deserialize)]
struct AnthropicContent {
    text: Option<String>,
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    input: Option<serde_json::Value>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    id: Option<String>,
}
#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
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
    format: ApiFormat,
    extra_body: serde_json::Map<String, serde_json::Value>,
    max_retries: usize,
    retry_delay_ms: u64,
    thinking_effort: Option<String>,
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
            format: ApiFormat::OpenAI,
            extra_body: Default::default(),
            max_retries: 2,
            retry_delay_ms: 1000,
            thinking_effort: None,
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

    /// Enable DeepSeek thinking mode (reasoning_effort: "high" or "max").
    pub fn with_thinking(mut self, effort: &str) -> Self {
        self.thinking_effort = Some(effort.to_string());
        self
    }

    /// Use Anthropic Messages API format (DeepSeek `/anthropic/messages` endpoint).
    pub fn with_anthropic(mut self) -> Self {
        self.format = ApiFormat::Anthropic;
        self
    }

    /// Send a POST with retry. On success returns the reqwest Response for parsing.
    async fn post_with_retry(&self, body: &serde_json::Value) -> crate::Result<reqwest::Response> {
        let url = if self.format == ApiFormat::Anthropic {
            format!("{}/messages", self.base_url)
        } else {
            format!("{}/chat/completions", self.base_url)
        };
        let mut last_err = None;
        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(
                    self.retry_delay_ms * attempt as u64,
                ))
                .await;
            }
            let mut req = self
                .client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(body);
            if self.format == ApiFormat::Anthropic {
                req = req.header("x-api-key", &self.api_key);
            } else {
                req = req.header("Authorization", format!("Bearer {}", self.api_key));
            }
            let response = match req.send().await {
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

    fn parse_response(json: serde_json::Value) -> crate::Result<LLMResponse> {
        let resp: ChatResponse = serde_json::from_value(json)?;
        let choice = resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| Error::ApiError {
                status: 200,
                body: "No choices in response".into(),
            })?;
        Ok(LLMResponse {
            message: crate::types::AssistantMessage {
                content: choice.message.content.unwrap_or_default(),
                tool_calls: choice.message.tool_calls.map(|arr| {
                    arr.into_iter()
                        .map(|tc| crate::types::ToolCall {
                            id: tc.id,
                            call_type: tc.call_type,
                            function: crate::types::FunctionCall {
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
            usage: resp.usage.map(|u| crate::types::TokenUsage {
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
        if self.format == ApiFormat::Anthropic {
            return self.build_anthropic_body(messages, tools, stream);
        }

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
        if let Some(ref effort) = self.thinking_effort {
            body["thinking"] = serde_json::json!({"type": "enabled"});
            body["reasoning_effort"] = serde_json::Value::String(effort.clone());
        }
        for (k, v) in &self.extra_body {
            body[k] = v.clone();
        }
        body
    }

    fn build_anthropic_body(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        stream: bool,
    ) -> serde_json::Value {
        // System prompt: top-level `system` field (not in messages array)
        let system = messages
            .iter()
            .filter_map(|m| {
                if let Message::System(ref s) = m {
                    Some(s.content.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        // Conversation messages: Anthropic format uses content blocks
        let anthropic_msgs: Vec<serde_json::Value> = messages.iter()
            .filter(|m| !matches!(m, Message::System(_)))
            .map(|m| match m {
                Message::User(ref u) => serde_json::json!({
                    "role": "user",
                    "content": [{"type": "text", "text": u.content}]
                }),
                Message::Assistant(ref a) => {
                    let mut blocks: Vec<serde_json::Value> = Vec::new();
                    if !a.content.is_empty() {
                        blocks.push(serde_json::json!({"type": "text", "text": a.content}));
                    }
                    if let Some(ref tool_calls) = a.tool_calls {
                        for tc in tool_calls {
                            let input: serde_json::Value = serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                            blocks.push(serde_json::json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.function.name,
                                "input": input,
                            }));
                        }
                    }
                    serde_json::json!({"role": "assistant", "content": blocks})
                }
                Message::Tool(ref t) => serde_json::json!({
                    "role": "user",
                    "content": [{"type": "tool_result", "tool_use_id": t.tool_call_id, "content": t.content}]
                }),
                Message::System(_) => serde_json::json!({"role": "user", "content": ""}), // unreachable: filtered above
            }).collect();

        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": anthropic_msgs,
        });
        if stream {
            body["stream"] = serde_json::Value::Bool(true);
        }
        if !system.is_empty() {
            body["system"] = serde_json::Value::String(system);
        }
        if !tools.is_empty() {
            body["tools"] = serde_json::json!(tools.iter().map(|t| serde_json::json!({
                "name": t.function.name,
                "description": t.function.description,
                "input_schema": serde_json::to_value(&t.function.parameters).unwrap_or_default(),
            })).collect::<Vec<_>>());
        }
        // DeepSeek thinking via Anthropic format
        if let Some(ref effort) = self.thinking_effort {
            body["thinking"] = serde_json::json!({"type": "enabled"});
            body["output_config"] = serde_json::json!({"effort": effort});
        }
        for (k, v) in &self.extra_body {
            body[k] = v.clone();
        }
        body
    }

    fn parse_anthropic_response(json: serde_json::Value) -> crate::Result<LLMResponse> {
        let resp: AnthropicResponse = serde_json::from_value(json)?;
        let mut content = String::new();
        let mut tool_calls = Vec::new();
        for block in &resp.content {
            match block.content_type.as_str() {
                "text" => {
                    if let Some(ref t) = block.text {
                        content.push_str(t);
                    }
                }
                "tool_use" => {
                    tool_calls.push(crate::types::ToolCall {
                        id: block.id.clone().unwrap_or_default(),
                        call_type: "function".to_string(),
                        function: crate::types::FunctionCall {
                            name: block.name.clone().unwrap_or_default(),
                            arguments: block
                                .input
                                .as_ref()
                                .map(|v| v.to_string())
                                .unwrap_or_default(),
                        },
                    });
                }
                _ => {}
            }
        }
        Ok(LLMResponse {
            message: crate::types::AssistantMessage {
                content,
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls)
                },
            },
            finish_reason: match resp.stop_reason.as_deref() {
                Some("end_turn") => FinishReason::Stop,
                Some("tool_use") => FinishReason::ToolCalls,
                Some("max_tokens") => FinishReason::Length,
                Some(o) => FinishReason::Custom(o.to_string()),
                None => FinishReason::Stop,
            },
            usage: resp.usage.map(|u| crate::types::TokenUsage {
                prompt_tokens: u.input_tokens.unwrap_or(0),
                completion_tokens: u.output_tokens.unwrap_or(0),
                total_tokens: u.input_tokens.unwrap_or(0) + u.output_tokens.unwrap_or(0),
            }),
        })
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
        if self.format == ApiFormat::Anthropic {
            Self::parse_anthropic_response(json)
        } else {
            Self::parse_response(json)
        }
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
                            .send(StreamEvent::Finish(crate::types::FinishReason::Custom(
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
                            .send(StreamEvent::Finish(crate::types::FinishReason::Stop))
                            .await;
                        return;
                    }
                    if let Some(data) = line.strip_prefix("data: ") {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(r) =
                                json["choices"][0]["delta"]["reasoning_content"].as_str()
                            {
                                if !r.is_empty()
                                    && tx.send(StreamEvent::Thinking(r.to_string())).await.is_err()
                                {
                                    return;
                                }
                            }
                            if let Some(c) = json["choices"][0]["delta"]["content"].as_str() {
                                if !c.is_empty()
                                    && tx.send(StreamEvent::Content(c.to_string())).await.is_err()
                                {
                                    return;
                                }
                            }
                            if let Some(r) = json["choices"][0]["finish_reason"].as_str() {
                                let fr = match r {
                                    "stop" => crate::types::FinishReason::Stop,
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
                .send(StreamEvent::Finish(crate::types::FinishReason::Stop))
                .await;
        });
        Ok(LLMStream { receiver: rx })
    }
}
