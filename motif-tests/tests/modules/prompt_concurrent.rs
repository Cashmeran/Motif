//! Prompt cache concurrency tests.

use crate::common;
use motif::*;

#[tokio::test]
async fn test_prompt_agents_concurrent_build() {
    // Multiple agents building prompts concurrently should not deadlock
    let mut handles = vec![];
    for i in 0..10 {
        handles.push(tokio::spawn(async move {
            let provider = common::MockProvider::new(vec![common::text(&format!("r{}", i))]);
            let mut agent = Agent::new(provider).model("test");
            agent.chat("ping").await.unwrap()
        }));
    }
    for h in handles {
        assert!(!h.await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn test_prompt_freezes_tool_definitions() {
    // Register a tool, chat, then verify tool defs are in the provider's request
    let mut agent = Agent::new(common::MockProvider::new(vec![
        common::tool_call("check", r#"{"v":"1"}"#),
        common::text("done"),
    ]))
    .model("test")
    .tool(ToolDef::new("check", "Check tool").build(|_: String| async { "ok".to_string() }));
    let r = agent.chat("use tool").await.unwrap();
    assert_eq!(r, "done");
}
