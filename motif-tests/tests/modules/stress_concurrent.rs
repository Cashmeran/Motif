use crate::common;
use motif::*;

#[tokio::test]
async fn test_5_agents_parallel() {
    let mut handles = vec![];
    for i in 0..5 {
        let handle = tokio::spawn(async move {
            let provider = common::MockProvider::new(vec![common::text(&format!("r{}", i))]);
            let mut agent = Agent::new(provider).model("test");
            agent.chat(&format!("ping {}", i)).await.unwrap()
        });
        handles.push(handle);
    }
    for h in handles {
        assert!(!h.await.unwrap().is_empty());
    }
}
