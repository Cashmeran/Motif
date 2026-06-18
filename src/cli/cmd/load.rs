use crate::commands::{Command, Outcome, Registry};
use crate::config::{self, Config};
use motif::core::agent::Agent;
use motif::core::history::FileHistory;

pub struct Load;

#[async_trait::async_trait]
impl Command for Load {
    fn name(&self) -> &'static str { "load" }
    fn desc(&self) -> &'static str { "Load session" }
    async fn run(&self, agent: &mut Agent, args: &str, cfg: &Config, _: &Registry) -> Outcome {
        let id = args.trim();
        if id.is_empty() { println!("Usage: /load <id>"); return Outcome::Continue; }
        match FileHistory::load(id) {
            Some(h) => {
                *agent = config::make_agent(cfg).history(h);
                println!("Loaded session {}", id);
            }
            None => println!("Session not found: {}", id),
        }
        Outcome::Continue
    }
}
