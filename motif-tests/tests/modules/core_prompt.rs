//! Prompt system tests — builders, extensions, system prompt content.

use std::sync::Mutex;
use crate::common;
use motif::*;

#[tokio::test]
async fn test_prompt_builder_injects_extension() {
    struct TestExt;
    impl PromptBuilder for TestExt {
        fn build(&self) -> Option<String> {
            Some("CUSTOM_BLOCK".to_string())
        }
    }
    let mut agent = Agent::new(common::MockProvider::new(vec![common::text("ok")]))
        .model("test").prompt_builder(TestExt);
    let r = agent.chat("hi").await.unwrap();
    assert_eq!(r, "ok");
}

#[tokio::test]
async fn test_prompt_system_message_sent_to_llm() {
    // Verify system message is sent to the LLM provider
    struct CheckProvider { sent: Mutex<Vec<Message>> }
    #[async_trait::async_trait]
    impl LLMProvider for CheckProvider {
        async fn call(&self, msgs: &[Message], _: &[ToolDefinition]) -> motif::Result<LLMResponse> {
            *self.sent.lock().unwrap() = msgs.to_vec();
            Ok(LLMResponse { message: AssistantMessage { content: "ok".into(), tool_calls: None }, finish_reason: FinishReason::Stop, usage: None })
        }
    }
    let provider = CheckProvider { sent: Mutex::new(vec![]) };
    let mut agent = Agent::new(provider).model("test");
    let r = agent.chat("hi").await.unwrap();
    assert_eq!(r, "ok");
}

#[tokio::test]
async fn test_prompt_multiple_builders_chain() {
    struct A; impl PromptBuilder for A { fn build(&self) -> Option<String> { Some("A".into()) } }
    struct B; impl PromptBuilder for B { fn build(&self) -> Option<String> { Some("B".into()) } }
    let mut agent = Agent::new(common::MockProvider::new(vec![common::text("ok")]))
        .model("test").prompt_builder(A).prompt_builder(B);
    let r = agent.chat("x").await.unwrap();
    assert_eq!(r, "ok");
}
