use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, SystemTime};

// --- Messages ---

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SystemMessage {
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserMessage {
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AssistantMessage {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolMessage {
    pub content: String,
    pub tool_call_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    System(SystemMessage),
    User(UserMessage),
    Assistant(AssistantMessage),
    Tool(ToolMessage),
}

// --- Tool Calls ---

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

// --- Timing ---

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TimedMessage {
    pub message: Message,
    pub timestamp: SystemTime,
    pub elapsed: Duration,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolResult {
    pub tool_message: ToolMessage,
    pub timestamp: SystemTime,
    pub elapsed: Duration,
}

// --- Tool Definition ---

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: Parameters,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Parameters(Value);

impl Parameters {
    pub fn new(schema: Value) -> Self {
        Self(schema)
    }
}

// --- LLM Response ---

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    #[serde(untagged)]
    Custom(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LLMResponse {
    pub message: AssistantMessage,
    pub finish_reason: FinishReason,
}

// --- Streaming ---

/// A streaming LLM response. The receiver yields `StreamEvent` items until
/// the stream is exhausted (receiver is dropped by the provider).
pub struct LLMStream {
    pub receiver: tokio::sync::mpsc::Receiver<StreamEvent>,
}

/// Events emitted during a streaming LLM response.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Text content delta.
    Content(String),
    /// Streaming has finished with the given reason.
    Finish(FinishReason),
}

// --- Convenience constructors ---

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Message::System(SystemMessage {
            content: content.into(),
        })
    }

    pub fn user(content: impl Into<String>) -> Self {
        Message::User(UserMessage {
            content: content.into(),
        })
    }
}

impl TimedMessage {
    pub fn new(message: Message) -> Self {
        Self {
            message,
            timestamp: SystemTime::now(),
            elapsed: Duration::ZERO,
        }
    }
}

impl ToolDefinition {
    pub fn new(name: &str, description: &str, parameters: Parameters) -> Self {
        Self {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: name.to_string(),
                description: description.to_string(),
                parameters,
            },
        }
    }
}
