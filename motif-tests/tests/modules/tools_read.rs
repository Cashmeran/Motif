//! Read tool tests.

use crate::common;
use motif_tools;
use std::fs;

#[test]
fn test_read_basic() {
    fs::write("_t_rd.txt", "line1\nline2\nline3\n").unwrap();
    let (_, tool) = motif_tools::read::register().into_parts();
    let result = common::call_tool(&tool, r#"{"file_path":"_t_rd.txt"}"#);
    assert!(result.contains("line1"));
    assert!(result.contains("line3"));
    fs::remove_file("_t_rd.txt").ok();
}

#[test]
fn test_read_offset_limit() {
    fs::write("_t_rd_off.txt", "a\nb\nc\nd\ne\n").unwrap();
    let (_, tool) = motif_tools::read::register().into_parts();
    let result = common::call_tool(
        &tool,
        r#"{"file_path":"_t_rd_off.txt","offset":2,"limit":2}"#,
    );
    assert!(result.contains("c"), "Expected line c: {}", result);
    assert!(result.contains("d"), "Expected line d: {}", result);
    fs::remove_file("_t_rd_off.txt").ok();
}

#[test]
fn test_read_missing_file() {
    let (_, tool) = motif_tools::read::register().into_parts();
    let result = common::call_tool(&tool, r#"{"file_path":"nonexistent_file.txt"}"#);
    assert!(result.contains("Cannot access") || result.contains("Error"));
}

#[test]
fn test_read_protected_file() {
    let (_, tool) = motif_tools::read::register().into_parts();
    let result = common::call_tool(&tool, r#"{"file_path":".env"}"#);
    assert!(!result.contains("API_KEY") && !result.contains("SECRET"));
}
