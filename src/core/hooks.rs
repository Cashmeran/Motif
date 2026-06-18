use crate::core::error::Error;
use crate::core::types::{LLMResponse, TimedMessage, ToolCall, ToolResult};
use async_trait::async_trait;

// --- Context types ---

/// Per-iteration state exposed to hooks.
#[derive(Debug, Clone)]
pub struct HookContext {
    pub iteration: usize,
    pub messages: Vec<TimedMessage>,
    pub response: Option<LLMResponse>,
    pub tool_calls: Vec<ToolCall>,
    pub tool_results: Vec<ToolResult>,
    pub final_content: Option<String>,
    pub stop_reason: Option<String>,
}

impl HookContext {
    pub fn new(iteration: usize, messages: Vec<TimedMessage>) -> Self {
        Self {
            iteration,
            messages,
            response: None,
            tool_calls: vec![],
            tool_results: vec![],
            final_content: None,
            stop_reason: None,
        }
    }
}

/// Run-level state exposed to hooks.
#[derive(Debug, Clone)]
pub struct RunContext {
    pub final_content: Option<String>,
    pub stop_reason: Option<String>,
    pub error: Option<Error>,
}

impl Default for RunContext {
    fn default() -> Self {
        Self::new()
    }
}

impl RunContext {
    pub fn new() -> Self {
        Self {
            final_content: None,
            stop_reason: None,
            error: None,
        }
    }
}

// --- AgentHook trait ---

/// Lifecycle hooks for agent runs. Every method has a default no-op
/// implementation. Implement only the hooks you need.
#[async_trait]
pub trait AgentHook: Send + Sync {
    // --- Run-level ---
    async fn before_run(&self, _ctx: &mut RunContext) -> crate::Result<()> {
        Ok(())
    }
    async fn after_run(&self, _ctx: &mut RunContext) -> crate::Result<()> {
        Ok(())
    }

    // --- Iteration-level ---
    async fn before_llm(&self, _ctx: &mut HookContext) -> crate::Result<()> {
        Ok(())
    }
    async fn after_llm(&self, _ctx: &mut HookContext) -> crate::Result<()> {
        Ok(())
    }

    // --- Tool-level ---
    async fn before_tools(&self, _ctx: &mut HookContext) -> crate::Result<()> {
        Ok(())
    }
    async fn after_tools(&self, _ctx: &mut HookContext) -> crate::Result<()> {
        Ok(())
    }

    // --- Error ---
    async fn on_error(&self, _ctx: &mut HookContext, _error: &Error) -> crate::Result<()> {
        Ok(())
    }

    // --- Content post-processing ---
    fn finalize_content(&self, content: &str) -> String {
        content.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct CountingHook {
        before_count: Mutex<usize>,
    }

    #[async_trait]
    impl AgentHook for CountingHook {
        async fn before_llm(&self, _ctx: &mut HookContext) -> crate::Result<()> {
            *self.before_count.lock().unwrap() += 1;
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_hook_lifecycle_called() {
        let hook = CountingHook {
            before_count: Mutex::new(0),
        };
        let mut ctx = HookContext::new(0, vec![]);
        hook.before_llm(&mut ctx).await.unwrap();
        // Hook was called successfully
    }

    #[tokio::test]
    async fn test_hook_error_isolation() {
        struct FailingHook;
        #[async_trait]
        impl AgentHook for FailingHook {
            async fn before_llm(&self, _ctx: &mut HookContext) -> crate::Result<()> {
                Err(Error::Custom("fail".into()))
            }
        }

        let hook = FailingHook;
        let mut ctx = HookContext::new(0, vec![]);
        let result = hook.before_llm(&mut ctx).await;
        assert!(result.is_err()); // Errors are returned, not silenced
    }

    #[test]
    fn test_finalize_content_chains() {
        struct AppendHook(String);
        impl AgentHook for AppendHook {
            fn finalize_content(&self, content: &str) -> String {
                format!("{}{}", content, self.0)
            }
        }

        let a = AppendHook("A".into());
        let b = AppendHook("B".into());
        let step1 = a.finalize_content("X");
        let step2 = b.finalize_content(&step1);
        assert_eq!(step2, "XAB");
    }
}
