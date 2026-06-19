use crate::common;
use motif::*;
use motif_tools;
use std::sync::Arc;

#[test]
fn test_web_fetch_file_url() {
    let (_, tool) = motif_tools::web_fetch::register().into_parts();
    let result = common::call_tool(&tool, r#"{"url":"file:///etc/passwd"}"#);
    assert!(result.contains("only http and https"), "Got: {}", result);
}
