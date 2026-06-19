//! Web fetch tool tests.

use crate::common;
use motif::*;
use motif_tools;
use std::sync::Arc;

#[test]
fn test_web_fetch_invalid_url() {
    let (_, tool) = motif_tools::web_fetch::register().into_parts();
    let result = common::call_tool(&tool, r#"{"url":"not-a-url"}"#);
    assert!(result.contains("only http and https"), "Got: {}", result);
}

#[test]
fn test_web_fetch_empty_url() {
    let (_, tool) = motif_tools::web_fetch::register().into_parts();
    let result = common::call_tool(&tool, r#"{"url":""}"#);
    assert!(result.contains("required"), "Got: {}", result);
}
