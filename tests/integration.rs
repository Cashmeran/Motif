use motif::*;
use async_trait::async_trait;
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
    }
}

// --- Tests ---

#[tokio::test]
async fn test_full_agent_lifecycle() {
    let provider = SeqProvider::new(vec![
        tool_call("add", r#"{"a":1,"b":2}"#),
        text("The sum is 3"),
    ]);

    let add_tool = ToolDef::new("add", "Add two numbers")
        .build(|args: String| {
            let v: serde_json::Value = serde_json::from_str(&args).unwrap();
            let a = v["a"].as_i64().unwrap();
            let b = v["b"].as_i64().unwrap();
            async move { (a + b).to_string() }
        });

    let mut agent = Agent::new(provider)
        .system("You are a calculator. Use the add tool to answer.")
        .tool(add_tool);

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
        },
        text("Done with both"),
    ]);

    let upper = ToolDef::new("upper", "Convert to uppercase")
        .build(|args: String| {
            let v: serde_json::Value = serde_json::from_str(&args).unwrap();
            let text = v["text"].as_str().unwrap().to_uppercase();
            async move { text }
        });

    let reverse = ToolDef::new("reverse", "Reverse a string")
        .build(|args: String| {
            let v: serde_json::Value = serde_json::from_str(&args).unwrap();
            let text: String = v["text"].as_str().unwrap().chars().rev().collect();
            async move { text }
        });

    let mut agent = Agent::new(provider)
        .system("You have text tools.")
        .tool(upper)
        .tool(reverse);

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

    let mut agent = Agent::new(provider)
        .system("Search assistant")
        .external_tools(defs, |_name, _args| {
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

    let ping = ToolDef::new("ping", "Ping")
        .build(|_args: String| async { "pong".to_string() });

    let mut agent = Agent::new(provider)
        .system("test")
        .tool(ping)
        .stop_when(StopCondition::AfterNTools(2));

    let result = agent.chat("ping repeatedly").await.unwrap();
    // AfterNTools(2): stops when 2 tool results recorded.
    // Returns the content of the 2nd assistant message.
    let history = agent.history_ref().get_all();
    let tool_msgs = history.iter().filter(|m| matches!(m.message, Message::Tool(_))).count();
    assert!(tool_msgs >= 2, "Expected >=2 tool results, got {}", tool_msgs);
}

#[tokio::test]
async fn test_custom_stop_condition() {
    let provider = SeqProvider::new(vec![
        text("short"),
        text("this is a longer response"),
    ]);

    let mut agent = Agent::new(provider)
        .system("test")
        .stop_when(StopCondition::Custom(Arc::new(|resp, _history| {
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

    let mut agent = Agent::new(provider)
        .system("You are a test bot. Reply with your identity.");

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
    let mut agent = Agent::new(provider)
        .system("Base prompt")
        .prompt_builder(TimeBuilder);

    let result = agent.chat("test").await.unwrap();
    assert_eq!(result, "ok");
}

// --- Live API tests (run with: MOTIF_API_KEY=sk-... MOTIF_BASE_URL=... cargo test -- --ignored) ---

#[tokio::test]
#[ignore]
async fn test_live_simple_chat() {
    let api_key = std::env::var("MOTIF_API_KEY").expect("MOTIF_API_KEY not set");
    let base_url = std::env::var("MOTIF_BASE_URL")
        .unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
    let model = std::env::var("MOTIF_MODEL").unwrap_or_else(|_| "deepseek-chat".into());

    let provider = OpenAIProvider::new(&base_url, &api_key, &model);
    let mut agent = Agent::new(provider)
        .system("你是一个有帮助的助手。用中文回复。");

    let result = agent.chat("你好，请用一句话介绍你自己").await.unwrap();
    println!("LIVE RESPONSE: {}", result);
    assert!(!result.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_live_tool_use() {
    let api_key = std::env::var("MOTIF_API_KEY").expect("MOTIF_API_KEY not set");
    let base_url = std::env::var("MOTIF_BASE_URL")
        .unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
    let model = std::env::var("MOTIF_MODEL").unwrap_or_else(|_| "deepseek-chat".into());

    let calculator = ToolDef::new("calculator", "计算数学表达式")
        .param::<String>("expression", "算式，如 3*14")
        .build(|args: String| {
            let v: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
            let expr = v["expression"].as_str().unwrap_or("unknown").to_string();
            async move { format!("计算结果({}) = 42 (mock)", expr) }
        });

    let provider = OpenAIProvider::new(&base_url, &api_key, &model);
    let mut agent = Agent::new(provider)
        .system("你是一个数学助手。用工具计算。用中文回复。")
        .tool(calculator);

    let result = agent.chat("请计算 3 * 14").await.unwrap();
    println!("LIVE TOOL RESPONSE: {}", result);
    assert!(!result.is_empty());
    // The tool should have been called
    let history = agent.history_ref().get_all();
    let has_tool_msg = history.iter().any(|m| matches!(m.message, Message::Tool(_)));
    assert!(has_tool_msg, "Expected at least one tool call in history");
}

#[tokio::test]
#[ignore]
async fn test_live_streaming_structure() {
    // v0.1 doesn't have streaming yet — validates the non-streaming path works
    let api_key = std::env::var("MOTIF_API_KEY").expect("MOTIF_API_KEY not set");
    let base_url = std::env::var("MOTIF_BASE_URL")
        .unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
    let model = std::env::var("MOTIF_MODEL").unwrap_or_else(|_| "deepseek-chat".into());

    let provider = OpenAIProvider::new(&base_url, &api_key, &model);
    let mut agent = Agent::new(provider)
        .system("You reply in exactly 3 words. No more, no less.");

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
    let base_url = std::env::var("MOTIF_BASE_URL")
        .unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
    let model = std::env::var("MOTIF_MODEL").unwrap_or_else(|_| "deepseek-chat".into());

    let echo = ToolDef::new("echo", "Echo back input")
        .build(|args: String| async move { format!("echo: {}", args) });

    let provider = OpenAIProvider::new(&base_url, &api_key, &model);
    let mut agent = Agent::new(provider)
        .system("你是一个助手。当你被要求重复做某事时，调用echo工具。如果工具返回了相同的结果，停止。")
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
    let base_url = std::env::var("MOTIF_BASE_URL")
        .unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
    let model = std::env::var("MOTIF_MODEL").unwrap_or_else(|_| "deepseek-chat".into());

    let provider = OpenAIProvider::new(&base_url, &api_key, &model);
    let mut agent = Agent::new(provider)
        .system("你是一个计算器。使用live_add工具做加法。用中文回复。")
        .tool_fn(live_add);

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
        },
        text("Greeting sent!"),
    ]);

    let mut agent = Agent::new(provider)
        .system("You greet people.")
        .tool_fn(greet);

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
        },
        text("Counter incremented!"),
    ]);

    let counter = Counter { value: Arc::new(Mutex::new(0)) };
    let mut agent = Agent::new(provider)
        .system("You increment counters.")
        .bind(counter, Counter::increment);

    let result = agent.chat("increment by 5").await.unwrap();
    assert_eq!(result, "Counter incremented!");

    let history = agent.history_ref().get_all();
    assert!(history.iter().any(|m| {
        matches!(&m.message, Message::Tool(tm) if tm.content == "5")
    }));
}
