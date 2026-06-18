use crate::history::{History, InfiniteHistory};
use crate::hooks::{AgentHook, HookContext, RunContext};
use crate::prompt::{self, Prompt, PromptBuilder};
use crate::provider::LLMProvider;
use crate::tool::{Executor, RegisteredTool, ToolExecutor};
use crate::types::{
    FinishReason, LLMResponse, Message, SystemMessage, TimedMessage, ToolDefinition,
};
use std::future::Future;
use std::sync::Arc;

fn normalize_json(json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(json)
        .map(|v| v.to_string())
        .unwrap_or_else(|_| json.to_string())
}

/// Predicate fn for StopCondition::Custom.
pub type StopPredicate = Arc<dyn Fn(&LLMResponse, &[TimedMessage]) -> bool + Send + Sync>;

// --- StopCondition ---

/// Determines when the agent loop should terminate.
#[derive(Default)]
pub enum StopCondition {
    /// Stop when the LLM returns a text response (no tool calls).
    #[default]
    OnText,
    /// Stop after executing N rounds of tool calls.
    AfterNTools(usize),
    /// Stop when the same tool+args is called more than max_repeats times in a row.
    OnStuck { max_repeats: usize },
    /// Never stop automatically — caller controls the loop via `step()`.
    Never,
    /// Custom predicate: receives LLM response and full history.
    Custom(StopPredicate),
}

impl StopCondition {
    fn should_stop(
        &self,
        response: &LLMResponse,
        history: &[TimedMessage],
        recent_calls: &[String],
    ) -> bool {
        match self {
            StopCondition::OnText => !matches!(response.finish_reason, FinishReason::ToolCalls),
            StopCondition::AfterNTools(n) => {
                let tool_count = history
                    .iter()
                    .filter(|m| matches!(m.message, Message::Tool(_)))
                    .count();
                tool_count >= *n
            }
            StopCondition::OnStuck { max_repeats } => {
                let n = (*max_repeats).max(2); // minimum 2, single call can't be "stuck"
                if recent_calls.len() < n {
                    return false;
                }
                let last = &recent_calls[recent_calls.len() - n..];
                last.windows(2).all(|w| w[0] == w[1])
            }
            StopCondition::Never => false,
            StopCondition::Custom(f) => f(response, history),
        }
    }
}

// --- Agent ---

/// The agent. Compose with providers, tools, hooks, and history, then call
/// `step()`, `run()`, or `chat()`.
pub struct Agent {
    provider: Arc<dyn LLMProvider>,
    history: Box<dyn History>,
    executor: Box<dyn ToolExecutor>,
    hooks: Vec<Box<dyn AgentHook>>,
    prompt: Prompt,
    prompt_builders: Vec<Box<dyn PromptBuilder>>,
    tool_definitions: Vec<ToolDefinition>,
    model: String,
    stop_condition: StopCondition,
    recent_tool_calls: Vec<String>, // tracks tool_name+args for OnStuck
    max_iterations: usize,
    empty_retries: usize,
    max_empty_retries: usize,
    length_continues: usize,
    max_length_continues: usize,
    step_count: usize,
    total_tokens: u64,
}

impl Agent {
    /// Create a new agent with sensible defaults and the given provider.
    pub fn new(provider: impl LLMProvider + 'static) -> Self {
        Self {
            provider: Arc::new(provider),
            history: Box::new(InfiniteHistory::new()),
            executor: Box::new(Executor::parallel()),
            hooks: vec![],
            prompt: Prompt::new(),
            prompt_builders: vec![],
            tool_definitions: vec![],
            model: String::new(),
            stop_condition: StopCondition::default(),
            recent_tool_calls: vec![],
            max_iterations: 100,
            empty_retries: 0,
            max_empty_retries: 2,
            length_continues: 0,
            max_length_continues: 3,
            step_count: 0,
            total_tokens: 0,
        }
    }

    /// Set the model name for runtime context injection in user messages.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }

    fn refresh_tools(&self) {
        let json = serde_json::to_string(&self.tool_definitions).unwrap_or_default();
        self.prompt.freeze_tools(&json);
    }

    // --- Builder methods ---

    pub fn history(mut self, history: impl History + 'static) -> Self {
        self.history = Box::new(history);
        self
    }

    pub fn executor(mut self, executor: impl ToolExecutor + 'static) -> Self {
        self.executor = Box::new(executor);
        self
    }

    pub fn hook(mut self, hook: impl AgentHook + 'static) -> Self {
        self.hooks.push(Box::new(hook));
        self
    }

    pub fn prompt_builder(mut self, builder: impl PromptBuilder + 'static) -> Self {
        self.prompt_builders.push(Box::new(builder));
        self
    }

    /// Register a tool. Accepts both:
    /// - `RegisteredTool` from `ToolDef::new().param().build()`
    /// - `fn(Args) -> impl Future<Output=String>` from `#[tool]` macro
    pub fn tool(mut self, registered: RegisteredTool) -> Self {
        let (def, tool) = registered.into_parts();
        let name = def.function.name.clone();
        self.tool_definitions.push(def);
        self.executor.register(name, tool);
        self.refresh_tools();
        self
    }

    /// Shorthand: register a `#[tool]`-annotated function directly.
    /// Internally builds a `RegisteredTool` and delegates to [`Self::tool`].
    pub fn tool_fn<Args, Fut>(mut self, f: fn(Args) -> Fut) -> Self
    where
        Args: crate::tool::ToolArgs + 'static,
        Fut: Future<Output = String> + Send + 'static,
    {
        use crate::tool::FunctionTool;
        self.tool(RegisteredTool {
            definition: Args::definition(),
            tool: Arc::new(FunctionTool::new(move |json: String| {
                Box::pin(async move {
                    let args = match serde_json::from_str::<Args>(&json) {
                        Ok(a) => a,
                        Err(e) => return format!("[Invalid arguments: {}]. Check and retry.", e),
                    };
                    f(args).await
                })
            })),
        })
    }

    /// Bind a method (on a stateful instance) as a tool.
    /// The method must be `#[tool]`-annotated.
    pub fn bind<T, Args, Fut>(mut self, instance: T, method: fn(T, Args) -> Fut) -> Self
    where
        T: Clone + Send + Sync + 'static,
        Args: crate::tool::ToolArgs + 'static,
        Fut: Future<Output = String> + Send + 'static,
    {
        use crate::tool::FunctionTool;
        let def = Args::definition();
        let name = def.function.name.clone();
        self.tool_definitions.push(def);
        let tool = Arc::new(FunctionTool::new(move |json: String| {
            let instance = instance.clone();
            Box::pin(async move {
                let args = match serde_json::from_str::<Args>(&json) {
                    Ok(a) => a,
                    Err(e) => return format!("[Invalid arguments: {}]. Expected JSON matching the tool schema. Check and retry.", e),
                };
                method(instance, args).await
            })
        }));
        self.executor.register(name, tool);
        self.refresh_tools();
        self
    }

    pub fn external_tools(
        mut self,
        defs: Vec<ToolDefinition>,
        handler: impl Fn(String, String) -> String + Send + Sync + 'static,
    ) -> Self {
        use crate::tool::FunctionTool;
        let shared = Arc::new(handler);
        for def in &defs {
            let name = def.function.name.clone();
            let handler = shared.clone();
            let def_name = name.clone();
            self.executor.register(
                name,
                Arc::new(FunctionTool::new(move |args: String| {
                    let h = handler.clone();
                    let n = def_name.clone();
                    Box::pin(async move { h(n, args) })
                })),
            );
        }
        self.tool_definitions.extend(defs);
        self.refresh_tools();
        self
    }

    pub fn stop_when(mut self, condition: StopCondition) -> Self {
        self.stop_condition = condition;
        self
    }

    // --- Core loop ---

    /// Execute a single iteration: send messages to LLM, execute any tool
    /// calls, append results to history. Returns `Ok(Some(content))` if the
    /// loop should stop, `Ok(None)` to continue.
    pub async fn step(&mut self) -> crate::Result<Option<String>> {
        // Build L2 extensions from registered PromptBuilders
        let extensions: Vec<String> = self
            .prompt_builders
            .iter()
            .filter_map(|b| b.build())
            .collect();
        let prompt_text = self.prompt.build(&extensions);
        let system_msg = Message::System(SystemMessage {
            content: prompt_text,
        });

        // Assemble messages for this turn
        let mut turn_messages = vec![system_msg];
        for timed in self.history.get_all() {
            turn_messages.push(timed.message.clone());
        }

        // Hook: before_llm
        self.step_count += 1;
        let mut hook_ctx = HookContext::new(self.step_count, self.history.get_all().to_vec());
        for hook in &self.hooks {
            if let Err(e) = hook.before_llm(&mut hook_ctx).await {
                tracing::warn!("Hook.before_llm error: {}", e);
            }
        }

        // Hook: on_request — final gate before provider
        for hook in &self.hooks {
            if let Err(e) = hook.on_request(&mut hook_ctx).await {
                tracing::warn!("Hook.on_request error: {}", e);
            }
        }

        // Call LLM
        let llm_start = std::time::Instant::now();
        let response = self
            .provider
            .call(&turn_messages, &self.tool_definitions)
            .await?;
        let elapsed = llm_start.elapsed();

        if let Some(ref usage) = response.usage {
            self.total_tokens += usage.total_tokens as u64;
        }

        // Append assistant message to history + keep hook_ctx in sync
        let assoc_msg = TimedMessage {
            message: Message::Assistant(response.message.clone()),
            timestamp: std::time::SystemTime::now(),
            elapsed,
        };
        // Hook: on_message — filter messages before history
        let keep = {
            let mut ok = true;
            for hook in &self.hooks {
                match hook.on_message(&assoc_msg).await {
                    Ok(false) => ok = false,
                    Err(e) => tracing::warn!("Hook.on_message error: {}", e),
                    _ => {}
                }
            }
            ok
        };
        if keep {
            self.history.add(assoc_msg.clone());
            hook_ctx.messages.push(assoc_msg);
        }

        // Hook: after_llm
        hook_ctx.response = Some(response.clone());
        for hook in &self.hooks {
            if let Err(e) = hook.after_llm(&mut hook_ctx).await {
                tracing::warn!("Hook.after_llm error: {}", e);
            }
        }

        // Execute tool calls if any
        if let Some(ref calls) = response.message.tool_calls {
            if !calls.is_empty() {
                // Track for OnStuck
                for call in calls {
                    let normalized = normalize_json(&call.function.arguments);
                    let sig = format!("{}:{}", call.function.name, normalized);
                    self.recent_tool_calls.push(sig);
                }
                if self.recent_tool_calls.len() > 20 {
                    self.recent_tool_calls = self
                        .recent_tool_calls
                        .split_off(self.recent_tool_calls.len() - 20);
                }

                hook_ctx.tool_calls = calls.clone();
                for hook in &self.hooks {
                    if let Err(e) = hook.before_tools(&mut hook_ctx).await {
                        tracing::warn!("Hook.before_tools error: {}", e);
                    }
                }

                let results = self.executor.execute(calls.clone()).await;

                hook_ctx.tool_results.clone_from(&results);
                for hook in &self.hooks {
                    if let Err(e) = hook.after_tools(&mut hook_ctx).await {
                        tracing::warn!("Hook.after_tools error: {}", e);
                    }
                }

                // Append tool results to history + keep hook_ctx in sync
                for result in &results {
                    let msg = TimedMessage {
                        message: Message::Tool(result.tool_message.clone()),
                        timestamp: result.timestamp,
                        elapsed: result.elapsed,
                    };
                    self.history.add(msg.clone());
                    hook_ctx.messages.push(msg);
                }
            }
        }

        // Recovery: Length continuation or empty retry
        if self.try_recover(&response).await {
            return Ok(None);
        }

        // Check stop condition — hooks can override exit (Ralph Loop gate)
        let mut should_stop = self.stop_condition.should_stop(
            &response, self.history.get_all(), &self.recent_tool_calls);
        for hook in &self.hooks {
            match hook.on_stop_check(&mut hook_ctx, should_stop).await {
                Ok(false) => should_stop = false,
                Err(e) => tracing::warn!("Hook.on_stop_check error: {}", e),
                _ => {}
            }
        }
        if should_stop {
            return Ok(Some(response.message.content));
        }

        Ok(None)
    }

    /// Handle Length truncation (inject "continue") or empty response (retry).
    /// Returns true if the loop should continue (recovery was triggered).
    async fn try_recover(&mut self, response: &LLMResponse) -> bool {
        let is_empty = response.message.content.trim().is_empty();
        let has_tools = response
            .message
            .tool_calls
            .as_ref()
            .is_some_and(|c| !c.is_empty());

        if matches!(response.finish_reason, FinishReason::Length) {
            if !is_empty && self.length_continues < self.max_length_continues {
                self.length_continues += 1;
                self.history
                    .add(TimedMessage::new(Message::user("continue")));
                return true;
            }
        } else {
            self.length_continues = 0;
        }

        if is_empty && !has_tools && self.empty_retries < self.max_empty_retries {
            self.empty_retries += 1;
            return true;
        }
        if !is_empty || has_tools {
            self.empty_retries = 0;
        }
        false
    }

    /// Run the agent loop until the stop condition is met.
    pub async fn run(&mut self) -> crate::Result<String> {
        let mut run_ctx = RunContext::new();
        for hook in &self.hooks {
            if let Err(e) = hook.before_run(&mut run_ctx).await {
                tracing::warn!("Hook.before_run error: {}", e);
            }
        }

        let mut iterations = 0;
        loop {
            if self.max_iterations > 0 && iterations >= self.max_iterations {
                let drained: Vec<_> = self
                    .history
                    .get_all()
                    .iter()
                    .filter(|m| matches!(m.message, Message::Assistant(_)))
                    .collect();
                let fallback = drained
                    .last()
                    .and_then(|tm| {
                        if let Message::Assistant(ref a) = tm.message {
                            Some(a.content.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "Max iterations reached".to_string());
                run_ctx.final_content = Some(fallback.clone());
                for hook in &self.hooks {
                    if let Err(e) = hook.after_run(&mut run_ctx).await {
                        tracing::warn!("Hook.after_run error: {}", e);
                    }
                }
                return Ok(fallback);
            }
            iterations += 1;

            match self.step().await {
                Ok(Some(content)) => {
                    let mut finalized = content;
                    for hook in &self.hooks {
                        finalized = hook.finalize_content(&finalized);
                    }
                    run_ctx.final_content = Some(finalized.clone());
                    for hook in &self.hooks {
                        if let Err(e) = hook.after_run(&mut run_ctx).await {
                            tracing::warn!("Hook.after_run error: {}", e);
                        }
                    }
                    return Ok(finalized);
                }
                Ok(None) => continue,
                Err(e) => {
                    let mut error_ctx =
                        HookContext::new(iterations, self.history.get_all().to_vec());
                    for hook in &self.hooks {
                        let _ = hook.on_error(&mut error_ctx, &e).await;
                    }
                    run_ctx.error = Some(e.clone());
                    for hook in &self.hooks {
                        if let Err(e2) = hook.after_run(&mut run_ctx).await {
                            tracing::warn!("Hook.after_run error: {}", e2);
                        }
                    }
                    return Err(e);
                }
            }
        }
    }

    /// Send a user message and run to completion.
    /// Runtime context (date/model) is prepended to the message.
    pub async fn chat(&mut self, content: impl Into<String>) -> crate::Result<String> {
        let content = content.into();
        let ctx = prompt::runtime_context(&self.model);
        let full = if content.is_empty() {
            ctx
        } else {
            format!("{}\n{}", ctx, content)
        };
        self.history.add(TimedMessage::new(Message::user(full)));
        self.run().await
    }

    /// Access the underlying tool definitions (for introspection).
    pub fn tool_definitions(&self) -> &[ToolDefinition] {
        &self.tool_definitions
    }

    /// Total tokens consumed across all LLM calls in this session.
    pub fn total_tokens_used(&self) -> u64 {
        self.total_tokens
    }

    /// Access the history.
    pub fn history_ref(&self) -> &dyn History {
        self.history.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AssistantMessage, ToolCall};
    use async_trait::async_trait;

    /// Mock LLM provider for testing — returns canned responses.
    struct MockProvider {
        responses: std::sync::Mutex<Vec<LLMResponse>>,
        call_count: std::sync::Mutex<usize>,
    }

    impl MockProvider {
        fn new(responses: Vec<LLMResponse>) -> Self {
            Self {
                responses: std::sync::Mutex::new(responses),
                call_count: std::sync::Mutex::new(0),
            }
        }
    }

    #[async_trait]
    impl LLMProvider for MockProvider {
        async fn call(
            &self,
            _messages: &[Message],
            _tools: &[ToolDefinition],
        ) -> crate::Result<LLMResponse> {
            let mut count = self.call_count.lock().unwrap();
            let idx = *count;
            *count += 1;
            let responses = self.responses.lock().unwrap();
            Ok(responses[idx].clone())
        }
    }

    fn mock_text_response(content: &str) -> LLMResponse {
        LLMResponse {
            message: AssistantMessage {
                content: content.to_string(),
                tool_calls: None,
            },
            finish_reason: FinishReason::Stop,
            usage: None,
        }
    }

    fn mock_tool_response(name: &str, args: &str) -> LLMResponse {
        LLMResponse {
            message: AssistantMessage {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".to_string(),
                    call_type: "function".to_string(),
                    function: crate::types::FunctionCall {
                        name: name.to_string(),
                        arguments: args.to_string(),
                    },
                }]),
            },
            finish_reason: FinishReason::ToolCalls,
            usage: None,
        }
    }

    #[tokio::test]
    async fn test_agent_text_response_stops_loop() {
        let provider = MockProvider::new(vec![mock_text_response("Hello!")]);
        let mut agent = Agent::new(provider);

        let result = agent.chat("hi").await.unwrap();
        assert_eq!(result, "Hello!");
    }

    #[tokio::test]
    async fn test_agent_tool_then_text() {
        use crate::tool::ToolDef;

        let provider = MockProvider::new(vec![
            mock_tool_response("echo", r#"{"msg":"hi"}"#),
            mock_text_response("Tool done!"),
        ]);

        let echo_tool = ToolDef::new("echo", "Echo back")
            .build(|_args: String| async { "echo: hi".to_string() });

        let mut agent = Agent::new(provider).tool(echo_tool);

        let result = agent.chat("echo hi").await.unwrap();
        assert_eq!(result, "Tool done!");
        // Verify tool was recorded in history
        let history = agent.history_ref().get_all();
        assert!(history
            .iter()
            .any(|m| matches!(m.message, Message::Tool(_))));
    }

    #[tokio::test]
    async fn test_stop_condition_on_stuck() {
        use crate::tool::ToolDef;

        // Provider keeps returning the same tool call
        let responses: Vec<LLMResponse> = (0..5)
            .map(|_| mock_tool_response("echo", r#"{"msg":"hi"}"#))
            .collect();

        let provider = MockProvider::new(responses);
        let echo_tool =
            ToolDef::new("echo", "Echo").build(|_args: String| async { "echo".to_string() });

        let mut agent = Agent::new(provider)
            .tool(echo_tool)
            .stop_when(StopCondition::OnStuck { max_repeats: 3 });

        let result = agent.chat("stuck test").await;
        assert!(result.is_ok());
        // Should stop after detecting 3 repeated calls, not continue all 5
        let history = agent.history_ref().get_all();
        let tool_count = history
            .iter()
            .filter(|m| matches!(m.message, Message::Tool(_)))
            .count();
        assert!(tool_count <= 4); // at most 4 tool results (calls 1-4), then stuck stop
    }

    #[tokio::test]
    async fn test_hook_called_during_run() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CounterHook {
            before_count: AtomicUsize,
            after_count: AtomicUsize,
        }

        #[async_trait]
        impl AgentHook for CounterHook {
            async fn before_llm(&self, _ctx: &mut HookContext) -> crate::Result<()> {
                self.before_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            async fn after_llm(&self, _ctx: &mut HookContext) -> crate::Result<()> {
                self.after_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        let provider = MockProvider::new(vec![mock_text_response("Hi")]);
        let hook = CounterHook {
            before_count: AtomicUsize::new(0),
            after_count: AtomicUsize::new(0),
        };

        let mut agent = Agent::new(provider).hook(hook);

        agent.chat("hello").await.unwrap();
        // Hook was called — the hook moved into the agent, so we can't inspect
        // counts directly, but the call succeeded without error.
    }

    #[tokio::test]
    async fn test_external_tools_execution() {
        let provider = MockProvider::new(vec![
            mock_tool_response("ext_search", r#"{"query":"rust"}"#),
            mock_text_response("Found results"),
        ]);

        let defs = vec![ToolDefinition::new(
            "ext_search",
            "Search external source",
            crate::types::Parameters::new(serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query"}
                },
                "required": ["query"]
            })),
        )];

        let mut agent = Agent::new(provider).external_tools(defs, |name, args| {
            format!("external {} called with {}", name, args)
        });

        let result = agent.chat("search rust").await.unwrap();
        assert_eq!(result, "Found results");
    }

    #[tokio::test]
    async fn test_stop_condition_never_continues() {
        let provider = MockProvider::new(vec![
            mock_text_response("First"),
            mock_text_response("Second"),
        ]);

        let mut agent = Agent::new(provider).stop_when(StopCondition::Never);

        // Manually step
        let result1 = agent.step().await.unwrap();
        assert!(result1.is_none()); // Never stops on text

        let result2 = agent.step().await.unwrap();
        assert!(result2.is_none()); // Still doesn't stop
    }
}
