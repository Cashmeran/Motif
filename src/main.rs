//! Motif CLI — terminal chat interface. Configuration is read from
//! `~/.motif/config.json` on first launch, falling back to env vars.

use motif::*;
use rustyline::{error::ReadlineError, DefaultEditor};
use serde::{Deserialize, Serialize};
use std::env;
use std::io::Write;
use std::path::PathBuf;

const DEFAULT_BASE_URL: &str = "https://api.deepseek.com/v1";
const DEFAULT_MODEL: &str = "deepseek-chat";

#[derive(Serialize, Deserialize)]
struct Config {
    api_key: String,
    #[serde(default = "default_base_url")]
    base_url: String,
    #[serde(default = "default_model")]
    model: String,
}
fn default_base_url() -> String { DEFAULT_BASE_URL.into() }
fn default_model() -> String { DEFAULT_MODEL.into() }

fn config_path() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".motif").join("config.json")
}

fn load_or_create_config() -> Config {
    let path = config_path();

    // 1. Try config file
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<Config>(&data) {
                return cfg;
            }
        }
    }

    // 2. Try env vars
    if let Ok(key) = env::var("MOTIF_API_KEY") {
        return Config {
            api_key: key,
            base_url: env::var("MOTIF_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into()),
            model: env::var("MOTIF_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into()),
        };
    }

    // 3. Prompt and save
    eprint!("Enter your API key: ");
    std::io::stderr().flush().ok();
    let mut key = String::new();
    std::io::stdin().read_line(&mut key).ok();
    let key = key.trim().to_string();

    if key.is_empty() {
        eprintln!("No API key provided. Set MOTIF_API_KEY or create ~/.motif/config.json");
        std::process::exit(1);
    }

    let cfg = Config { api_key: key, base_url: DEFAULT_BASE_URL.into(), model: DEFAULT_MODEL.into() };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string_pretty(&cfg) {
        if std::fs::write(&path, &json).is_ok() {
            // Restrict config file to owner-only (API key inside)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).ok();
            }
            eprintln!("Config saved to {}", path.display());
        }
    }
    cfg
}

#[tokio::main]
async fn main() {
    let cfg = load_or_create_config();
    let mut agent = make_agent(&cfg);

    println!("Motif CLI · model: {} · /help /clear /exit", cfg.model);

    let mut editor = DefaultEditor::new().expect("Failed to init line editor");

    loop {
        match editor.readline("> ") {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() { continue; }
                if line == "/exit" || line == "/quit" { break; }
                if line == "/clear" { agent = make_agent(&cfg); println!("Session cleared."); continue; }
                if line == "/help" { println!("Commands: /help /clear /exit"); continue; }
                if line == "/status" {
                    println!("Tokens used: {} | History: {} messages | Model: {}",
                        agent.total_tokens_used(), agent.history_ref().get_all().len(), cfg.model);
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
    let provider = OpenAIProvider::new(base_url.to_string(), api_key.to_string(), model.to_string());
    Agent::new(provider).model(model).max_iterations(100)
}

fn make_agent(cfg: &Config) -> Agent {
    let provider = OpenAIProvider::new(
        cfg.base_url.clone(), cfg.api_key.clone(), cfg.model.clone());
    Agent::new(provider).model(&cfg.model).max_iterations(100)
}
