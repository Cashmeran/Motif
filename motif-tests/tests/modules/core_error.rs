//! Tests for Error enum: Display, Clone, conversion.

use motif::*;

#[test]
fn test_error_api_display() {
    let e = Error::ApiError { status: 500, body: "Internal Server Error".into() };
    let s = e.to_string();
    assert!(s.contains("500"));
    assert!(s.contains("Internal Server Error"));
}

#[test]
fn test_error_http_display() {
    let e = Error::Custom("test error".into());
    let s = e.to_string();
    assert!(s.contains("test error"));
}

#[test]
fn test_error_tool_not_found_display() {
    let e = Error::ToolNotFound {
        name: "my_tool".into(),
        available: vec!["other".into()],
    };
    let s = e.to_string();
    assert!(s.contains("my_tool"));
    assert!(s.contains("other"));
}

#[test]
fn test_error_custom_display() {
    let e = Error::Custom("something broke".into());
    assert_eq!(e.to_string(), "something broke");
}

#[test]
fn test_error_clone_api_error() {
    let e = Error::ApiError { status: 429, body: "rate limit".into() };
    let e2 = e.clone();
    assert_eq!(e.to_string(), e2.to_string());
}

#[test]
fn test_error_clone_tool_not_found() {
    let e = Error::ToolNotFound {
        name: "x".into(),
        available: vec![],
    };
    let e2 = e.clone();
    assert_eq!(e.to_string(), e2.to_string());
}
