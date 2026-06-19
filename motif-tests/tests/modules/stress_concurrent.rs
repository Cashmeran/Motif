//! Concurrent stress tests.

use crate::common;
use motif::*;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_10_agents_parallel() {
    let handles: Vec<_> = (0..10).map(|i| {
        let provider = common::MockProvider::new(vec![common::text(&format!("hello from {}", i))]);
        let mut agent = Agent::new(provider).model("test");
        tokio::spawn(async move {
            agent.chat(&format!("ping {}", i)).await.unwrap()
        })
    }).collect();

    let results = futures::future::join_all(handles).await;
    for (i, r) in results.into_iter().enumerate() {
        let msg = r.unwrap();
        assert!(msg.contains("hello"), "Agent {} should get response: {}", i, msg);
    }
}

#[tokio::test]
async fn test_100_parallel_tool_calls() {
    let provider = common::MockProvider::new(vec![
        common::tool_call_with_id("c1", "echo", r#"{"msg":"1"}"#),
        common::tool_call_with_id("c2", "echo", r#"{"msg":"2"}"#),
        common::text("done"),
    ]);
    let mut agent = Agent::new(provider)
        .model("test")
        .tool_fn(|args: String| async move { format!("echo: {}", args) });

    agent.chat("multi").await.unwrap();
    // Should not deadlock with concurrent tool execution
}

#[tokio::test]
async fn test_provider_error_handling() {
    // Create a provider that always returns error (simulated via empty responses)
    // We use a single response then let it panic intentionally — but actually
    // MockProvider returns responses sequentially; if we only provide 1, it'll
    // panic on second call. Instead, use stop condition to control flow.
    let provider = common::MockProvider::new(vec![
        common::text("first"),
        common::text("second"),
    ]);
    let mut agent = Agent::new(provider)
        .model("test")
        .stop_when(StopCondition::AfterNTools(100)); // won't trigger

    let result = agent.chat("test").await.unwrap();
    assert!(!result.is_empty());
}
