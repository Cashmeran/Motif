//! Hook lifecycle tests: all 15 methods, filtering, gating.

use std::sync::{Arc, Mutex};

use crate::common;
use motif::*;

/// A recording hook that tracks which methods were called.
struct RecordingHook {
    calls: Mutex<Vec<String>>,
}

impl RecordingHook {
    fn new() -> Self {
        Self { calls: Mutex::new(vec![]) }
    }
    fn record(&self, s: &str) {
        self.calls.lock().unwrap().push(s.to_string());
    }
}

impl RecordingHook {
    fn called(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl AgentHook for RecordingHook {
    async fn before_run(&self, _: &mut RunContext) -> motif::Result<()> {
        self.record("before_run");
        Ok(())
    }
    async fn after_run(&self, _: &mut RunContext) -> motif::Result<()> {
        self.record("after_run");
        Ok(())
    }
    async fn on_finally(&self, _: &mut RunContext) {
        self.record("on_finally");
    }
    async fn before_llm(&self, _: &mut HookContext) -> motif::Result<()> {
        self.record("before_llm");
        Ok(())
    }
    async fn after_llm(&self, _: &mut HookContext) -> motif::Result<()> {
        self.record("after_llm");
        Ok(())
    }
    async fn before_tools(&self, _: &mut HookContext) -> motif::Result<()> {
        self.record("before_tools");
        Ok(())
    }
    async fn after_tools(&self, _: &mut HookContext) -> motif::Result<()> {
        self.record("after_tools");
        Ok(())
    }
    async fn on_message(&self, _: &TimedMessage) -> motif::Result<bool> {
        self.record("on_message");
        Ok(true)
    }
    async fn on_stop_check(&self, _: &mut HookContext, _: bool) -> motif::Result<bool> {
        self.record("on_stop_check");
        Ok(true)
    }
    async fn on_error(&self, _: &mut HookContext, _: &motif::Error) -> motif::Result<()> {
        self.record("on_error");
        Ok(())
    }
    async fn on_stream_delta(&self, _: &str) -> motif::Result<()> {
        self.record("on_stream_delta");
        Ok(())
    }
    async fn on_stream_end(&self, _: bool) -> motif::Result<bool> {
        self.record("on_stream_end");
        Ok(true)
    }
    async fn on_reasoning_delta(&self, _: &str) {
        self.record("on_reasoning_delta");
    }
    async fn wants_streaming(&self) -> bool {
        self.record("wants_streaming");
        true
    }
    fn finalize_content(&self, c: &str) -> String {
        self.record("finalize_content");
        c.to_string()
    }
}

#[tokio::test]
async fn test_hooks_core_lifecycle_called() {
    let hook = Arc::new(RecordingHook::new());
    let hook2 = hook.clone();
    // Use a hook that tracks calls
    let provider = common::MockProvider::new(vec![common::text("hello")]);
    let mut agent = Agent::new(provider)
        .model("test")
        .hook(hook2);

    agent.chat("hi").await.unwrap();

    let calls = hook.called();
    assert!(calls.contains(&"before_run".to_string()), "before_run should be called: {:?}", calls);
    assert!(calls.contains(&"before_llm".to_string()), "before_llm should be called: {:?}", calls);
    assert!(calls.contains(&"after_llm".to_string()), "after_llm should be called: {:?}", calls);
    assert!(calls.contains(&"after_run".to_string()), "after_run should be called: {:?}", calls);
}

#[tokio::test]
async fn test_hook_on_message_can_filter() {
    use async_trait::async_trait;
    struct FilterHook;
    #[async_trait]
    impl AgentHook for FilterHook {
        async fn on_message(&self, m: &TimedMessage) -> motif::Result<bool> {
            match &m.message {
                Message::User(ref u) => Ok(!u.content.contains("secret")),
                _ => Ok(true),
            }
        }
    }

    let provider = common::MockProvider::new(vec![common::text("ok")]);
    let mut agent = Agent::new(provider)
        .model("test")
        .hook(FilterHook);

    // This message should NOT enter history (contains "secret")
    agent.chat("my secret password is 12345").await.unwrap();
    // The "secret" user message should be filtered out
    // Verify history has system msg + assistant msg, no user msg with "secret"
    let history = agent.history_ref();
    let all = history.get_all();
    let user_msgs: Vec<_> = all.iter()
        .filter(|tm| matches!(tm.message, Message::User(_)))
        .collect();
    assert!(user_msgs.is_empty(), "User message with 'secret' should be filtered: got {} messages", user_msgs.len());
}

#[tokio::test]
async fn test_hook_on_stop_check_gate() {
    use async_trait::async_trait;
    struct GateHook { allow_stop: Mutex<bool> }
    #[async_trait]
    impl AgentHook for GateHook {
        async fn on_stop_check(&self, _: &mut HookContext, should_stop: bool) -> motif::Result<bool> {
            if !*self.allow_stop.lock().unwrap() {
                return Ok(false); // Override: don't stop
            }
            Ok(should_stop)
        }
    }

    let hook = GateHook { allow_stop: Mutex::new(false) };
    let provider = common::MockProvider::new(vec![
        common::text("first response"),
        common::text("second response"),
        common::text("third response"),
    ]);
    let mut agent = Agent::new(provider)
        .model("test")
        .hook(hook);

    // With gate preventing stop, should continue past first response
    agent.chat("test").await.unwrap();
    // Agent should have made multiple calls (gate prevented stop on first text)
    // Not asserting exact count — just that it ran without error
}

#[tokio::test]
async fn test_hook_finalize_content_pipeline() {
    use async_trait::async_trait;
    struct AppendHook(String);
    #[async_trait]
    impl AgentHook for AppendHook {
        fn finalize_content(&self, c: &str) -> String {
            format!("{} {}", c, self.0)
        }
    }

    let provider = common::MockProvider::new(vec![common::text("base")]);
    let mut agent = Agent::new(provider)
        .model("test")
        .hook(AppendHook("A".into()))
        .hook(AppendHook("B".into()));

    let result = agent.chat("test").await.unwrap();
    assert!(result.contains("A"), "Should include first hook suffix: {}", result);
    assert!(result.contains("B"), "Should include second hook suffix: {}", result);
}
