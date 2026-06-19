//! Bash tool tests: echo, timeout, destructive, metachar detection.

use crate::common;
use motif_tools;

#[test]
fn test_bash_echo() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo hello","timeout_ms":5000}"#);
    if cfg!(target_os = "windows") {
        assert!(
            result.contains("hello") || result.contains("echo"),
            "Got: {}",
            result
        );
    } else {
        assert!(result.contains("hello"), "Got: {}", result);
    }
}

#[test]
fn test_bash_error_command() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(
        &tool,
        r#"{"command":"nonexistent_command_xyz 2>&1","timeout_ms":5000}"#,
    );
    assert!(!result.contains("panicked"), "Should not panic");
}

#[test]
fn test_bash_timeout() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let cmd = if cfg!(target_os = "windows") {
        "timeout /t 10"
    } else {
        "sleep 10"
    };
    let result = common::call_tool(
        &tool,
        &format!(r#"{{"command":"{}","timeout_ms":500}}"#, cmd),
    );
    assert!(
        result.contains("timed out") || result.contains("timeout"),
        "Got: {}",
        result
    );
}

#[test]
fn test_bash_destructive_blocked() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"rm -rf /","timeout_ms":5000}"#);
    assert!(
        result.contains("Destructive") || result.contains("detected"),
        "Got: {}",
        result
    );
}

#[test]
fn test_bash_unquoted_dollar_var_blocked() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo $HOME","timeout_ms":5000}"#);
    assert!(
        result.contains("not allowed"),
        "Should block unquoted $VAR: {}",
        result
    );
}

#[test]
fn test_bash_unquoted_dollar_subshell_blocked() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo $(whoami)","timeout_ms":5000}"#);
    assert!(
        result.contains("not allowed"),
        "Should block $(): {}",
        result
    );
}

#[test]
fn test_bash_backtick_blocked() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo `whoami`","timeout_ms":5000}"#);
    assert!(
        result.contains("not allowed"),
        "Should block backtick: {}",
        result
    );
}

#[test]
fn test_bash_unquoted_glob_blocked() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"ls *.rs","timeout_ms":5000}"#);
    assert!(
        result.contains("not allowed"),
        "Should block unquoted glob: {}",
        result
    );
}

#[test]
fn test_bash_quoted_dollar_blocked() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo \"$HOME\"","timeout_ms":5000}"#);
    assert!(
        result.contains("not allowed"),
        "Should block $ in double quotes: {}",
        result
    );
}

#[test]
fn test_bash_escaped_dollar_allowed() {
    let (_, tool) = motif_tools::bash::register().into_parts();
    let result = common::call_tool(&tool, r#"{"command":"echo \\$var","timeout_ms":5000}"#);
    assert!(
        !result.contains("not allowed"),
        "Should allow escaped $: {}",
        result
    );
}
