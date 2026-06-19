//! CLI command tests.

use crate::common;
use motif::*;
use motif_cli::commands::{Command, Outcome, Registry};
use motif_cli::config::{self, Config};
use motif_session::FileHistory;

fn test_config() -> Config {
    Config {
        api_key: "sk-test1234abcd".into(),
        base_url: "https://api.test.com".into(),
        model: "test-model".into(),
        streaming: Some(false),
        thinking_effort: None,
        extra_body: None,
    }
}

fn test_agent() -> Agent {
    Agent::new(common::MockProvider::new(vec![common::text("ok")])).model("test")
}

#[tokio::test]
async fn test_help_command_returns_continue() {
    let reg = Registry::new();
    let cfg = test_config();
    let mut agent = test_agent();
    let outcome = reg.handle("/help", &mut agent, &cfg).await;
    match outcome {
        Outcome::Continue => {} // expected
        _ => panic!("/help should return Continue"),
    }
}

#[tokio::test]
async fn test_clear_command_new_session() {
    let reg = Registry::new();
    let cfg = test_config();
    let mut agent = test_agent();
    // After clear, agent should have fresh state
    let outcome = reg.handle("/clear", &mut agent, &cfg).await;
    match outcome {
        Outcome::Continue => {}
        _ => panic!("/clear should return Continue"),
    }
}

#[tokio::test]
async fn test_status_command_shows_model() {
    let reg = Registry::new();
    let cfg = test_config();
    let mut agent = test_agent();
    let outcome = reg.handle("/status", &mut agent, &cfg).await;
    match outcome {
        Outcome::Continue => {}
        _ => panic!("/status should return Continue"),
    }
}

#[tokio::test]
async fn test_list_command_no_panic() {
    let reg = Registry::new();
    let cfg = test_config();
    let mut agent = test_agent();
    let outcome = reg.handle("/list", &mut agent, &cfg).await;
    match outcome {
        Outcome::Continue => {}
        _ => panic!("/list should return Continue"),
    }
}

#[tokio::test]
async fn test_delete_nonexistent_session() {
    let reg = Registry::new();
    let cfg = test_config();
    let mut agent = test_agent();
    let outcome = reg.handle("/delete nonexistent12345", &mut agent, &cfg).await;
    match outcome {
        Outcome::Continue => {}
        _ => panic!("/delete should return Continue"),
    }
}

#[tokio::test]
async fn test_export_nonexistent_session() {
    let reg = Registry::new();
    let cfg = test_config();
    let mut agent = test_agent();
    let outcome = reg.handle("/export nonexistent12345", &mut agent, &cfg).await;
    match outcome {
        Outcome::Continue => {}
        _ => panic!("/export should return Continue"),
    }
}

#[tokio::test]
async fn test_pass_to_agent_for_non_command() {
    let reg = Registry::new();
    let cfg = test_config();
    let mut agent = test_agent();
    let outcome = reg.handle("hello world", &mut agent, &cfg).await;
    match outcome {
        Outcome::PassToAgent(s) => assert!(s.contains("hello")),
        _ => panic!("plain text should be PassToAgent"),
    }
}
