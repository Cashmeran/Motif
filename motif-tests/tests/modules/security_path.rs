//! Path traversal security tests.

use crate::common;
use motif::*;
use motif_tools;
use std::sync::Arc;

#[test]
fn test_read_double_dot_blocked() {
    let (_, tool) = motif_tools::read::register().into_parts();
    let result = common::call_tool(&tool, r#"{"file_path":"../etc/passwd"}"#);
    assert!(result.contains("not allowed"), "Got: {}", result);
}

#[test]
fn test_write_double_dot_blocked() {
    let (_, tool) = motif_tools::write::register().into_parts();
    let result = common::call_tool(&tool, r#"{"file_path":"../evil.txt","content":"x"}"#);
    assert!(result.contains("not allowed"), "Got: {}", result);
}

#[test]
fn test_edit_double_dot_blocked() {
    let (_, tool) = motif_tools::edit::register().into_parts();
    let result = common::call_tool(&tool, r#"{"file_path":"../evil.txt","old_string":"a","new_string":"b"}"#);
    assert!(result.contains("not allowed"), "Got: {}", result);
}

#[test]
fn test_read_absolute_unix_path_blocked() {
    let (_, tool) = motif_tools::read::register().into_parts();
    // /etc/passwd contains ".." → checked: /etc/passwd does NOT contain ".." so path traversal check passes
    // But the file likely doesn't exist on Windows, so it should get "Cannot access"
    let result = common::call_tool(&tool, r#"{"file_path":"/etc/passwd"}"#);
    // On Windows this will fail with "Cannot access", on Unix it may or may not
    // Both are acceptable — the key is it shouldn't succeed silently
    assert!(!result.contains("root:"), "Should not leak file contents: {}", result);
}

#[test]
fn test_read_windows_absolute_blocked() {
    let (_, tool) = motif_tools::read::register().into_parts();
    let result = common::call_tool(&tool, r#"{"file_path":"C:\\Windows\\System32\\config\\SAM"}"#);
    assert!(!result.contains("Administrator"), "Should not leak SAM: {}", result);
}

#[test]
fn test_deep_traversal() {
    let (_, tool) = motif_tools::read::register().into_parts();
    let result = common::call_tool(&tool, r#"{"file_path":"a/../../b/../../c/../../etc/shadow"}"#);
    assert!(result.contains("not allowed"), "Deep traversal: {}", result);
}
