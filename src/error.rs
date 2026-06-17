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

// Manual Clone impl: reqwest::Error and serde_json::Error do not implement Clone,
// so we store their Display representations for cloning.
impl Clone for Error {
    fn clone(&self) -> Self {
        match self {
            Error::ApiError { status, body } => Error::ApiError {
                status: *status,
                body: body.clone(),
            },
            Error::Http(e) => Error::Custom(format!("Http: {}", e)),
            Error::Json(e) => Error::Custom(format!("Json: {}", e)),
            Error::UnexpectedMessage(s) => Error::UnexpectedMessage(s.clone()),
            Error::ToolNotFound(s) => Error::ToolNotFound(s.clone()),
            Error::HookError(s) => Error::HookError(s.clone()),
            Error::Custom(s) => Error::Custom(s.clone()),
        }
    }
}

pub type Result<T> = result::Result<T, Error>;
