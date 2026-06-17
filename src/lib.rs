//! # Motif
//!
//! A minimal, extensible Rust agent core. Compose an [`Agent`] with
//! a provider, tools, hooks, and history — then call [`Agent::chat`].
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use motif::*;
//!
//! #[tokio::main]
//! async fn main() -> motif::Result<()> {
//!     let provider = OpenAIProvider::new(
//!         "https://api.deepseek.com/v1",
//!         "sk-...",
//!         "deepseek-chat",
//!     );
//!
//!     let echo = ToolDef::new("echo", "Echo back input")
//!         .build(|args: String| async move { args });
//!
//!     let mut agent = Agent::new(provider)
//!         .model("deepseek-chat")
//!         .tool(echo);
//!
//!     let response = agent.chat("Hello!").await?;
//!     println!("{}", response);
//!     Ok(())
//! }
//! ```

mod agent;
mod error;
mod history;
mod hooks;
mod prompt;
mod provider;
pub mod tool;
mod types;

pub use agent::*;
pub use error::*;
pub use history::*;
pub use hooks::*;
pub use prompt::*;
pub use provider::*;
pub use tool::*;
pub use types::*;
