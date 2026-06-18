use std::result;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("LLM API error ({status}): {body}")]
    ApiError { status: u16, body: String },

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Tool '{name}' not found. Available: {available:?}")]
    ToolNotFound {
        name: String,
        available: Vec<String>,
    },

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
            Error::ToolNotFound { name, available } => Error::ToolNotFound {
                name: name.clone(),
                available: available.clone(),
            },
            Error::Custom(s) => Error::Custom(s.clone()),
        }
    }
}

pub type Result<T> = result::Result<T, Error>;
