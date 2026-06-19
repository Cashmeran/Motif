use crate::common;
use motif::*;
use motif_tools;
use std::sync::Arc;

#[test]
fn test_read_path_traversal() {
    let (_, tool) = motif_tools::read::register().into_parts();
    let result = common::call_tool(&tool, r#"{"file_path":"../etc/passwd"}"#);
    assert!(result.contains("not allowed"), "Got: {}", result);
}

#[test]
fn test_write_path_traversal() {
    let (_, tool) = motif_tools::write::register().into_parts();
    let result = common::call_tool(&tool, r#"{"file_path":"../evil.txt","content":"x"}"#);
    assert!(result.contains("not allowed"), "Got: {}", result);
}

#[test]
fn test_edit_path_traversal() {
    let (_, tool) = motif_tools::edit::register().into_parts();
    let result = common::call_tool(
        &tool,
        r#"{"file_path":"../evil.txt","old_string":"a","new_string":"b"}"#,
    );
    assert!(result.contains("not allowed"), "Got: {}", result);
}
