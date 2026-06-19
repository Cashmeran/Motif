//! Live API tests — require MOTIF_API_KEY env var.
//! All tests are #[ignore] — run with `cargo test -- --ignored`.

use motif::*;

fn live_provider() -> OpenAIProvider {
    let api_key = std::env::var("MOTIF_API_KEY")
        .expect("MOTIF_API_KEY not set");
    let base_url = std::env::var("MOTIF_BASE_URL")
        .unwrap_or_else(|_| "https://api.deepseek.com".into());
    let model = std::env::var("MOTIF_MODEL")
        .unwrap_or_else(|_| "deepseek-chat".into());
    OpenAIProvider::new(&base_url, &api_key, &model)
}

#[tokio::test]
#[ignore]
async fn test_live_simple_chat() {
    let mut agent = Agent::new(live_provider()).model("deepseek-chat");
    let result = agent.chat("Say 'hello' and nothing else.").await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_live_tool_use() {
    #[tool]
    async fn add(a: i64, b: i64) -> String {
        format!("{}", a + b)
    }

    let mut agent = Agent::new(live_provider())
        .model("deepseek-chat")
        .tool_fn(add);

    let result = agent.chat("What is 123 + 456? Use the add tool.").await.unwrap();
    assert!(!result.is_empty());
    assert!(result.contains("579"));
}

#[tokio::test]
#[ignore]
async fn test_live_token_counting() {
    let mut agent = Agent::new(live_provider()).model("deepseek-chat");
    agent.chat("Hello").await.unwrap();
    let tokens = agent.total_tokens_used();
    assert!(tokens > 0, "Should track token usage");
}

#[tokio::test]
#[ignore]
async fn test_live_streaming_structure() {
    let mut agent = Agent::new(live_provider()).model("deepseek-chat");
    // chat_stream returns text directly (streaming is consumed internally)
    let result = agent.chat_stream("Say 'streaming works'").await.unwrap();
    assert!(result.to_lowercase().contains("streaming") || !result.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_live_stop_condition_on_stuck() {
    let mut agent = Agent::new(live_provider())
        .model("deepseek-chat")
        .stop_when(StopCondition::OnStuck { max_repeats: 5 });
    let result = agent.chat("Repeat the word 'stuck' exactly 3 times").await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_live_tool_macro() {
    #[tool(name = "web_lookup")]
    async fn search_web(query: String) -> String {
        format!("Results for: {}", query)
    }

    let mut agent = Agent::new(live_provider())
        .model("deepseek-chat")
        .tool_fn(search_web);

    let result = agent.chat("Search for 'Rust agent framework' using web_lookup").await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_live_multi_tool_conversation() {
    #[tool] async fn get_date() -> String { "2024-01-15".into() }
    #[tool] async fn get_weather(city: String) -> String {
        format!("Weather in {}: sunny", city)
    }

    let mut agent = Agent::new(live_provider())
        .model("deepseek-chat")
        .tool_fn(get_date)
        .tool_fn(get_weather);

    let result = agent.chat("What's the date and weather in Paris?").await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_live_many_rounds() {
    let mut agent = Agent::new(live_provider())
        .model("deepseek-chat")
        .stop_when(StopCondition::AfterNTools(3));

    let result = agent.chat("Count from 1 to 3, saying each number.").await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_live_error_recovery_tool() {
    #[tool]
    async fn risky_op() -> String { "Error: something went wrong".into() }

    let mut agent = Agent::new(live_provider())
        .model("deepseek-chat")
        .tool_fn(risky_op);

    let result = agent.chat("Run risky_op and handle any errors.").await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_live_long_output() {
    let mut agent = Agent::new(live_provider()).model("deepseek-chat");
    let result = agent.chat("List the numbers 1 through 20, one per line.").await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_live_system_prompt_obedience() {
    let mut agent = Agent::new(live_provider()).model("deepseek-chat");
    // Ask for a number — the agent should return a numeric response
    let result = agent.chat("Reply with exactly the number 42 and nothing else.").await.unwrap();
    assert!(result.contains("42"));
}

#[tokio::test]
#[ignore]
async fn test_live_custom_temperature() {
    let provider = live_provider().with_body("temperature", serde_json::Value::Number(serde_json::Number::from(0)));
    let mut agent = Agent::new(provider).model("deepseek-chat");
    let result = agent.chat("Say hello").await.unwrap();
    assert!(!result.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_live_name_attribute() {
    #[tool(name = "web_lookup")]
    async fn search_web_2(query: String) -> String {
        format!("Search results for: {}", query)
    }

    let mut agent = Agent::new(live_provider())
        .model("deepseek-chat")
        .tool_fn(search_web_2);

    let result = agent.chat("Use 'web_lookup' to search for 'test'").await.unwrap();
    assert!(!result.is_empty());
}
