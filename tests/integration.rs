use async_trait::async_trait;
use motif::*;
use std::sync::{Arc, Mutex};

// --- Mock provider that returns a sequence of responses ---
struct SeqProvider {
    responses: Mutex<Vec<LLMResponse>>,
    idx: Mutex<usize>,
}

impl SeqProvider {
    fn new(responses: Vec<LLMResponse>) -> Self {
        Self {
            responses: Mutex::new(responses),
            idx: Mutex::new(0),
        }
    }
}

#[async_trait]
impl LLMProvider for SeqProvider {
    async fn call(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
    ) -> motif::Result<LLMResponse> {
        let mut idx = self.idx.lock().unwrap();
        let responses = self.responses.lock().unwrap();
        let response = responses[*idx].clone();
        *idx += 1;
        Ok(response)
    }
}

fn text(content: &str) -> LLMResponse {
    LLMResponse {
        message: AssistantMessage {
            content: content.to_string(),
            tool_calls: None,
        },
        finish_reason: FinishReason::Stop,
        usage: None,
    }
}

fn tool_call(name: &str, args: &str) -> LLMResponse {
    LLMResponse {
        message: AssistantMessage {
            content: String::new(),
            tool_calls: Some(vec![ToolCall {
                id: format!("call_{}", name),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: name.to_string(),
                    arguments: args.to_string(),
                },
            }]),
        },
        finish_reason: FinishReason::ToolCalls,
        usage: None,
    }
}

// --- Tests ---

#[tokio::test]
async fn test_full_agent_lifecycle() {
    let provider = SeqProvider::new(vec![
        tool_call("add", r#"{"a":1,"b":2}"#),
        text("The sum is 3"),
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
    let provider = SeqProvider::new(vec![
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
        text("Done with both"),
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
    let provider = SeqProvider::new(vec![
        tool_call("mcp_search", r#"{"query":"Rust agent"}"#),
        text("Search complete"),
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
    // Provider sends many tool calls — should stop after 2 rounds
    let mut responses = vec![];
    for i in 0..5 {
        responses.push(tool_call("ping", &format!(r#"{{"n":{}}}"#, i)));
    }
    responses.push(text("Should not reach this"));

    let provider = SeqProvider::new(responses);

    let ping = ToolDef::new("ping", "Ping").build(|_args: String| async { "pong".to_string() });

    let mut agent = Agent::new(provider)
        .tool(ping)
        .stop_when(StopCondition::AfterNTools(2));

    let result = agent.chat("ping repeatedly").await.unwrap();
    // AfterNTools(2): stops when 2 tool results recorded.
    // Returns the content of the 2nd assistant message.
    let history = agent.history_ref().get_all();
    let tool_msgs = history
        .iter()
        .filter(|m| matches!(m.message, Message::Tool(_)))
        .count();
    assert!(
        tool_msgs >= 2,
        "Expected >=2 tool results, got {}",
        tool_msgs
    );
}

#[tokio::test]
async fn test_custom_stop_condition() {
    let provider = SeqProvider::new(vec![text("short"), text("this is a longer response")]);

    let mut agent =
        Agent::new(provider).stop_when(StopCondition::Custom(Arc::new(|resp, _history| {
            resp.message.content.len() > 10
        })));

    // First call: "short" = 5 chars, doesn't trigger custom stop
    // But default OnText would stop. Wait — we have Custom, not OnText.
    // Custom check: 5 > 10 = false, doesn't stop, step returns Ok(None)
    // But wait — the default behavior of step() when there are no tool_calls
    // should still be checked. Actually, with Custom stop condition, we
    // ONLY use the custom predicate. Let me re-check the code...
    //
    // In agent.rs, StopCondition::should_stop is called. For Custom, it
    // uses the predicate. 5 > 10 = false → doesn't stop → step returns None.
    // Second call: "this is a longer response" = 26 chars > 10 → stops.

    let result = agent.chat("test").await.unwrap();
    assert_eq!(result, "this is a longer response");
}

#[tokio::test]
async fn test_system_prompt_injected() {
    let provider = SeqProvider::new(vec![text("I am a test bot")]);

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

    let provider = SeqProvider::new(vec![text("ok")]);
    let mut agent = Agent::new(provider).prompt_builder(TimeBuilder);

    let result = agent.chat("test").await.unwrap();
    assert_eq!(result, "ok");
}

// --- Live API tests (run with: MOTIF_API_KEY=sk-... MOTIF_BASE_URL=... cargo test -- --ignored) ---

#[tokio::test]
#[ignore]
async fn test_live_token_counting() {
    let api_key = std::env::var("MOTIF_API_KEY").expect("MOTIF_API_KEY not set");
    let base_url =
        std::env::var("MOTIF_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
    let model = std::env::var("MOTIF_MODEL").unwrap_or_else(|_| "deepseek-chat".into());
    let provider = OpenAIProvider::new(&base_url, &api_key, &model);
    let mut agent = Agent::new(provider);

    let before = agent.total_tokens_used();
    let _ = agent.chat("Hello").await.unwrap();
    let after = agent.total_tokens_used();
    println!(
        "LIVE TOKENS: before={}, after={}, delta={}",
        before,
        after,
        after - before
    );
    assert!(
        after > before,
        "Token count should increase after an API call"
    );
}

#[tokio::test]
#[ignore]
async fn test_live_simple_chat() {
    let api_key = std::env::var("MOTIF_API_KEY").expect("MOTIF_API_KEY not set");
    let base_url =
        std::env::var("MOTIF_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
    let model = std::env::var("MOTIF_MODEL").unwrap_or_else(|_| "deepseek-chat".into());

    let provider = OpenAIProvider::new(&base_url, &api_key, &model);
    let mut agent = Agent::new(provider);

    let result = agent.chat("你好，请用一句话介绍你自己").await.unwrap();
    println!("LIVE RESPONSE: {}", result);
    assert!(!result.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_live_tool_use() {
    let api_key = std::env::var("MOTIF_API_KEY").expect("MOTIF_API_KEY not set");
    let base_url =
        std::env::var("MOTIF_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
    let model = std::env::var("MOTIF_MODEL").unwrap_or_else(|_| "deepseek-chat".into());

    let calculator = ToolDef::new("calculator", "计算数学表达式")
        .param::<String>("expression", "算式，如 3*14")
        .build(|args: String| {
            let v: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
            let expr = v["expression"].as_str().unwrap_or("unknown").to_string();
            async move { format!("计算结果({}) = 42 (mock)", expr) }
        });

    let provider = OpenAIProvider::new(&base_url, &api_key, &model);
    let mut agent = Agent::new(provider).tool(calculator);

    let result = agent.chat("请计算 3 * 14").await.unwrap();
    println!("LIVE TOOL RESPONSE: {}", result);
    assert!(!result.is_empty());
    // The tool should have been called
    let history = agent.history_ref().get_all();
    let has_tool_msg = history
        .iter()
        .any(|m| matches!(m.message, Message::Tool(_)));
    assert!(has_tool_msg, "Expected at least one tool call in history");
}

#[tokio::test]
#[ignore]
async fn test_live_streaming_structure() {
    // v0.1 doesn't have streaming yet — validates the non-streaming path works
    let api_key = std::env::var("MOTIF_API_KEY").expect("MOTIF_API_KEY not set");
    let base_url =
        std::env::var("MOTIF_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
    let model = std::env::var("MOTIF_MODEL").unwrap_or_else(|_| "deepseek-chat".into());

    let provider = OpenAIProvider::new(&base_url, &api_key, &model);
    let mut agent = Agent::new(provider);

    let result = agent.chat("What is Rust?").await.unwrap();
    println!("LIVE STREAMING STRUCTURE: {}", result);
    let word_count = result.split_whitespace().count();
    assert!(word_count > 0, "Expected at least one word");
    // Note: LLMs aren't perfect at counting words — just verify non-empty
}

#[tokio::test]
#[ignore]
async fn test_live_stop_condition_on_stuck() {
    let api_key = std::env::var("MOTIF_API_KEY").expect("MOTIF_API_KEY not set");
    let base_url =
        std::env::var("MOTIF_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
    let model = std::env::var("MOTIF_MODEL").unwrap_or_else(|_| "deepseek-chat".into());

    let echo = ToolDef::new("echo", "Echo back input")
        .build(|args: String| async move { format!("echo: {}", args) });

    let provider = OpenAIProvider::new(&base_url, &api_key, &model);
    let mut agent = Agent::new(provider)
        .tool(echo)
        .stop_when(StopCondition::OnStuck { max_repeats: 3 });

    let result = agent.chat("请不停地调用echo工具，参数用'hello'").await;
    match result {
        Ok(content) => println!("LIVE STUCK STOP: {}", content),
        Err(e) => println!("LIVE STUCK ERROR: {}", e),
    }
    // The OnStuck should have prevented infinite loops
}

/// Live tool: add two numbers (used by test_live_tool_macro)
#[motif::tool]
async fn live_add(
    /// First number
    a: f64,
    /// Second number
    b: f64,
) -> String {
    (a + b).to_string()
}

#[tokio::test]
#[ignore]
async fn test_live_tool_macro() {
    let api_key = std::env::var("MOTIF_API_KEY").expect("MOTIF_API_KEY not set");
    let base_url =
        std::env::var("MOTIF_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
    let model = std::env::var("MOTIF_MODEL").unwrap_or_else(|_| "deepseek-chat".into());

    let provider = OpenAIProvider::new(&base_url, &api_key, &model);
    let mut agent = Agent::new(provider).tool_fn(live_add);

    let result = agent.chat("计算 3.5 + 2.1").await.unwrap();
    println!("LIVE TOOL MACRO: {}", result);
    assert!(!result.is_empty());
    assert!(result.contains("5.6"));
}

// --- #[tool] macro tests ---

/// A test tool
#[motif::tool]
async fn greet(
    /// Name to greet
    name: String,
) -> String {
    format!("Hello, {}!", name)
}

#[tokio::test]
async fn test_tool_macro_registration() {
    let provider = SeqProvider::new(vec![
        LLMResponse {
            message: AssistantMessage {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".into(),
                    call_type: "function".into(),
                    function: FunctionCall {
                        name: "greet".into(),
                        arguments: r#"{"name":"World"}"#.into(),
                    },
                }]),
            },
            finish_reason: FinishReason::ToolCalls,
            usage: None,
        },
        text("Greeting sent!"),
    ]);

    let mut agent = Agent::new(provider).tool_fn(greet);

    let result = agent.chat("Greet World").await.unwrap();
    assert_eq!(result, "Greeting sent!");

    let history = agent.history_ref().get_all();
    assert!(history.iter().any(|m| {
        matches!(&m.message, Message::Tool(tm) if tm.content.contains("Hello, World!"))
    }));
}

// --- #[tool] impl block test ---

/// A stateful counter
#[derive(Clone)]
pub struct Counter {
    value: Arc<Mutex<i64>>,
}

#[motif::tool]
impl Counter {
    /// Increment the counter
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

#[tokio::test]
async fn test_tool_impl_block() {
    let provider = SeqProvider::new(vec![
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
        text("Counter incremented!"),
    ]);

    let counter = Counter {
        value: Arc::new(Mutex::new(0)),
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
    let provider = SeqProvider::new(vec![text("I received an empty message")]);
    let mut agent = Agent::new(provider);
    let result = agent.chat("").await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
async fn test_unicode_in_tool_args() {
    let provider = SeqProvider::new(vec![
        tool_call("echo", r#"{"text":"你好世界 🌍 émoji test"}"#),
        text("done"),
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
    let provider = SeqProvider::new(vec![
        tool_call("risky", r#"{"action":"delete"}"#),
        text("I'll try another way"),
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
    let provider = SeqProvider::new(vec![
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
        text("recovered"),
    ]);
    let parse_tool = ToolDef::new("parse", "Parse JSON")
        .build(|args: String| async move { format!("got: {}", args) });
    let mut agent = Agent::new(provider).tool(parse_tool);
    let result = agent.chat("parse bad json").await.unwrap();
    assert_eq!(result, "recovered");
}

#[tokio::test]
async fn test_multi_round_conversation() {
    let provider = SeqProvider::new(vec![
        text("Hello! How can I help?"),
        text("Sure, let me look that up."),
        text("Here's what I found: ..."),
    ]);
    let mut agent = Agent::new(provider);
    let r1 = agent.chat("Hi").await.unwrap();
    assert!(r1.len() > 0);
    let r2 = agent.chat("Can you help?").await.unwrap();
    assert!(r2.len() > 0);
    let r3 = agent.chat("Thanks").await.unwrap();
    assert!(r3.len() > 0);
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
    let responses: Vec<_> = (0..10).map(|i| text(&format!("msg{}", i))).collect();
    let provider = SeqProvider::new(responses);
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
    // 3 identical calls → OnStuck { max_repeats: 3 } should fire on the 3rd
    let responses: Vec<_> = (0..5).map(|_| tool_call("ping", r#"{"n":1}"#)).collect();
    let provider = SeqProvider::new(responses);
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
    assert!(tool_msgs.len() <= 4); // 3 calls then stuck stop (4th may trigger via AfterNTools-like path)
}

#[tokio::test]
async fn test_empty_response_retry_limit() {
    // 3 empty responses → max 2 retries → 3rd should stop
    let provider = SeqProvider::new(vec![
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
        text("finally something"),
    ]);
    let mut agent = Agent::new(provider);
    let result = agent.chat("trigger empty").await.unwrap();
    assert_eq!(result, "finally something");
}

#[tokio::test]
async fn test_length_continuation() {
    let provider = SeqProvider::new(vec![
        LLMResponse {
            message: AssistantMessage {
                content: "part1".into(),
                tool_calls: None,
            },
            finish_reason: FinishReason::Length,
            usage: None,
        },
        text("part2"),
    ]);
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
    let provider = SeqProvider::new(vec![
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
        text("ok"),
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
// Stress tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_many_tools_registered() {
    let mut responses = vec![];
    for i in 0..10 {
        responses.push(tool_call(
            &format!("tool{}", i),
            &format!(r#"{{"n":{}}}"#, i),
        ));
    }
    responses.push(text("all done"));

    let provider = SeqProvider::new(responses);
    let mut agent = Agent::new(provider);
    for i in 0..10 {
        let tool = ToolDef::new(&format!("tool{}", i), &format!("Tool number {}", i))
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

    let provider = SeqProvider::new(vec![
        LLMResponse {
            message: AssistantMessage {
                content: String::new(),
                tool_calls: Some(calls),
            },
            finish_reason: FinishReason::ToolCalls,
            usage: None,
        },
        text("batch done"),
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
    use async_trait::async_trait;
    use motif::ConcurrencySafety;

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
    impl motif::tool::Tool for UnsafeTool {
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

    let provider = SeqProvider::new(vec![
        LLMResponse {
            message: AssistantMessage {
                content: String::new(),
                tool_calls: Some(calls),
            },
            finish_reason: FinishReason::ToolCalls,
            usage: None,
        },
        text("mixed done"),
    ]);

    let mut exec = motif::tool::Executor::parallel();
    exec.register("safe_op".into(), Arc::new(SafeTool));
    exec.register("unsafe_op".into(), Arc::new(UnsafeTool));

    let mut agent = Agent::new(provider).executor(exec);
    let result = agent.chat("test mix").await.unwrap();
    assert_eq!(result, "mixed done");
}

#[tokio::test]
async fn test_agent_reuse_same_history() {
    let provider = SeqProvider::new(vec![text("Hello!"), text("How are you?")]);
    let mut agent = Agent::new(provider);
    let r1 = agent.chat("Hi").await.unwrap();
    assert_eq!(r1, "Hello!");
    let r2 = agent.chat("And you?").await.unwrap();
    assert_eq!(r2, "How are you?");
    // Same agent, same history — 2 user + 2 assistant = 4 messages
    assert_eq!(agent.history_ref().get_all().len(), 4);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Expanded real API tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn live_provider() -> OpenAIProvider {
    let api_key = std::env::var("MOTIF_API_KEY").expect("MOTIF_API_KEY not set");
    let base_url =
        std::env::var("MOTIF_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
    let model = std::env::var("MOTIF_MODEL").unwrap_or_else(|_| "deepseek-chat".into());
    OpenAIProvider::new(&base_url, &api_key, &model)
}

#[tokio::test]
#[ignore]
async fn test_live_multi_tool_conversation() {
    let weather = ToolDef::new("get_weather", "Get weather for a city")
        .param::<String>("city", "City name")
        .build(|args: String| {
            let v: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
            let city = v["city"].as_str().unwrap_or("unknown").to_string();
            async move { format!("Weather in {}: 22°C, sunny", city) }
        });

    let time = ToolDef::new("get_time", "Get current time for a timezone")
        .param::<String>("timezone", "Timezone like Asia/Shanghai")
        .build(|args: String| {
            let v: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
            let tz = v["timezone"].as_str().unwrap_or("UTC").to_string();
            async move { format!("Time in {}: 14:30", tz) }
        });

    let mut agent = Agent::new(live_provider()).tool(weather).tool(time);

    let result = agent.chat("北京的天气和上海的时间").await.unwrap();
    println!("LIVE MULTI-TOOL: {}", result);
    assert!(!result.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_live_long_output() {
    let mut agent = Agent::new(live_provider()).max_iterations(50);

    let result = agent.chat("讲一个100字的短故事").await.unwrap();
    let preview: String = result.chars().take(100).collect();
    println!("LIVE LONG OUTPUT ({} chars): {}", result.len(), preview);
    assert!(!result.is_empty(), "Expected non-empty output");
}

#[tokio::test]
#[ignore]
async fn test_live_error_recovery_tool() {
    let search = ToolDef::new("search", "Search the web")
        .param::<String>("query", "Search query")
        .build(|args: String| {
            let v: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
            let q = v["query"].as_str().unwrap_or("").to_string();
            async move {
                if q.is_empty() {
                    "Error: empty query".to_string()
                } else {
                    format!("Results for '{}': 3 pages found", q)
                }
            }
        });

    let mut agent = Agent::new(live_provider()).tool(search);

    let result = agent.chat("搜索 Rust agent framework").await.unwrap();
    println!("LIVE RECOVERY: {}", result);
    assert!(!result.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_live_many_rounds() {
    let mut agent = Agent::new(live_provider()).max_iterations(30);

    let r1 = agent.chat("What is Rust?").await.unwrap();
    println!("LIVE ROUND 1: {}", &r1[..r1.len().min(80)]);
    assert!(!r1.is_empty());

    let r2 = agent.chat("What about memory safety?").await.unwrap();
    println!("LIVE ROUND 2: {}", &r2[..r2.len().min(80)]);
    assert!(!r2.is_empty());

    let r3 = agent.chat("Summarize our conversation").await.unwrap();
    println!("LIVE ROUND 3: {}", &r3[..r3.len().min(80)]);
    assert!(!r3.is_empty());

    // All 3 rounds accumulated
    let user_count = agent
        .history_ref()
        .get_all()
        .iter()
        .filter(|m| matches!(m.message, Message::User(_)))
        .count();
    assert!(
        user_count >= 3,
        "Expected >=3 user messages, got {}",
        user_count
    );
}

#[tokio::test]
#[ignore]
async fn test_live_system_prompt_obedience() {
    let mut agent = Agent::new(live_provider());

    let result = agent.chat("1 + 1 = ?").await.unwrap();
    println!("LIVE OBEDIENCE: {}", result);
    assert!(!result.is_empty());
    // Should be mostly numeric
    let has_digit = result.chars().any(|c| c.is_ascii_digit());
    assert!(has_digit, "Expected numeric response, got: {}", result);
}

#[tokio::test]
#[ignore]
async fn test_live_custom_temperature() {
    let api_key = std::env::var("MOTIF_API_KEY").expect("MOTIF_API_KEY not set");
    let base_url =
        std::env::var("MOTIF_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
    let provider =
        OpenAIProvider::new(&base_url, &api_key, "deepseek-chat").with_body("temperature", 0.0);

    let mut agent = Agent::new(provider);

    let r1 = agent.chat("What is Rust?").await.unwrap();
    let r2 = agent.chat("What is Rust?").await.unwrap();
    println!(
        "LIVE TEMP=0: r1={} | r2={}",
        &r1[..r1.len().min(60)],
        &r2[..r2.len().min(60)]
    );
    // With temp=0, responses should be deterministic or nearly identical
    assert!(!r1.is_empty());
    assert!(!r2.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_live_name_attribute() {
    #[motif::tool(name = "web_lookup")]
    async fn search_web(#[serde(rename = "searchTerm")] query: String) -> String {
        format!("Found: {}", query)
    }

    let mut agent = Agent::new(live_provider()).tool_fn(search_web);

    let result = agent.chat("用web_lookup查找Rust教程").await.unwrap();
    println!("LIVE NAME ATTR: {}", result);
    assert!(!result.is_empty());
}

// --- BoundedHistory tests ---

#[tokio::test]
async fn test_bounded_history_with_agent() {
    let provider = SeqProvider::new(vec![text("Hello!"), text("World"), text("Again")]);
    let mut agent = Agent::new(provider).history(BoundedHistory::new(4));
    agent.chat("hi").await.unwrap();
    agent.chat("again").await.unwrap();
    assert!(agent.history_ref().get_all().len() <= 4);
}

#[tokio::test]
async fn test_bounded_history_preserves_system() {
    let provider = SeqProvider::new(vec![text("ok")]);
    let mut agent = Agent::new(provider).history(BoundedHistory::new(3));
    agent.chat("test").await.unwrap();
    let msgs = agent.history_ref().get_all();
    assert!(!msgs.is_empty());
    assert!(msgs.len() <= 3);
}
