//! Bash injection security tests — attempt to bypass metachar detection.

use crate::common;
use motif::*;
use motif_tools;
use std::sync::Arc;

#[test]
fn test_bash_dollar_brace_blocked() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo ${IFS}","timeout_ms":5000}"#);
    assert!(result.contains("not allowed"), "Should block ${{IFS}}: {}", result);
}

#[test]
fn test_bash_dollar_at_blocked() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo $@","timeout_ms":5000}"#);
    assert!(result.contains("not allowed"), "Should block $@: {}", result);
}

#[test]
fn test_bash_dollar_star_blocked() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo $*","timeout_ms":5000}"#);
    assert!(result.contains("not allowed"), "Should block $*: {}", result);
}

#[test]
fn test_bash_dollar_question_blocked() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo $?","timeout_ms":5000}"#);
    assert!(result.contains("not allowed"), "Should block $?: {}", result);
}

#[test]
fn test_bash_dollar_hash_blocked() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo $#","timeout_ms":5000}"#);
    assert!(result.contains("not allowed"), "Should block $#: {}", result);
}

#[test]
fn test_bash_safe_single_quote_dollar() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"awk '{print $1}' file.txt","timeout_ms":5000}"#);
    // $1 inside single quotes is safe (no expansion)
    assert!(!result.contains("not allowed"), "Should allow $1 in single quotes: {}", result);
}

#[test]
fn test_bash_subshell_dollar_nested_blocked() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo $(echo $(whoami))","timeout_ms":5000}"#);
    assert!(result.contains("not allowed"), "Should block nested $(): {}", result);
}
