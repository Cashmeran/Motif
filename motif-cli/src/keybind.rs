//! Custom keybindings. Extend via `KeyRegistry::new().add(...)`.
//! Future: load user overrides from `~/.motif/keybinds.json`.
//!
//! Note: rustyline's custom command callbacks require an `EventHandler` which
//! is not yet wired in Motif. This module defines the data model and registry.
//! Integration with the input loop will happen when concrete bindings
//! (Ctrl+S save, Ctrl+O load) are needed.

use crate::commands::{Outcome, Registry as CmdRegistry};
use crate::config::Config;
use motif::Agent;

pub struct Binding {
    pub key_name: &'static str,
    pub action: fn(&mut Agent, &Config, &CmdRegistry) -> Outcome,
}

pub struct KeyRegistry {
    bindings: Vec<Binding>,
}

impl KeyRegistry {
    pub fn new() -> Self { Self { bindings: vec![] } }

    pub fn add(mut self, key_name: &'static str, action: fn(&mut Agent, &Config, &CmdRegistry) -> Outcome) -> Self {
        self.bindings.push(Binding { key_name, action });
        self
    }
}
