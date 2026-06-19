use motif::*;

#[test]
fn test_error_api_display() {
    let err = Error::ApiError {
        status: 500,
        body: "err".to_string(),
    };
    let s = format!("{}", err);
    assert!(s.contains("500"));
}

#[test]
fn test_error_tool_not_found() {
    let err = Error::ToolNotFound {
        name: "x".to_string(),
        available: vec![],
    };
    let s = format!("{}", err);
    assert!(s.contains("x"));
}

#[test]
fn test_error_clone() {
    let err = Error::ApiError {
        status: 400,
        body: "bad".to_string(),
    };
    let cloned = err.clone();
    assert_eq!(format!("{}", err), format!("{}", cloned));
}

#[test]
fn test_error_custom() {
    let err = Error::Custom("msg".to_string());
    let s = format!("{}", err);
    assert!(s.contains("msg"));
}
