use crate::commands::{Command, Outcome, Registry};
use crate::config::Config;
use motif::Agent;

pub struct Status;

#[async_trait::async_trait]
impl Command for Status {
    fn name(&self) -> &'static str {
        "status"
    }
    fn desc(&self) -> &'static str {
        "Token/model info"
    }
    async fn run(&self, agent: &mut Agent, _: &str, _: &Config, _: &Registry) -> Outcome {
        println!(
            "Model: {} | Tokens: {} | Messages: {}",
            agent.get_model(),
            agent.total_tokens_used(),
            agent.history_ref().get_all().len(),
        );
        Outcome::Continue
    }
}
