use crate::common;
use motif::*;
use motif_tools;
use std::sync::Arc;

#[test]
fn test_bash_dollar_brace() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo ${IFS}","timeout_ms":5000}"#);
    assert!(result.contains("not allowed"), "Got: {}", result);
}

#[test]
fn test_bash_dollar_at() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo $@","timeout_ms":5000}"#);
    assert!(result.contains("not allowed"), "Got: {}", result);
}

#[test]
fn test_bash_dollar_star() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo $*","timeout_ms":5000}"#);
    assert!(result.contains("not allowed"), "Got: {}", result);
}

#[test]
fn test_bash_dollar_question() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo $?","timeout_ms":5000}"#);
    assert!(result.contains("not allowed"), "Got: {}", result);
}

#[test]
fn test_bash_single_quote_safe() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"awk '{print $1}' /dev/null","timeout_ms":5000}"#);
    assert!(!result.contains("not allowed"), "Got: {}", result);
}
