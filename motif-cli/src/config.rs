//! Configuration persistence at `~/.motif/config.json`.

use serde::{Deserialize, Serialize};
use std::env;
use std::io::Write;
use std::path::PathBuf;

const DEFAULT_BASE_URL: &str = "https://api.deepseek.com";
const DEFAULT_MODEL: &str = "deepseek-v4-pro";

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub api_key: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub streaming: Option<bool>,
    #[serde(default = "default_thinking_effort")]
    pub thinking_effort: String,
    #[serde(default)]
    pub extra_body: Option<serde_json::Map<String, serde_json::Value>>,
}

fn default_base_url() -> String {
    DEFAULT_BASE_URL.into()
}
fn default_model() -> String {
    DEFAULT_MODEL.into()
}
fn default_thinking_effort() -> String {
    "max".into()
}

pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".motif")
        .join("config.json")
}

pub fn load_or_create() -> Config {
    let path = config_path();

    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<Config>(&data) {
                return cfg;
            }
        }
    }

    if let Ok(key) = env::var("MOTIF_API_KEY") {
        return Config {
            api_key: key,
            base_url: env::var("MOTIF_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into()),
            model: env::var("MOTIF_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into()),
            thinking_effort: env::var("MOTIF_THINKING_EFFORT")
                .unwrap_or_else(|_| default_thinking_effort()),
            streaming: Some(true),
            extra_body: None,
        };
    }

    eprint!("Enter your API key: ");
    std::io::stderr().flush().ok();
    let mut key = String::new();
    std::io::stdin().read_line(&mut key).ok();
    let key = key.trim().to_string();
    if key.is_empty() {
        eprintln!("No API key provided.");
        std::process::exit(1);
    }

    let cfg = Config {
        api_key: key,
        base_url: DEFAULT_BASE_URL.into(),
        model: DEFAULT_MODEL.into(),
        thinking_effort: default_thinking_effort(),
        streaming: Some(true),
        extra_body: None,
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string_pretty(&cfg) {
        if std::fs::write(&path, &json).is_ok() {
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

pub fn make_agent(cfg: &Config) -> motif::Agent {
    use crate::hooks;
    use motif::Agent;
    use motif::OpenAIProvider;
    use motif_tools;

    let mut provider = OpenAIProvider::new(&cfg.base_url, &cfg.api_key, &cfg.model);
    provider = provider.with_thinking(&cfg.thinking_effort);
    if let Some(ref extra) = cfg.extra_body {
        for (k, v) in extra {
            provider = provider.with_body(k, v.clone());
        }
    }

    Agent::new(provider)
        .history(motif_session::FileHistory::new(None))
        .model(&cfg.model)
        .tool(motif_tools::search::register())
        .tool(motif_tools::read::register())
        .tool(motif_tools::write::register())
        .tool(motif_tools::edit::register())
        .tool(motif_tools::web_fetch::register())
        .tool(motif_tools::bash::register())
        .hook(hooks::StreamPrinter)
        .hook(hooks::ContextInjection)
}
