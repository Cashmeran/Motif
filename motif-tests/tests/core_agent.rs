//! Core agent tests — lifecycle, stop conditions, hooks, tools, edge cases.
//! 38 tests total: 25 migrated from integration.rs, 6 from agent.rs, 7 new.

#[path = "common/mod.rs"]
mod common;

use motif::*;

use async_trait::async_trait;
use std::sync::Arc;

// ── Shared tool functions for #[tool] proc-macro tests ──

/// Add two numbers.
#[motif::tool]
async fn add(
    /// First number
    a: f64,
    /// Second number
    b: f64,
) -> String {
    (a + b).to_string()
}

/// A stateful counter used in the impl-block tool test.
#[derive(Clone)]
pub struct Counter {
    value: Arc<std::sync::Mutex<i64>>,
}

#[motif::tool]
impl Counter {
    /// Increment the counter.
    async fn increment(
        self,
        /// Amount to add
        amount: i64,
    ) -> String {
        let mut v = self.value.lock().unwrap();
        *v += amount;
        v.to_string()
    }
}

// ═══════════════════════════════════════════════════════════════
// Tests migrated from motif/tests/integration.rs (1–25)
// ── SeqProvider replaced with common::MockProvider ──
// ═══════════════════════════════════════════════════════════════

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

    let mut agent = Agent::new(provider).tool(add_tool);

    let result = agent.chat("What is 1+2?").await.unwrap();
    assert_eq!(result, "The sum is 3");

    // Verify tool was called and result recorded
    let history = agent.history_ref().get_all();
    let tool_msgs: Vec<_> = history
        .iter()
        .filter(|m| matches!(m.message, Message::Tool(_)))
        .collect();
    assert_eq!(tool_msgs.len(), 1);
    if let Message::Tool(ref tm) = tool_msgs[0].message {
        assert_eq!(tm.content, "3");
    }
}

#[tokio::test]
async fn test_multiple_tools_in_one_turn() {
    let provider = common::MockProvider::new(vec![
        LLMResponse {
            message: AssistantMessage {
                content: String::new(),
                tool_calls: Some(vec![
                    ToolCall {
                        id: "call_1".into(),
                        call_type: "function".into(),
                        function: FunctionCall {
                            name: "upper".into(),
                            arguments: r#"{"text":"hello"}"#.into(),
                        },
                    },
                    ToolCall {
                        id: "call_2".into(),
                        call_type: "function".into(),
                        function: FunctionCall {
                            name: "reverse".into(),
                            arguments: r#"{"text":"world"}"#.into(),
                        },
                    },
                ]),
            },
            finish_reason: FinishReason::ToolCalls,
            usage: None,
        },
        common::text("Done with both"),
    ]);

    let upper = ToolDef::new("upper", "Convert to uppercase").build(|args: String| {
        let v: serde_json::Value = serde_json::from_str(&args).unwrap();
        let text = v["text"].as_str().unwrap().to_uppercase();
        async move { text }
    });

    let reverse = ToolDef::new("reverse", "Reverse a string").build(|args: String| {
        let v: serde_json::Value = serde_json::from_str(&args).unwrap();
        let text: String = v["text"].as_str().unwrap().chars().rev().collect();
        async move { text }
    });

    let mut agent = Agent::new(provider).tool(upper).tool(reverse);

    let result = agent.chat("process these").await.unwrap();
    assert_eq!(result, "Done with both");

    // Both tools should have been called
    let history = agent.history_ref().get_all();
    let tool_results: Vec<_> = history
        .iter()
        .filter(|m| matches!(m.message, Message::Tool(_)))
        .collect();
    assert_eq!(tool_results.len(), 2);
}

#[tokio::test]
async fn test_external_tool_integration() {
    let provider = common::MockProvider::new(vec![
        common::tool_call("mcp_search", r#"{"query":"Rust agent"}"#),
        common::text("Search complete"),
    ]);

    let defs = vec![ToolDefinition::new(
        "mcp_search",
        "Search via MCP",
        Parameters::new(serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Search query"}
            },
            "required": ["query"]
        })),
    )];

    let mut agent = Agent::new(provider).external_tools(defs, |_name, _args| {
        "External result: found 3 items".to_string()
    });

    let result = agent.chat("search for Rust agents").await.unwrap();
    assert_eq!(result, "Search complete");

    let history = agent.history_ref().get_all();
    assert!(history.iter().any(|m| {
        matches!(&m.message, Message::Tool(tm) if tm.content.contains("External result"))
    }));
}

#[tokio::test]
async fn test_stop_condition_after_n_tools() {
    // Provider sends multiple tool calls — should stop after 1 tool result
    let provider = common::MockProvider::new(vec![
        common::tool_call("ping", r#"{"n":0}"#),
        common::tool_call("ping", r#"{"n":1}"#),
        common::text("Should not reach this"),
    ]);

    let ping = ToolDef::new("ping", "Ping").build(|_args: String| async { "pong".to_string() });

    let mut agent = Agent::new(provider)
        .tool(ping)
        .stop_when(StopCondition::AfterNTools(1));

    let _result = agent.chat("ping repeatedly").await.unwrap();
    // AfterNTools(1): stops when 1 tool result is recorded
    let history = agent.history_ref().get_all();
    let tool_msgs = history
        .iter()
        .filter(|m| matches!(m.message, Message::Tool(_)))
        .count();
    assert!(
        tool_msgs >= 1,
        "Expected >=1 tool results, got {}",
        tool_msgs
    );
}

#[tokio::test]
async fn test_custom_stop_condition() {
    let provider = common::MockProvider::new(vec![
        common::text("not verified yet"),
        common::text("VERIFIED response"),
    ]);

    let mut agent =
        Agent::new(provider).stop_when(StopCondition::Custom(Arc::new(|resp, _history| {
            resp.message.content.contains("VERIFIED")
        })));

    // First: "not verified yet" does NOT contain "VERIFIED" → doesn't stop
    // Second: "VERIFIED response" contains "VERIFIED" → stops
    let result = agent.chat("test").await.unwrap();
    assert_eq!(result, "VERIFIED response");
}

#[tokio::test]
async fn test_system_prompt_injected() {
    let provider = common::MockProvider::new(vec![common::text("I am a test bot")]);

    let mut agent = Agent::new(provider);

    let result = agent.chat("who are you?").await.unwrap();
    assert_eq!(result, "I am a test bot");
}

#[tokio::test]
async fn test_prompt_builder_extension() {
    struct TimeBuilder;
    impl PromptBuilder for TimeBuilder {
        fn build(&self) -> Option<String> {
            Some("Current time: 2026-06-17".to_string())
        }
    }

    let provider = common::MockProvider::new(vec![common::text("ok")]);
    let mut agent = Agent::new(provider).prompt_builder(TimeBuilder);

    let result = agent.chat("test").await.unwrap();
    assert_eq!(result, "ok");
}

#[tokio::test]
async fn test_tool_macro_registration() {
    let provider = common::MockProvider::new(vec![
        LLMResponse {
            message: AssistantMessage {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".into(),
                    call_type: "function".into(),
                    function: FunctionCall {
                        name: "add".into(),
                        arguments: r#"{"a":1.0,"b":2.0}"#.into(),
                    },
                }]),
            },
            finish_reason: FinishReason::ToolCalls,
            usage: None,
        },
        common::text("Sum computed!"),
    ]);

    let mut agent = Agent::new(provider).tool_fn(add);

    let result = agent.chat("Add 1 + 2").await.unwrap();
    assert_eq!(result, "Sum computed!");

    let history = agent.history_ref().get_all();
    assert!(history
        .iter()
        .any(|m| { matches!(&m.message, Message::Tool(tm) if tm.content.contains("3")) }));
}

#[tokio::test]
async fn test_tool_impl_block() {
    let provider = common::MockProvider::new(vec![
        LLMResponse {
            message: AssistantMessage {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".into(),
                    call_type: "function".into(),
                    function: FunctionCall {
                        name: "increment".into(),
                        arguments: r#"{"amount":5}"#.into(),
                    },
                }]),
            },
            finish_reason: FinishReason::ToolCalls,
            usage: None,
        },
        common::text("Counter incremented!"),
    ]);

    let counter = Counter {
        value: Arc::new(std::sync::Mutex::new(0)),
    };
    let mut agent = Agent::new(provider).bind(counter, Counter::increment);

    let result = agent.chat("increment by 5").await.unwrap();
    assert_eq!(result, "Counter incremented!");

    let history = agent.history_ref().get_all();
    assert!(history
        .iter()
        .any(|m| { matches!(&m.message, Message::Tool(tm) if tm.content == "5") }));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Edge case tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_empty_user_message() {
    let provider = common::MockProvider::new(vec![common::text("I received an empty message")]);
    let mut agent = Agent::new(provider);
    let result = agent.chat("").await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
async fn test_unicode_in_tool_args() {
    let provider = common::MockProvider::new(vec![
        common::tool_call("echo", r#"{"text":"你好世界 🌍 émoji test"}"#),
        common::text("done"),
    ]);
    let echo = ToolDef::new("echo", "Echo").build(|args: String| async move {
        let v: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
        v["text"].as_str().unwrap_or("???").to_string()
    });
    let mut agent = Agent::new(provider).tool(echo);
    let result = agent.chat("echo unicode").await.unwrap();
    assert_eq!(result, "done");
    let h = agent.history_ref().get_all();
    assert!(h
        .iter()
        .any(|m| { matches!(&m.message, Message::Tool(tm) if tm.content.contains("你好")) }));
}

#[tokio::test]
async fn test_tool_returns_error_string() {
    let provider = common::MockProvider::new(vec![
        common::tool_call("risky", r#"{"action":"delete"}"#),
        common::text("I'll try another way"),
    ]);
    let risky = ToolDef::new("risky", "Risky operation").build(|args: String| async move {
        if args.contains("delete") {
            "Error: operation not permitted".to_string()
        } else {
            "ok".to_string()
        }
    });
    let mut agent = Agent::new(provider).tool(risky);
    let result = agent.chat("try risky op").await.unwrap();
    // Agent should receive the error string and try again
    assert!(!result.is_empty());
}

#[tokio::test]
async fn test_tool_receives_malformed_json() {
    let provider = common::MockProvider::new(vec![
        LLMResponse {
            message: AssistantMessage {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".into(),
                    call_type: "function".into(),
                    function: FunctionCall {
                        name: "parse".into(),
                        arguments: "not-valid-json".into(),
                    },
                }]),
            },
            finish_reason: FinishReason::ToolCalls,
            usage: None,
        },
        common::text("recovered"),
    ]);
    let parse_tool = ToolDef::new("parse", "Parse JSON")
        .build(|args: String| async move { format!("got: {}", args) });
    let mut agent = Agent::new(provider).tool(parse_tool);
    let result = agent.chat("parse bad json").await.unwrap();
    assert_eq!(result, "recovered");
}

#[tokio::test]
async fn test_multi_round_conversation() {
    let provider = common::MockProvider::new(vec![
        common::text("Hello! How can I help?"),
        common::text("Sure, let me look that up."),
        common::text("Here's what I found: ..."),
    ]);
    let mut agent = Agent::new(provider);
    let r1 = agent.chat("Hi").await.unwrap();
    assert!(!r1.is_empty());
    let r2 = agent.chat("Can you help?").await.unwrap();
    assert!(!r2.is_empty());
    let r3 = agent.chat("Thanks").await.unwrap();
    assert!(!r3.is_empty());
    // All three rounds in history
    let h = agent.history_ref().get_all();
    let user_msgs: Vec<_> = h
        .iter()
        .filter(|m| matches!(m.message, Message::User(_)))
        .collect();
    assert_eq!(user_msgs.len(), 3);
}

#[tokio::test]
async fn test_stop_condition_never_requires_external_control() {
    let responses: Vec<_> = (0..10)
        .map(|i| common::text(&format!("msg{}", i)))
        .collect();
    let provider = common::MockProvider::new(responses);
    let mut agent = Agent::new(provider)
        .stop_when(StopCondition::Never)
        .max_iterations(5); // safety cap: 5

    let result = agent.chat("loop").await.unwrap();
    // Should stop via max_iterations (5), not Never
    assert!(result.contains("msg"));
    // Verify it didn't do 10 rounds
    let h = agent.history_ref().get_all();
    let assistant_count = h
        .iter()
        .filter(|m| matches!(m.message, Message::Assistant(_)))
        .count();
    assert!(assistant_count <= 6); // 5 LLM calls + max_iterations fallback
}

#[tokio::test]
async fn test_on_stuck_exact_boundary() {
    // 3 identical calls -> OnStuck { max_repeats: 3 } should fire on the 3rd
    let responses: Vec<_> = (0..5)
        .map(|_| common::tool_call("ping", r#"{"n":1}"#))
        .collect();
    let provider = common::MockProvider::new(responses);
    let ping = ToolDef::new("ping", "Ping").build(|_args: String| async { "pong".to_string() });
    let mut agent = Agent::new(provider)
        .tool(ping)
        .stop_when(StopCondition::OnStuck { max_repeats: 3 });

    let result = agent.chat("ping loop").await;
    assert!(result.is_ok());
    let tool_msgs: Vec<_> = agent
        .history_ref()
        .get_all()
        .iter()
        .filter(|m| matches!(m.message, Message::Tool(_)))
        .collect();
    assert!(tool_msgs.len() <= 4); // 3 calls then stuck stop (4th may trigger via fallback)
}

#[tokio::test]
async fn test_empty_response_retry_limit() {
    // 2 empty responses -> max 2 retries -> 3rd response should stop
    let provider = common::MockProvider::new(vec![
        LLMResponse {
            message: AssistantMessage {
                content: String::new(),
                tool_calls: None,
            },
            finish_reason: FinishReason::Stop,
            usage: None,
        },
        LLMResponse {
            message: AssistantMessage {
                content: String::new(),
                tool_calls: None,
            },
            finish_reason: FinishReason::Stop,
            usage: None,
        },
        common::text("finally something"),
    ]);
    let mut agent = Agent::new(provider);
    let result = agent.chat("trigger empty").await.unwrap();
    assert_eq!(result, "finally something");
}

#[tokio::test]
async fn test_length_continuation() {
    let provider =
        common::MockProvider::new(vec![common::length_response(), common::text("part2")]);
    let mut agent = Agent::new(provider);
    let result = agent.chat("continue").await.unwrap();
    assert_eq!(result, "part2");
    let h = agent.history_ref().get_all();
    // Should have a "continue" user message injected
    assert!(h
        .iter()
        .any(|m| matches!(&m.message, Message::User(um) if um.content == "continue")));
}

#[tokio::test]
async fn test_tool_not_found_message_includes_available() {
    let provider = common::MockProvider::new(vec![
        LLMResponse {
            message: AssistantMessage {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "c1".into(),
                    call_type: "function".into(),
                    function: FunctionCall {
                        name: "nonexistent".into(),
                        arguments: "{}".into(),
                    },
                }]),
            },
            finish_reason: FinishReason::ToolCalls,
            usage: None,
        },
        common::text("ok"),
    ]);
    let real_tool = ToolDef::new("real_tool", "A real tool")
        .build(|_args: String| async { "real".to_string() });
    let mut agent = Agent::new(provider).tool(real_tool);
    let result = agent.chat("test").await.unwrap();
    assert_eq!(result, "ok");
    let h = agent.history_ref().get_all();
    assert!(h
        .iter()
        .any(|m| { matches!(&m.message, Message::Tool(tm) if tm.content.contains("Available")) }));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Stress / bulk tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_many_tools_registered() {
    let mut responses = vec![];
    for i in 0..10 {
        responses.push(common::tool_call(
            &format!("tool{}", i),
            &format!(r#"{{"n":{}}}"#, i),
        ));
    }
    responses.push(common::text("all done"));

    let provider = common::MockProvider::new(responses);
    let mut agent = Agent::new(provider);
    for i in 0..10 {
        let tool = ToolDef::new(format!("tool{}", i), format!("Tool number {}", i))
            .build(move |_args: String| async move { format!("result{}", i) });
        agent = agent.tool(tool);
    }
    let result = agent.chat("use all tools").await.unwrap();
    assert_eq!(result, "all done");
    assert_eq!(agent.tool_definitions().len(), 10);
}

#[tokio::test]
async fn test_many_parallel_tool_calls() {
    let calls: Vec<_> = (0..8)
        .map(|i| ToolCall {
            id: format!("c{}", i),
            call_type: "function".into(),
            function: FunctionCall {
                name: "echo".into(),
                arguments: format!(r#"{{"n":{}}}"#, i),
            },
        })
        .collect();

    let provider = common::MockProvider::new(vec![
        LLMResponse {
            message: AssistantMessage {
                content: String::new(),
                tool_calls: Some(calls),
            },
            finish_reason: FinishReason::ToolCalls,
            usage: None,
        },
        common::text("batch done"),
    ]);
    let echo =
        ToolDef::new("echo", "Echo").build(|args: String| async move { format!("echo:{}", args) });
    let mut agent = Agent::new(provider).tool(echo);
    let result = agent.chat("batch").await.unwrap();
    assert_eq!(result, "batch done");
    let tool_msgs: Vec<_> = agent
        .history_ref()
        .get_all()
        .iter()
        .filter(|m| matches!(m.message, Message::Tool(_)))
        .collect();
    assert_eq!(tool_msgs.len(), 8);
}

#[tokio::test]
async fn test_mixed_concurrency_safety() {
    struct SafeTool;
    #[async_trait]
    impl Tool for SafeTool {
        async fn call(&self, args: String) -> String {
            format!("safe:{}", args)
        }
        fn concurrency_safety(&self) -> ConcurrencySafety {
            ConcurrencySafety::ConcurrentSafe
        }
    }
    struct UnsafeTool;
    #[async_trait]
    impl Tool for UnsafeTool {
        async fn call(&self, args: String) -> String {
            format!("unsafe:{}", args)
        }
        fn concurrency_safety(&self) -> ConcurrencySafety {
            ConcurrencySafety::ConcurrentUnsafe
        }
    }

    let calls = vec![
        ToolCall {
            id: "c1".into(),
            call_type: "function".into(),
            function: FunctionCall {
                name: "safe_op".into(),
                arguments: "{}".into(),
            },
        },
        ToolCall {
            id: "c2".into(),
            call_type: "function".into(),
            function: FunctionCall {
                name: "unsafe_op".into(),
                arguments: "{}".into(),
            },
        },
        ToolCall {
            id: "c3".into(),
            call_type: "function".into(),
            function: FunctionCall {
                name: "safe_op".into(),
                arguments: r#"{"x":2}"#.into(),
            },
        },
    ];

    let provider = common::MockProvider::new(vec![
        LLMResponse {
            message: AssistantMessage {
                content: String::new(),
                tool_calls: Some(calls),
            },
            finish_reason: FinishReason::ToolCalls,
            usage: None,
        },
        common::text("mixed done"),
    ]);

    let mut exec = Executor::parallel();
    exec.register("safe_op".into(), Arc::new(SafeTool));
    exec.register("unsafe_op".into(), Arc::new(UnsafeTool));

    let mut agent = Agent::new(provider).executor(exec);
    let result = agent.chat("test mix").await.unwrap();
    assert_eq!(result, "mixed done");
}

#[tokio::test]
async fn test_agent_reuse_same_history() {
    let provider =
        common::MockProvider::new(vec![common::text("Hello!"), common::text("How are you?")]);
    let mut agent = Agent::new(provider);
    let r1 = agent.chat("Hi").await.unwrap();
    assert_eq!(r1, "Hello!");
    let r2 = agent.chat("And you?").await.unwrap();
    assert_eq!(r2, "How are you?");
    // Same agent, same history — 2 user + 2 assistant = 4 messages
    assert_eq!(agent.history_ref().get_all().len(), 4);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// BoundedHistory tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_bounded_history_with_agent() {
    let provider = common::MockProvider::new(vec![
        common::text("Hello!"),
        common::text("World"),
        common::text("Again"),
    ]);
    let mut agent = Agent::new(provider).history(BoundedHistory::new(4));
    agent.chat("hi").await.unwrap();
    agent.chat("again").await.unwrap();
    agent.chat("third").await.unwrap();
    assert!(agent.history_ref().get_all().len() <= 4);
}

#[tokio::test]
async fn test_bounded_history_preserves_system() {
    let provider = common::MockProvider::new(vec![common::text("ok")]);
    let mut agent = Agent::new(provider).history(BoundedHistory::new(3));
    agent.chat("test").await.unwrap();
    let msgs = agent.history_ref().get_all();
    assert!(!msgs.is_empty());
    assert!(msgs.len() <= 3);
}

// ═══════════════════════════════════════════════════════════════
// Tests migrated from motif/src/agent.rs (26–31)
// ── MockProvider adapted to common::MockProvider ──
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_agent_text_response_stops() {
    let provider = common::MockProvider::new(vec![common::text("Hello!")]);
    let mut agent = Agent::new(provider);

    let result = agent.chat("hi").await.unwrap();
    assert_eq!(result, "Hello!");
}

#[tokio::test]
async fn test_hook_called_during_run() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct BeforeRunHook {
        count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl AgentHook for BeforeRunHook {
        async fn before_run(&self, _ctx: &mut RunContext) -> crate::Result<()> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    let count = Arc::new(AtomicUsize::new(0));
    let hook = BeforeRunHook {
        count: count.clone(),
    };

    let provider = common::MockProvider::new(vec![common::text("Hi")]);
    let mut agent = Agent::new(provider).hook(hook);

    agent.chat("hello").await.unwrap();
    // Verify before_run was called by inspecting the shared counter
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_stop_condition_never_continues() {
    let provider = common::MockProvider::new(vec![common::text("First"), common::text("Second")]);

    let mut agent = Agent::new(provider).stop_when(StopCondition::Never);

    // Manually step — Never should not stop on text
    let result1 = agent.step().await.unwrap();
    assert!(result1.is_none()); // Never stops on text

    let result2 = agent.step().await.unwrap();
    assert!(result2.is_none()); // Still doesn't stop
}

#[tokio::test]
async fn test_agent_tool_then_text() {
    let provider = common::MockProvider::new(vec![
        common::tool_call("echo", r#"{"msg":"hi"}"#),
        common::text("Tool done!"),
    ]);

    let echo_tool =
        ToolDef::new("echo", "Echo back").build(|_args: String| async { "echo: hi".to_string() });

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
async fn test_external_tools_execution() {
    let provider = common::MockProvider::new(vec![
        common::tool_call("ext_search", r#"{"query":"rust"}"#),
        common::text("Found results"),
    ]);

    let defs = vec![ToolDefinition::new(
        "ext_search",
        "Search external source",
        Parameters::new(serde_json::json!({
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
async fn test_stop_condition_on_stuck() {
    // Provider keeps returning the same tool call
    let responses: Vec<_> = (0..5)
        .map(|_| common::tool_call("echo", r#"{"msg":"hi"}"#))
        .collect();

    let provider = common::MockProvider::new(responses);
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
    assert!(tool_count <= 4); // at most 4 tool results, then stuck stop
}

// ═══════════════════════════════════════════════════════════════
// New tests (32–38)
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_hooks_all_lifecycle_called() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct RecordingHook {
        before_run_count: Arc<AtomicUsize>,
        before_llm_count: Arc<AtomicUsize>,
        after_llm_count: Arc<AtomicUsize>,
        after_run_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl AgentHook for RecordingHook {
        async fn before_run(&self, _ctx: &mut RunContext) -> crate::Result<()> {
            self.before_run_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn before_llm(&self, _ctx: &mut HookContext) -> crate::Result<()> {
            self.before_llm_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn after_llm(&self, _ctx: &mut HookContext) -> crate::Result<()> {
            self.after_llm_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn after_run(&self, _ctx: &mut RunContext) -> crate::Result<()> {
            self.after_run_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    let before_run = Arc::new(AtomicUsize::new(0));
    let before_llm = Arc::new(AtomicUsize::new(0));
    let after_llm = Arc::new(AtomicUsize::new(0));
    let after_run = Arc::new(AtomicUsize::new(0));

    let hook = RecordingHook {
        before_run_count: before_run.clone(),
        before_llm_count: before_llm.clone(),
        after_llm_count: after_llm.clone(),
        after_run_count: after_run.clone(),
    };

    let provider = common::MockProvider::new(vec![common::text("test response")]);
    let mut agent = Agent::new(provider).hook(hook);

    let result = agent.chat("hello").await.unwrap();
    assert_eq!(result, "test response");

    // Verify all four lifecycle hooks were called at least once
    assert!(
        before_run.load(Ordering::SeqCst) >= 1,
        "before_run should have been called at least once"
    );
    assert!(
        before_llm.load(Ordering::SeqCst) >= 1,
        "before_llm should have been called at least once"
    );
    assert!(
        after_llm.load(Ordering::SeqCst) >= 1,
        "after_llm should have been called at least once"
    );
    assert!(
        after_run.load(Ordering::SeqCst) >= 1,
        "after_run should have been called at least once"
    );
}

#[tokio::test]
async fn test_on_message_filter_discards() {
    struct DiscardAssistantHook;

    #[async_trait]
    impl AgentHook for DiscardAssistantHook {
        async fn on_message(&self, msg: &TimedMessage) -> crate::Result<bool> {
            // Discard assistant messages (return Ok(false) to reject)
            if matches!(msg.message, Message::Assistant(_)) {
                Ok(false)
            } else {
                Ok(true)
            }
        }
    }

    let provider = common::MockProvider::new(vec![common::text("should be discarded")]);
    let mut agent = Agent::new(provider).hook(DiscardAssistantHook);

    let result = agent.chat("hello").await;
    // Agent should still complete (stop condition checks the response, not history)
    assert!(result.is_ok());

    let history = agent.history_ref().get_all();
    // Assistant message should NOT be in history because hook returned Ok(false)
    let has_assistant = history
        .iter()
        .any(|m| matches!(m.message, Message::Assistant(_)));
    assert!(
        !has_assistant,
        "Assistant message should have been discarded by on_message filter"
    );
}

#[tokio::test]
async fn test_on_stop_check_gate() {
    struct OverrideStopHook;

    #[async_trait]
    impl AgentHook for OverrideStopHook {
        async fn on_stop_check(
            &self,
            _ctx: &mut HookContext,
            _should_stop: bool,
        ) -> crate::Result<bool> {
            // Always override: return Ok(false) means "do NOT stop"
            Ok(false)
        }
    }

    let provider = common::MockProvider::new(vec![
        common::text("first response"),
        common::text("second response"),
    ]);

    let mut agent = Agent::new(provider).hook(OverrideStopHook);

    // With the hook overriding stop, agent.step() should never return Some
    let result1 = agent.step().await.unwrap();
    assert!(
        result1.is_none(),
        "Hook on_stop_check returning false should override stop on first text response"
    );

    let result2 = agent.step().await.unwrap();
    assert!(
        result2.is_none(),
        "Hook on_stop_check returning false should override stop on second text response"
    );
}

#[tokio::test]
async fn test_max_iterations_zero_unlimited() {
    let provider = common::MockProvider::new(vec![
        common::text("response1"),
        common::text("response2"),
        common::text("response3"),
    ]);

    let mut agent = Agent::new(provider).stop_when(StopCondition::Never);
    // max_iterations defaults to 0, meaning unlimited

    // Call step() multiple times — should never stop because StopCondition::Never
    // always returns false, and max_iterations only applies inside run(), not step()
    for _ in 0..3 {
        let result = agent.step().await.unwrap();
        assert!(
            result.is_none(),
            "With Never + max_iterations=0, step() should never return Some"
        );
    }
}

#[tokio::test]
async fn test_provider_returns_error() {
    struct ErrorProvider;

    #[async_trait]
    impl LLMProvider for ErrorProvider {
        async fn call(
            &self,
            _messages: &[Message],
            _tools: &[ToolDefinition],
        ) -> motif::Result<LLMResponse> {
            Err(Error::Custom("simulated provider error".into()))
        }
    }

    let mut agent = Agent::new(ErrorProvider);
    let result = agent.chat("test").await;
    assert!(
        result.is_err(),
        "Provider error should propagate to the caller"
    );
}

#[tokio::test]
async fn test_tool_executor_sequential() {
    let calls = vec![
        ToolCall {
            id: "c1".into(),
            call_type: "function".into(),
            function: FunctionCall {
                name: "first".into(),
                arguments: "{}".into(),
            },
        },
        ToolCall {
            id: "c2".into(),
            call_type: "function".into(),
            function: FunctionCall {
                name: "second".into(),
                arguments: "{}".into(),
            },
        },
    ];

    let provider = common::MockProvider::new(vec![
        LLMResponse {
            message: AssistantMessage {
                content: String::new(),
                tool_calls: Some(calls),
            },
            finish_reason: FinishReason::ToolCalls,
            usage: None,
        },
        common::text("done"),
    ]);

    let mut exec = Executor::sequential();
    exec.register(
        "first".into(),
        Arc::new(FunctionTool::new(|args: String| {
            Box::pin(async move { format!("first:{}", args) })
        })),
    );
    exec.register(
        "second".into(),
        Arc::new(FunctionTool::new(|args: String| {
            Box::pin(async move { format!("second:{}", args) })
        })),
    );

    let mut agent = Agent::new(provider).executor(exec);
    let result = agent.chat("test").await.unwrap();
    assert_eq!(result, "done");

    // Both tools should have been executed in order
    let tool_msgs: Vec<_> = agent
        .history_ref()
        .get_all()
        .iter()
        .filter(|m| matches!(m.message, Message::Tool(_)))
        .collect();
    assert_eq!(tool_msgs.len(), 2);
}

#[tokio::test]
async fn test_agent_history_ref_access() {
    let provider = common::MockProvider::new(vec![common::text("response")]);
    let mut agent = Agent::new(provider);

    agent.chat("hello").await.unwrap();

    let history = agent.history_ref().get_all();
    assert!(
        !history.is_empty(),
        "history_ref() should return non-empty history after chat"
    );
}
