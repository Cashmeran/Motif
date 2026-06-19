use crate::commands::{Command, Outcome, Registry};
use crate::config::Config;
use motif::Agent;
use motif_session::FileHistory;

pub struct List;

#[async_trait::async_trait]
impl Command for List {
    fn name(&self) -> &'static str {
        "list"
    }
    fn desc(&self) -> &'static str {
        "List sessions"
    }
    async fn run(&self, _: &mut Agent, _: &str, _: &Config, _: &Registry) -> Outcome {
        let sessions = FileHistory::list();
        if sessions.is_empty() {
            println!("No sessions.");
            return Outcome::Continue;
        }
        for s in &sessions {
            let id = s["id"].as_str().unwrap_or("?");
            let date = s["date"].as_str().unwrap_or("?");
            let count = s["count"].as_u64().unwrap_or(0);
            let first = s["first"].as_str().unwrap_or("");
            println!("{:<14} {} {:>4} msgs  {}", id, date, count, first);
        }
        Outcome::Continue
    }
}
