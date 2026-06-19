use crate::common;
use motif::*;

#[tokio::test]
async fn test_20_iterations() {
    let responses: Vec<_> = (0..20).map(|i| common::text(&format!("m{}", i))).collect();
    let provider = common::MockProvider::new(responses);
    let mut agent = Agent::new(provider).model("test");
    let r = agent.chat("start").await.unwrap();
    assert!(!r.is_empty());
}
