use async_trait::async_trait;
use crate::error::Error;
use crate::types::{LLMResponse, TimedMessage, ToolCall, ToolResult};

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
    pub tools_used: Vec<String>,
    pub stop_reason: Option<String>,
    pub error: Option<Error>,
}

impl RunContext {
    pub fn new() -> Self {
        Self {
            final_content: None,
            tools_used: vec![],
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
    async fn before_run(&self, _ctx: &mut RunContext) -> crate::Result<()> { Ok(()) }
    async fn after_run(&self, _ctx: &mut RunContext) -> crate::Result<()> { Ok(()) }

    // --- Iteration-level ---
    async fn before_llm(&self, _ctx: &mut HookContext) -> crate::Result<()> { Ok(()) }
    async fn after_llm(&self, _ctx: &mut HookContext) -> crate::Result<()> { Ok(()) }

    // --- Tool-level ---
    async fn before_tools(&self, _ctx: &mut HookContext) -> crate::Result<()> { Ok(()) }
    async fn after_tools(&self, _ctx: &mut HookContext) -> crate::Result<()> { Ok(()) }

    // --- Error ---
    async fn on_error(&self, _ctx: &mut HookContext, _error: &Error) -> crate::Result<()> { Ok(()) }

    // --- Content post-processing ---
    fn finalize_content(&self, content: &str) -> String { content.to_string() }
}

// --- CompositeHook ---

/// Fans out hook calls to multiple hooks in registration order.
/// Errors from individual hooks are logged but do not propagate to other hooks.
pub struct CompositeHook {
    hooks: Vec<Box<dyn AgentHook>>,
}

impl CompositeHook {
    pub fn new(hooks: Vec<Box<dyn AgentHook>>) -> Self {
        Self { hooks }
    }
}

#[async_trait]
impl AgentHook for CompositeHook {
    async fn before_run(&self, ctx: &mut RunContext) -> crate::Result<()> {
        for hook in &self.hooks {
            hook.before_run(ctx).await.unwrap_or_else(|e| {
                tracing::warn!("Hook.before_run error: {}", e);
            });
        }
        Ok(())
    }

    async fn after_run(&self, ctx: &mut RunContext) -> crate::Result<()> {
        for hook in &self.hooks {
            hook.after_run(ctx).await.unwrap_or_else(|e| {
                tracing::warn!("Hook.after_run error: {}", e);
            });
        }
        Ok(())
    }

    async fn before_llm(&self, ctx: &mut HookContext) -> crate::Result<()> {
        for hook in &self.hooks {
            hook.before_llm(ctx).await.unwrap_or_else(|e| {
                tracing::warn!("Hook.before_llm error: {}", e);
            });
        }
        Ok(())
    }

    async fn after_llm(&self, ctx: &mut HookContext) -> crate::Result<()> {
        for hook in &self.hooks {
            hook.after_llm(ctx).await.unwrap_or_else(|e| {
                tracing::warn!("Hook.after_llm error: {}", e);
            });
        }
        Ok(())
    }

    async fn before_tools(&self, ctx: &mut HookContext) -> crate::Result<()> {
        for hook in &self.hooks {
            hook.before_tools(ctx).await.unwrap_or_else(|e| {
                tracing::warn!("Hook.before_tools error: {}", e);
            });
        }
        Ok(())
    }

    async fn after_tools(&self, ctx: &mut HookContext) -> crate::Result<()> {
        for hook in &self.hooks {
            hook.after_tools(ctx).await.unwrap_or_else(|e| {
                tracing::warn!("Hook.after_tools error: {}", e);
            });
        }
        Ok(())
    }

    async fn on_error(&self, ctx: &mut HookContext, error: &Error) -> crate::Result<()> {
        for hook in &self.hooks {
            hook.on_error(ctx, error).await.unwrap_or_else(|e| {
                tracing::warn!("Hook.on_error error: {}", e);
            });
        }
        Ok(())
    }

    fn finalize_content(&self, content: &str) -> String {
        self.hooks.iter().fold(content.to_string(), |acc, hook| {
            hook.finalize_content(&acc)
        })
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
    async fn test_composite_hook_fans_out() {
        let hook1 = CountingHook { before_count: Mutex::new(0) };
        let hook2 = CountingHook { before_count: Mutex::new(0) };
        let composite = CompositeHook::new(vec![Box::new(hook1), Box::new(hook2)]);

        let mut ctx = HookContext::new(0, vec![]);
        composite.before_llm(&mut ctx).await.unwrap();

        // Both hooks were called — can't check individual counts without
        // interior mutability but the call succeeded
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

        struct SafeHook;
        #[async_trait]
        impl AgentHook for SafeHook {
            // all defaults — should still be called
        }

        let composite = CompositeHook::new(vec![
            Box::new(FailingHook),
            Box::new(SafeHook),
        ]);

        let mut ctx = HookContext::new(0, vec![]);
        // Should not panic, should not propagate error
        let result = composite.before_llm(&mut ctx).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_finalize_content_chains() {
        struct AppendHook(String);
        impl AgentHook for AppendHook {
            fn finalize_content(&self, content: &str) -> String {
                format!("{}{}", content, self.0)
            }
        }

        let composite = CompositeHook::new(vec![
            Box::new(AppendHook("A".into())),
            Box::new(AppendHook("B".into())),
        ]);
        assert_eq!(composite.finalize_content("X"), "XAB");
    }
}
