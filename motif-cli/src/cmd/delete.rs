use crate::commands::{Command, Outcome, Registry};
use crate::config::Config;
use motif::Agent;
use motif_session::FileHistory;

pub struct Delete;

#[async_trait::async_trait]
impl Command for Delete {
    fn name(&self) -> &'static str { "delete" }
    fn desc(&self) -> &'static str { "Delete a session by ID" }
    async fn run(&self, _: &mut Agent, args: &str, _: &Config, _: &Registry) -> Outcome {
        let id = args.trim();
        if id.is_empty() {
            println!("Usage: /delete <session-id>");
            return Outcome::Continue;
        }
        if FileHistory::delete(id) {
            println!("Deleted session {}", id);
        } else {
            println!("Session {} not found", id);
        }
        Outcome::Continue
    }
}
