use crate::commands::{Command, Outcome, Registry};
use crate::config::{self, Config};
use motif::Agent;

pub struct Clear;

#[async_trait::async_trait]
impl Command for Clear {
    fn name(&self) -> &'static str {
        "clear"
    }
    fn desc(&self) -> &'static str {
        "New session"
    }
    async fn run(&self, agent: &mut Agent, _: &str, cfg: &Config, _: &Registry) -> Outcome {
        *agent = config::make_agent(cfg);
        println!("Session cleared.");
        Outcome::Continue
    }
}
