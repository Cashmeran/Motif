use crate::error::Error;
use crate::types::{LLMResponse, TimedMessage, ToolCall, ToolResult};
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

    // --- Stream ---
    /// Whether this hook wants streaming output. Return false to opt out.
    /// All hooks must agree (no veto); one `false` doesn't disable streaming for others.
    fn wants_streaming(&self) -> bool { true }

    /// Called for each content delta during streaming. Default no-op.
    async fn on_stream_delta(&self, _delta: &str) -> crate::Result<()> { Ok(()) }

    /// Called when a streaming phase ends.
    /// `resuming: true` means tool calls follow (show spinner).
    /// `resuming: false` means this is the final response.
    async fn on_stream_end(&self, _resuming: bool) -> crate::Result<()> { Ok(()) }

    // --- Reasoning (DeepSeek thinking mode) ---
    /// Called for reasoning content deltas during streaming (thinking mode only).
    async fn on_reasoning_delta(&self, _delta: &str) -> crate::Result<()> { Ok(()) }

    // --- Message-level ---
    /// Called before a message is appended to history. Return `Ok(false)` to discard.
    async fn on_message(&self, _msg: &TimedMessage) -> crate::Result<bool> { Ok(true) }

    // --- Stop-level ---
    /// Called after stop condition is evaluated. Return `Ok(false)` to override
    /// exit and continue the loop (Ralph Loop gate pattern).
    async fn on_stop_check(
        &self,
        _ctx: &mut HookContext,
        should_stop: bool,
    ) -> crate::Result<bool> { Ok(should_stop) }

    // --- Error ---
    async fn on_error(&self, _ctx: &mut HookContext, _error: &Error) -> crate::Result<()> {
        Ok(())
    }

    // --- Finally ---
    /// Guaranteed to be called when `run()` exits, regardless of success,
    /// error, or max_iterations. Use for resource cleanup (flush buffers,
    /// close connections, save state).
    async fn on_finally(&self, _ctx: &mut RunContext) -> crate::Result<()> { Ok(()) }

    // --- Content post-processing ---
    fn finalize_content(&self, content: &str) -> String {
        content.to_string()
    }
}

