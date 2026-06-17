use std::result;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("LLM API error ({status}): {body}")]
    ApiError { status: u16, body: String },

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Expected assistant or tool_calls message, got: {0}")]
    UnexpectedMessage(String),

    #[error("Tool '{0}' not found")]
    ToolNotFound(String),

    #[error("Hook error: {0}")]
    HookError(String),

    #[error("{0}")]
    Custom(String),
}

pub type Result<T> = result::Result<T, Error>;
