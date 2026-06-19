use motif::*;

#[test] fn test_error_api_format() {
    let e = Error::ApiError { status: 429, body: "rate limited".into() };
    assert!(e.to_string().contains("429"));
    assert!(e.to_string().contains("rate limited"));
}

#[test] fn test_error_tool_not_found_format() {
    let e = Error::ToolNotFound { name: "my_tool".into(), available: vec!["other".into()] };
    assert!(e.to_string().contains("my_tool"));
    assert!(e.to_string().contains("other"));
}

#[test] fn test_error_clone_preserves_message() {
    let e = Error::ApiError { status: 500, body: "boom".into() };
    assert_eq!(e.to_string(), e.clone().to_string());
}

#[test] fn test_error_custom() {
    assert_eq!(Error::Custom("msg".into()).to_string(), "msg");
}
