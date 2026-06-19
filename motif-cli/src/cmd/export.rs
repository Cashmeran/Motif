use crate::commands::{Command, Outcome, Registry};
use crate::config::Config;
use motif::Agent;
use motif_session::FileHistory;

pub struct Export;

#[async_trait::async_trait]
impl Command for Export {
    fn name(&self) -> &'static str { "export" }
    fn desc(&self) -> &'static str { "Export session as JSON" }
    async fn run(&self, _: &mut Agent, args: &str, _: &Config, _: &Registry) -> Outcome {
        let id = args.trim();
        if id.is_empty() {
            println!("Usage: /export <session-id>");
            return Outcome::Continue;
        }
        match FileHistory::export(id) {
            Some(json) => println!("{}", json),
            None => println!("Session {} not found or empty", id),
        }
        Outcome::Continue
    }
}
