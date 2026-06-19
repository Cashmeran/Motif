use crate::commands::{Command, Outcome, Registry};
use crate::config::Config;
use motif::Agent;

pub struct Help;

#[async_trait::async_trait]
impl Command for Help {
    fn name(&self) -> &'static str {
        "help"
    }
    fn desc(&self) -> &'static str {
        "Show commands"
    }
    async fn run(&self, _: &mut Agent, _: &str, _: &Config, reg: &Registry) -> Outcome {
        println!("Commands:");
        for (name, desc) in reg.list() {
            println!("  /{:<10} {}", name, desc);
        }
        Outcome::Continue
    }
}
