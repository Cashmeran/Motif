use motif_cli::config::Config;

#[test] fn test_config_parse() {
    let json = r#"{"api_key":"sk-test","base_url":"https://x.com","model":"m"}"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.api_key, "sk-test");
}
#[test] fn test_config_defaults() {
    let json = r#"{"api_key":"minimal-key"}"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.api_key, "minimal-key");
}
