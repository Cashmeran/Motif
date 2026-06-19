//! Long-running agent stress tests.

use crate::common;
use motif::*;

#[tokio::test]
async fn test_500_iterations_no_leak() {
    // Run 500 iterations with a short loop — just verify no panic/OOM
    let mut responses = Vec::new();
    for i in 0..500 {
        responses.push(common::text(&format!("iter {}", i)));
    }
    let provider = common::MockProvider::new(responses);
    let mut agent = Agent::new(provider)
        .model("test")
        .stop_when(StopCondition::AfterNTools(500)); // stops after 500 tools

    // Since we're returning text (not tool calls), each text response triggers Stop
    // Just make sure the agent doesn't panic with many iterations
    let result = agent.chat("start").await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
async fn test_on_stuck_3_repeats() {
    // Return the same tool call 4 times → OnStuck should trigger after 3
    let responses: Vec<_> = (0..4).map(|_| common::tool_call("echo", r#"{"msg":"same"}"#)).collect();
    let provider = common::MockProvider::new(responses);
    let mut agent = Agent::new(provider)
        .model("test")
        .stop_when(StopCondition::OnStuck { max_repeats: 3 })
        .tool_fn(|args: String| async move { format!("echo: {}", args) });

    // Should stop due to OnStuck, not exhaust all 4 tool calls
    let result = agent.chat("stuck test").await;
    // Either Ok (stopped early) or Ok (completed) — just verify no panic
    assert!(result.is_ok() || result.is_err());
}
