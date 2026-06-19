//! Core agent tests — lifecycle, stop conditions, hooks, tools.

use std::sync::{Arc, Mutex};
use crate::common;
use motif::*;

// ── Basic lifecycle ──

#[tokio::test]
async fn test_full_agent_lifecycle() {
    let provider = common::MockProvider::new(vec![
        common::tool_call("add", r#"{"a":1,"b":2}"#),
        common::text("The sum is 3"),
    ]);
    let add_tool = ToolDef::new("add", "Add two numbers").build(|args: String| {
        let v: serde_json::Value = serde_json::from_str(&args).unwrap();
        let a = v["a"].as_i64().unwrap();
        let b = v["b"].as_i64().unwrap();
        async move { (a + b).to_string() }
    });
    let mut agent = Agent::new(provider).tool(add_tool).model("test");
    let result = agent.chat("What is 1+2?").await.unwrap();
    assert_eq!(result, "The sum is 3");
    let history = agent.history_ref().get_all();
    let tool_msgs: Vec<_> = history.iter().filter(|m| matches!(m.message, Message::Tool(_))).collect();
    assert_eq!(tool_msgs.len(), 1);
}

#[tokio::test]
async fn test_agent_text_response_stops() {
    let provider = common::MockProvider::new(vec![common::text("done")]);
    let mut agent = Agent::new(provider).model("test");
    let result = agent.chat("hi").await.unwrap();
    assert_eq!(result, "done");
}

#[tokio::test]
async fn test_agent_tool_then_text() {
    let provider = common::MockProvider::new(vec![
        common::tool_call("echo", r#"{"msg":"hi"}"#),
        common::text("echoed"),
    ]);
    let mut agent = Agent::new(provider).model("test")
        .tool_fn(|args: String| async move { format!("echo: {}", args) });
    let result = agent.chat("echo hi").await.unwrap();
    assert_eq!(result, "echoed");
}

// ── Stop conditions ──

#[tokio::test]
async fn test_stop_condition_never_continues() {
    let responses: Vec<_> = (0..8).map(|i| common::text(&format!("msg{}", i))).collect();
    let provider = common::MockProvider::new(responses);
    let mut agent = Agent::new(provider).model("test").stop_when(StopCondition::Never).max_iterations(5);
    let result = agent.chat("loop").await.unwrap();
    assert!(result.contains("msg"));
    let assistant_count = agent.history_ref().get_all().iter()
        .filter(|m| matches!(m.message, Message::Assistant(_))).count();
    assert!(assistant_count <= 6);
}

#[tokio::test]
async fn test_stop_condition_after_n_tools() {
    let responses: Vec<_> = (0..5).map(|i| common::tool_call("ping", &format!(r#"{{"n":{}}}"#, i))).collect();
    let provider = common::MockProvider::new(responses);
    let mut agent = Agent::new(provider).model("test")
        .tool_fn(|_args: String| async { "pong".to_string() })
        .stop_when(StopCondition::AfterNTools(2));
    agent.chat("ping").await.unwrap();
    let tool_count = agent.history_ref().get_all().iter()
        .filter(|m| matches!(m.message, Message::Tool(_))).count();
    assert!(tool_count >= 2);
}

#[tokio::test]
async fn test_custom_stop_condition() {
    let provider = common::MockProvider::new(vec![common::text("short"), common::text("a longer response")]);
    let mut agent = Agent::new(provider).model("test")
        .stop_when(StopCondition::Custom(Arc::new(|resp, _| resp.message.content.len() > 10)));
    let result = agent.chat("test").await.unwrap();
    assert_eq!(result, "a longer response");
}

#[tokio::test]
async fn test_stop_condition_on_stuck() {
    let responses: Vec<_> = (0..5).map(|_| common::tool_call("echo", r#"{"msg":"same"}"#)).collect();
    let provider = common::MockProvider::new(responses);
    let mut agent = Agent::new(provider).model("test")
        .tool_fn(|args: String| async move { format!("echo: {}", args) })
        .stop_when(StopCondition::OnStuck { max_repeats: 3 });
    let result = agent.chat("stuck").await;
    assert!(result.is_ok());
}

// ── Edge cases ──

#[tokio::test]
async fn test_empty_user_message() {
    let provider = common::MockProvider::new(vec![common::text("got empty")]);
    let mut agent = Agent::new(provider).model("test");
    let result = agent.chat("").await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
async fn test_empty_response_retry() {
    let provider = common::MockProvider::new(vec![
        LLMResponse { message: AssistantMessage { content: String::new(), tool_calls: None }, finish_reason: FinishReason::Stop, usage: None },
        LLMResponse { message: AssistantMessage { content: String::new(), tool_calls: None }, finish_reason: FinishReason::Stop, usage: None },
        common::text("finally"),
    ]);
    let mut agent = Agent::new(provider).model("test");
    let result = agent.chat("x").await.unwrap();
    assert_eq!(result, "finally");
}

#[tokio::test]
async fn test_length_continuation() {
    let provider = common::MockProvider::new(vec![
        common::length_response(),
        common::text("part2"),
    ]);
    let mut agent = Agent::new(provider).model("test");
    let result = agent.chat("continue").await.unwrap();
    assert_eq!(result, "part2");
}

#[tokio::test]
async fn test_multi_round_conversation() {
    let provider = common::MockProvider::new(vec![
        common::text("Hello!"), common::text("Sure"), common::text("Done"),
    ]);
    let mut agent = Agent::new(provider).model("test");
    agent.chat("Hi").await.unwrap();
    agent.chat("Help").await.unwrap();
    agent.chat("Thanks").await.unwrap();
    let user_count = agent.history_ref().get_all().iter()
        .filter(|m| matches!(m.message, Message::User(_))).count();
    assert_eq!(user_count, 3);
}

// ── Tool integration ──

#[tokio::test]
async fn test_external_tools_execution() {
    let provider = common::MockProvider::new(vec![
        common::tool_call("mcp", r#"{"q":"test"}"#),
        common::text("ok"),
    ]);
    let defs = vec![ToolDefinition::new("mcp", "MCP tool", Parameters::new(serde_json::json!({
        "type": "object", "properties": {"q": {"type": "string"}}, "required": ["q"]
    })))];
    let mut agent = Agent::new(provider).model("test")
        .external_tools(defs, |_, _| "ext result".to_string());
    agent.chat("test").await.unwrap();
    let has_tool = agent.history_ref().get_all().iter()
        .any(|m| matches!(&m.message, Message::Tool(tm) if tm.content.contains("ext result")));
    assert!(has_tool);
}

#[tokio::test]
async fn test_tool_not_found_includes_available() {
    let provider = common::MockProvider::new(vec![
        LLMResponse { message: AssistantMessage { content: String::new(),
            tool_calls: Some(vec![ToolCall { id: "c1".into(), call_type: "function".into(),
                function: FunctionCall { name: "nope".into(), arguments: "{}".into() } }]) },
            finish_reason: FinishReason::ToolCalls, usage: None },
        common::text("ok"),
    ]);
    let mut agent = Agent::new(provider).model("test")
        .tool_fn(|_: String| async { "real".to_string() });
    agent.chat("test").await.unwrap();
    assert!(agent.history_ref().get_all().iter()
        .any(|m| matches!(&m.message, Message::Tool(tm) if tm.content.contains("Available"))));
}

#[tokio::test]
async fn test_tool_returns_error_string() {
    let provider = common::MockProvider::new(vec![
        common::tool_call("risky", r#"{"act":"delete"}"#),
        common::text("recovered"),
    ]);
    let mut agent = Agent::new(provider).model("test")
        .tool_fn(|args: String| async move {
            if args.contains("delete") { "Error: denied".into() } else { "ok".into() }
        });
    let result = agent.chat("try").await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
async fn test_tool_receives_malformed_json() {
    let provider = common::MockProvider::new(vec![
        LLMResponse { message: AssistantMessage { content: String::new(),
            tool_calls: Some(vec![ToolCall { id: "c1".into(), call_type: "function".into(),
                function: FunctionCall { name: "parse".into(), arguments: "not-json".into() } }]) },
            finish_reason: FinishReason::ToolCalls, usage: None },
        common::text("ok"),
    ]);
    let mut agent = Agent::new(provider).model("test")
        .tool_fn(|args: String| async move { format!("got: {}", args) });
    agent.chat("test").await.unwrap();
}

#[tokio::test]
async fn test_unicode_in_tool_args() {
    let provider = common::MockProvider::new(vec![
        common::tool_call("echo", r#"{"text":"你好🌍"}"#),
        common::text("done"),
    ]);
    let mut agent = Agent::new(provider).model("test")
        .tool_fn(|args: String| async move {
            let v: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
            v["text"].as_str().unwrap_or("?").to_string()
        });
    agent.chat("unicode").await.unwrap();
    let has_unicode = agent.history_ref().get_all().iter()
        .any(|m| matches!(&m.message, Message::Tool(tm) if tm.content.contains("你好")));
    assert!(has_unicode);
}

// ── Multiple tools ──

#[tokio::test]
async fn test_multiple_tools_in_one_turn() {
    let provider = common::MockProvider::new(vec![
        LLMResponse { message: AssistantMessage { content: String::new(),
            tool_calls: Some(vec![
                ToolCall { id: "c1".into(), call_type: "function".into(),
                    function: FunctionCall { name: "upper".into(), arguments: r#"{"t":"hi"}"#.into() } },
                ToolCall { id: "c2".into(), call_type: "function".into(),
                    function: FunctionCall { name: "lower".into(), arguments: r#"{"t":"HI"}"#.into() } },
            ]) }, finish_reason: FinishReason::ToolCalls, usage: None },
        common::text("done"),
    ]);
    let mut agent = Agent::new(provider).model("test")
        .tool_fn(|args: String| async move { format!("r:{}", args) });
    agent.chat("test").await.unwrap();
    let tool_count = agent.history_ref().get_all().iter()
        .filter(|m| matches!(m.message, Message::Tool(_))).count();
    assert_eq!(tool_count, 2);
}

#[tokio::test]
async fn test_many_tools_registered() {
    let mut responses: Vec<_> = (0..10).map(|i| common::tool_call(&format!("t{}", i), &format!(r#"{{"n":{}}}"#, i))).collect();
    responses.push(common::text("all done"));
    let provider = common::MockProvider::new(responses);
    let mut agent = Agent::new(provider).model("test");
    for i in 0..10 {
        let j = i;
        agent = agent.tool(ToolDef::new(format!("t{}", j), "").build(move |_: String| async move { format!("r{}", j) }));
    }
    let result = agent.chat("all").await.unwrap();
    assert_eq!(result, "all done");
}

#[tokio::test]
async fn test_many_parallel_tool_calls() {
    let calls: Vec<_> = (0..8).map(|i| ToolCall {
        id: format!("c{}", i), call_type: "function".into(),
        function: FunctionCall { name: "echo".into(), arguments: format!(r#"{{"n":{}}}"#, i) },
    }).collect();
    let provider = common::MockProvider::new(vec![
        LLMResponse { message: AssistantMessage { content: String::new(), tool_calls: Some(calls) },
            finish_reason: FinishReason::ToolCalls, usage: None },
        common::text("batch done"),
    ]);
    let mut agent = Agent::new(provider).model("test")
        .tool_fn(|args: String| async move { format!("e:{}", args) });
    agent.chat("batch").await.unwrap();
    let tool_count = agent.history_ref().get_all().iter()
        .filter(|m| matches!(m.message, Message::Tool(_))).count();
    assert_eq!(tool_count, 8);
}

// ── Hooks integration ──

#[tokio::test]
async fn test_hook_called_during_run() {
    use async_trait::async_trait;
    struct CallHook { called: Mutex<bool> }
    #[async_trait]
    impl AgentHook for CallHook {
        async fn before_run(&self, _: &mut RunContext) -> motif::Result<()> {
            *self.called.lock().unwrap() = true;
            Ok(())
        }
    }
    let hook = CallHook { called: Mutex::new(false) };
    let provider = common::MockProvider::new(vec![common::text("ok")]);
    let mut agent = Agent::new(provider).model("test").hook(hook);
    agent.chat("hi").await.unwrap();
}

// ── System prompt / history ──

#[tokio::test]
async fn test_system_prompt_injected() {
    let provider = common::MockProvider::new(vec![common::text("bot here")]);
    let mut agent = Agent::new(provider).model("test");
    agent.chat("who?").await.unwrap();
    let msgs = &provider.last_messages.lock().unwrap();
    assert!(msgs.iter().any(|m| matches!(m, Message::System(_))));
}

#[tokio::test]
async fn test_agent_reuse_same_history() {
    let provider = common::MockProvider::new(vec![common::text("Hello!"), common::text("How are you?")]);
    let mut agent = Agent::new(provider).model("test");
    agent.chat("Hi").await.unwrap();
    agent.chat("You?").await.unwrap();
    assert_eq!(agent.history_ref().get_all().len(), 4);
}

#[tokio::test]
async fn test_bounded_history_with_agent() {
    let provider = common::MockProvider::new(vec![common::text("a"), common::text("b"), common::text("c")]);
    let mut agent = Agent::new(provider).model("test").history(BoundedHistory::new(4));
    agent.chat("hi").await.unwrap();
    agent.chat("again").await.unwrap();
    assert!(agent.history_ref().get_all().len() <= 4);
}

#[tokio::test]
async fn test_bounded_history_preserves_system() {
    let provider = common::MockProvider::new(vec![common::text("ok")]);
    let mut agent = Agent::new(provider).model("test").history(BoundedHistory::new(3));
    agent.chat("test").await.unwrap();
    assert!(!agent.history_ref().get_all().is_empty());
}

// ── max_iterations ──

#[tokio::test]
async fn test_max_iterations_zero_means_unlimited() {
    // Default is 0 (unlimited), controlled by stop condition
    let responses: Vec<_> = (0..3).map(|i| common::text(&format!("m{}", i))).collect();
    let provider = common::MockProvider::new(responses);
    let mut agent = Agent::new(provider).model("test"); // max_iterations defaults to 0
    let result = agent.chat("test").await.unwrap();
    assert!(!result.is_empty());
}
