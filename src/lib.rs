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

mod core;

pub mod tool {
    pub use crate::core::tool::*;
}

pub mod tools;

pub use core::agent::*;
pub use core::error::*;
pub use core::history::*;
pub use core::hooks::*;
pub use core::prompt::*;
pub use core::provider::*;
pub use core::tool::*;
pub use core::types::*;
