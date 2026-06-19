//! Command registry. Each slash-command lives in `cmd/`.

use crate::cmd;
use crate::config::Config;
use motif::Agent;
use std::collections::HashMap;

pub enum Outcome {
    Continue,
    #[allow(dead_code)] // used in bin target (main.rs), not visible to lib clippy
    Exit,
    PassToAgent(String),
}

#[async_trait::async_trait]
pub trait Command: Send + Sync {
    fn name(&self) -> &'static str;
    fn desc(&self) -> &'static str;
    async fn run(&self, agent: &mut Agent, args: &str, cfg: &Config, reg: &Registry) -> Outcome;
}

pub struct Registry {
    cmds: HashMap<String, Box<dyn Command>>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        let mut r = Self {
            cmds: HashMap::new(),
        };
        r.add(cmd::help::Help);
        r.add(cmd::clear::Clear);
        r.add(cmd::status::Status);
        r.add(cmd::list::List);
        r.add(cmd::load::Load);
        r.add(cmd::config_cmd::ConfigCmd);
        r.add(cmd::delete::Delete);
        r.add(cmd::export::Export);
        r
    }

    pub fn add(&mut self, c: impl Command + 'static) {
        self.cmds.insert(c.name().to_string(), Box::new(c));
    }

    pub async fn handle(&self, input: &str, agent: &mut Agent, cfg: &Config) -> Outcome {
        let trimmed = input.trim();
        if !trimmed.starts_with('/') {
            return Outcome::PassToAgent(trimmed.to_string());
        }
        let (name, args) = match trimmed[1..].split_once(' ') {
            Some((n, a)) => (n, a.trim()),
            None => (&trimmed[1..], ""),
        };
        if let Some(c) = self.cmds.get(name) {
            c.run(agent, args, cfg, self).await
        } else {
            Outcome::Continue
        }
    }

    pub fn list(&self) -> Vec<(&'static str, &'static str)> {
        let mut v: Vec<_> = self.cmds.values().map(|c| (c.name(), c.desc())).collect();
        v.sort_by_key(|(n, _)| *n);
        v
    }
}
