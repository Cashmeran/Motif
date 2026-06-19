use crate::commands::{Command, Outcome, Registry};
use crate::config::Config;
use motif::Agent;

pub struct ConfigCmd;

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return "***".to_string();
    }
    format!("{}...{}", &key[..4], &key[key.len() - 4..])
}

#[async_trait::async_trait]
impl Command for ConfigCmd {
    fn name(&self) -> &'static str {
        "config"
    }
    fn desc(&self) -> &'static str {
        "Show current config"
    }
    async fn run(&self, _: &mut Agent, _: &str, cfg: &Config, _: &Registry) -> Outcome {
        println!("base_url:    {}", cfg.base_url);
        println!("model:       {}", cfg.model);
        println!("api_key:     {}", mask_key(&cfg.api_key));
        println!("streaming:   {}", cfg.streaming.unwrap_or(true));
        if let Some(ref e) = cfg.thinking_effort {
            println!("thinking:    {}", e);
        }
        if let Some(ref extra) = cfg.extra_body {
            println!(
                "extra_body:  {}",
                serde_json::to_string_pretty(extra).unwrap_or_default()
            );
        }
        Outcome::Continue
    }
}
