//! Hook lifecycle tests — verify hook methods are called and can affect behavior.

use std::sync::{Arc, Mutex};
use crate::common;
use motif::*;

#[tokio::test]
async fn test_hook_before_run_called() {
    let counter = Arc::new(Mutex::new(0usize));
    let c = counter.clone();
    struct H { c: Arc<Mutex<usize>> }
    #[async_trait::async_trait]
    impl AgentHook for H {
        async fn before_run(&self, _: &mut RunContext) -> motif::Result<()> {
            *self.c.lock().unwrap() += 1; Ok(())
        }
    }
    let mut agent = Agent::new(common::MockProvider::new(vec![common::text("ok")]))
        .model("test").hook(H { c });
    agent.chat("hi").await.unwrap();
    assert_eq!(*counter.lock().unwrap(), 1, "before_run should be called exactly once");
}

#[tokio::test]
async fn test_hook_on_message_can_filter() {
    struct FilterHook;
    #[async_trait::async_trait]
    impl AgentHook for FilterHook {
        async fn on_message(&self, m: &TimedMessage) -> motif::Result<bool> {
            match &m.message {
                Message::User(ref u) => Ok(!u.content.contains("secret")),
                _ => Ok(true),
            }
        }
    }
    let mut agent = Agent::new(common::MockProvider::new(vec![common::text("response")]))
        .model("test").hook(FilterHook);
    agent.chat("my secret is xyz").await.unwrap();
    let user_msgs: Vec<_> = agent.history_ref().get_all().iter()
        .filter(|m| matches!(m.message, Message::User(_))).collect();
    assert!(user_msgs.is_empty(), "User message with 'secret' should be filtered out");
}

#[tokio::test]
async fn test_hook_on_stop_check_can_gate() {
    use std::sync::atomic::{AtomicBool, Ordering};
    // GateHook: opens the gate after the first stop attempt
    // This means the agent MUST call the LLM at least 2 times
    // (first text response → gate prevents stop → second LLM call → gate allows stop)
    struct GateHook { gate_opened: AtomicBool }
    #[async_trait::async_trait]
    impl AgentHook for GateHook {
        async fn on_stop_check(&self, _: &mut HookContext, should_stop: bool) -> motif::Result<bool> {
            if self.gate_opened.load(Ordering::SeqCst) { return Ok(should_stop); }
            self.gate_opened.store(true, Ordering::SeqCst);
            Ok(false) // block the first stop attempt
        }
    }
    let hook = GateHook { gate_opened: AtomicBool::new(false) };
    let mut agent = Agent::new(common::MockProvider::new(vec![
        common::text("first"), common::text("second"),
    ])).model("test").hook(hook).max_iterations(10);
    // Gate blocks first stop → agent runs again → gets "second" → stops
    let r = agent.chat("test").await.unwrap();
    assert_eq!(r, "second", "Gate should force agent to second response");
}

#[tokio::test]
async fn test_hook_finalize_content_pipeline() {
    struct PrefixHook;
    impl AgentHook for PrefixHook {
        fn finalize_content(&self, c: &str) -> String { format!("[A]{}", c) }
    }
    struct SuffixHook;
    impl AgentHook for SuffixHook {
        fn finalize_content(&self, c: &str) -> String { format!("{}[B]", c) }
    }
    let mut agent = Agent::new(common::MockProvider::new(vec![common::text("body")]))
        .model("test").hook(PrefixHook).hook(SuffixHook);
    let r = agent.chat("x").await.unwrap();
    assert!(r.starts_with("[A]"), "Should have prefix: {}", r);
    assert!(r.ends_with("[B]"), "Should have suffix: {}", r);
}

#[tokio::test]
async fn test_hook_on_error_called() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let called = Arc::new(AtomicBool::new(false));
    let c = called.clone();
    struct ErrHook { c: Arc<AtomicBool> }
    #[async_trait::async_trait]
    impl AgentHook for ErrHook {
        async fn on_error(&self, _: &mut HookContext, _: &Error) -> motif::Result<()> {
            self.c.store(true, Ordering::SeqCst);
            Ok(())
        }
    }
    // Use a provider that panics on call to trigger error path
    struct PanicProvider;
    #[async_trait::async_trait]
    impl LLMProvider for PanicProvider {
        async fn call(&self, _: &[Message], _: &[ToolDefinition]) -> motif::Result<LLMResponse> {
            Err(Error::Custom("forced error".into()))
        }
    }
    let mut agent = Agent::new(PanicProvider).model("test").hook(ErrHook { c });
    let r = agent.chat("x").await;
    assert!(r.is_err(), "Should return error");
    assert!(called.load(Ordering::SeqCst), "on_error should have been called");
}
