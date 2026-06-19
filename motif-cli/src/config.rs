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
    pub thinking_effort: Option<String>,
    #[serde(default)]
    pub extra_body: Option<serde_json::Map<String, serde_json::Value>>,
}

fn default_base_url() -> String { DEFAULT_BASE_URL.into() }
fn default_model() -> String { DEFAULT_MODEL.into() }

pub fn config_path() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".motif").join("config.json")
}

pub fn load_or_create() -> Config {
    let path = config_path();

    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<Config>(&data) { return cfg; }
        }
    }

    if let Ok(key) = env::var("MOTIF_API_KEY") {
        return Config {
            api_key: key,
            base_url: env::var("MOTIF_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into()),
            model: env::var("MOTIF_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into()),
            thinking_effort: env::var("MOTIF_THINKING_EFFORT").ok(),
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
        thinking_effort: None,
        streaming: Some(true),
        extra_body: None,
    };
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).ok(); }
    if let Ok(json) = serde_json::to_string_pretty(&cfg) {
        if std::fs::write(&path, &json).is_ok() {
            #[cfg(unix)] {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).ok();
            }
            eprintln!("Config saved to {}", path.display());
        }
    }
    cfg
}

pub fn make_agent(cfg: &Config) -> motif::Agent {
    use motif::Agent;
    use motif::OpenAIProvider;
    use motif_tools;

    let mut provider = OpenAIProvider::new(&cfg.base_url, &cfg.api_key, &cfg.model);
    if let Some(ref effort) = cfg.thinking_effort { provider = provider.with_thinking(effort); }
    if let Some(ref extra) = cfg.extra_body {
        for (k, v) in extra { provider = provider.with_body(k, v.clone()); }
    }
    
    Agent::new(provider)
        .history(motif_session::FileHistory::new(None))
        .model(&cfg.model)
        // max_iterations uses core default (0 = unlimited)
        .tool(motif_tools::search::register())
        .tool(motif_tools::read::register())
        .tool(motif_tools::write::register())
        .hook(StreamPrinter)
}

/// Hook that prints streaming content deltas to stdout.
struct StreamPrinter;
#[async_trait::async_trait]
impl motif::AgentHook for StreamPrinter {
    async fn on_stream_delta(&self, delta: &str) -> motif::Result<()> {
        use std::io::Write;
        print!("{}", delta);
        std::io::stdout().flush().ok();
        Ok(())
    }
}
