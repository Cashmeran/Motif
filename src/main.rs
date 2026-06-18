//! Motif CLI — terminal chat interface.
//!
//! Usage:
//!   MOTIF_API_KEY=sk-... motif                    # DeepSeek (default)
//!   MOTIF_BASE_URL=https://api.openai.com/v1 \
//!   MOTIF_MODEL=gpt-4o MOTIF_API_KEY=sk-... motif # OpenAI

use motif::*;
use rustyline::{error::ReadlineError, DefaultEditor};
use std::env;

const DEFAULT_BASE_URL: &str = "https://api.deepseek.com/v1";
const DEFAULT_MODEL: &str = "deepseek-chat";

#[tokio::main]
async fn main() {
    let api_key = env::var("MOTIF_API_KEY").unwrap_or_else(|_| {
        eprintln!("MOTIF_API_KEY not set. Export it or pass as env var.");
        std::process::exit(1);
    });
    let base_url = env::var("MOTIF_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
    let model = env::var("MOTIF_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

    let provider = OpenAIProvider::new(&base_url, &api_key, &model);
    let mut agent = Agent::new(provider)
        .model(&model)
        .max_iterations(100);

    println!("Motif CLI · model: {} · /help /clear /exit", model);

    let mut editor = DefaultEditor::new().expect("Failed to init line editor");

    loop {
        match editor.readline("> ") {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() { continue; }
                if line == "/exit" || line == "/quit" { break; }
                if line == "/clear" { agent = new_agent(&base_url, &api_key, &model); println!("Session cleared."); continue; }
                if line == "/help" { println!("Commands: /help /clear /exit"); continue; }
                if line == "/status" {
                    println!("Tokens used: {} | History: {} messages | Model: {}",
                        agent.total_tokens_used(), agent.history_ref().get_all().len(), model);
                    continue;
                }

                let _ = editor.add_history_entry(&line);

                match agent.chat(&line).await {
                    Ok(response) => {
                        println!("\n{}\n", response);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
        }
    }
}

fn new_agent(base_url: &str, api_key: &str, model: &str) -> Agent {
    let provider = OpenAIProvider::new(base_url, api_key, model);
    Agent::new(provider).model(model).max_iterations(100)
}
