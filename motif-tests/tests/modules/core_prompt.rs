use crate::common;
use motif::*;

struct TestExt;
impl PromptBuilder for TestExt {
    fn build(&self) -> Option<String> {
        Some("EXTENSION_TEXT".to_string())
    }
}

#[tokio::test]
async fn test_prompt_builder_extension() {
    let provider = common::MockProvider::new(vec![common::text("ok")]);
    let mut agent = Agent::new(provider).model("test").prompt_builder(TestExt);
    let r = agent.chat("hi").await.unwrap();
    assert_eq!(r, "ok");
}

#[tokio::test]
async fn test_prompt_system_exists() {
    let provider = common::MockProvider::new(vec![common::text("ok")]);
    let mut agent = Agent::new(provider).model("test");
    let r = agent.chat("hi").await.unwrap();
    assert_eq!(r, "ok");
}
