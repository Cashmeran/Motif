//! CLI config tests.

use motif_cli::config::Config;

#[test]
fn test_config_parse_valid() {
    let json = r#"{
        "api_key": "sk-test123",
        "base_url": "https://api.test.com",
        "model": "test-model"
    }"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.api_key, "sk-test123");
    assert_eq!(cfg.model, "test-model");
}

#[test]
fn test_config_default_values() {
    let json = r#"{"api_key": "sk-test"}"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.api_key, "sk-test");
    // Default base_url and model should be set
    assert!(!cfg.base_url.is_empty());
    assert!(!cfg.model.is_empty());
}

#[test]
fn test_config_extra_body() {
    let json = r#"{
        "api_key": "sk-test",
        "extra_body": {"temperature": 0.7, "top_p": 0.9}
    }"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert!(cfg.extra_body.is_some());
    let extra = cfg.extra_body.unwrap();
    assert_eq!(extra.get("temperature").unwrap().as_f64().unwrap(), 0.7);
}

#[test]
fn test_config_serialization_roundtrip() {
    let cfg = Config {
        api_key: "sk-roundtrip".into(),
        base_url: "https://api.example.com".into(),
        model: "gpt-4".into(),
        streaming: Some(true),
        thinking_effort: Some("max".into()),
        extra_body: None,
    };
    let json = serde_json::to_string(&cfg).unwrap();
    let parsed: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.api_key, cfg.api_key);
    assert_eq!(parsed.thinking_effort, Some("max".into()));
}
