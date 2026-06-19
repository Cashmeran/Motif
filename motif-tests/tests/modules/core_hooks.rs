use std::sync::{Arc, Mutex};
use crate::common;
use motif::*;

struct CountHook { count: Mutex<usize> }
#[async_trait::async_trait]
impl AgentHook for CountHook {
    async fn before_run(&self, _: &mut RunContext) -> motif::Result<()> {
        *self.count.lock().unwrap() += 1;
        Ok(())
    }
}

#[tokio::test]
async fn test_hook_before_run_called() {
    let counter = Arc::new(Mutex::new(0usize));
    let c = counter.clone();
    struct H { c: Arc<Mutex<usize>> }
    #[async_trait::async_trait]
    impl AgentHook for H {
        async fn before_run(&self, _: &mut RunContext) -> motif::Result<()> {
            *self.c.lock().unwrap() += 1;
            Ok(())
        }
    }
    let provider = common::MockProvider::new(vec![common::text("ok")]);
    let mut agent = Agent::new(provider).model("test").hook(H { c });
    agent.chat("hi").await.unwrap();
    assert!(*counter.lock().unwrap() > 0, "before_run should be called");
}
