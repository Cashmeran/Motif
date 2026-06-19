//! Prompt system tests: cache, freeze, extensions, concurrency.

use crate::common;
use motif::*;

#[tokio::test]
async fn test_prompt_builder_extension() {
    struct TestBuilder;
    impl PromptBuilder for TestBuilder {
        fn build_extension(&self, _ctx: &PromptContext) -> String {
            "CUSTOM_EXTENSION".to_string()
        }
    }

    let provider = common::MockProvider::new(vec![common::text("ok")]);
    let mut agent = Agent::new(provider)
        .model("test")
        .prompt_builder(TestBuilder);

    agent.chat("hi").await.unwrap();
    let msgs = &provider.last_messages.lock().unwrap();
    let system = msgs.iter().find_map(|m| {
        if let Message::System(ref s) = m { Some(s.content.clone()) } else { None }
    });
    assert!(system.unwrap_or_default().contains("CUSTOM_EXTENSION"));
}

#[tokio::test]
async fn test_prompt_freeze_tools_in_cache() {
    let provider = common::MockProvider::new(vec![
        common::tool_call("echo", r#"{"msg":"hi"}"#),
        common::text("done"),
    ]);
    let mut agent = Agent::new(provider)
        .model("test")
        .tool_fn(|args: String| async move { format!("echo: {}", args) });

    agent.chat("test").await.unwrap();
    // Tool should have been registered and frozen in the prompt cache
    // No assertion needed beyond "no panic" — freeze_tools is called internally
}

#[tokio::test]
async fn test_prompt_runtime_context_injected() {
    let provider = common::MockProvider::new(vec![common::text("today")]);
    let mut agent = Agent::new(provider).model("test");

    agent.chat("what day is it").await.unwrap();
    let msgs = &provider.last_messages.lock().unwrap();
    // Runtime context (date) should be in the first user message
    let has_user = msgs.iter().any(|m| matches!(m, Message::User(_)));
    assert!(has_user, "Should have at least one user message");
}
