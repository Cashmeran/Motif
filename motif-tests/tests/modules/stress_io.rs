//! Large I/O stress tests.

use crate::common;
use motif::*;
use motif_tools;
use std::fs;
use std::sync::Arc;

#[test]
fn test_write_1mb_file() {
    let content = "x".repeat(1_000_000); // ~1MB
    let (_, tool) = motif_tools::write::register().into_parts();
    let args = format!(r#"{{"file_path":"_t_big.txt","content":"{}"}}"#, content);
    let result = common::call_tool(&tool, &args);
    assert!(result.contains("Wrote") || result.contains("bytes"), "Got: {}", result);
    assert!(fs::metadata("_t_big.txt").map(|m| m.len()).unwrap_or(0) > 900_000);
    fs::remove_file("_t_big.txt").ok();
}

#[test]
fn test_read_256kb_boundary() {
    // Create a file just under 256KB
    let line = "a".repeat(100) + "\n";
    let content = line.repeat(2500); // ~250KB
    fs::write("_t_256k.txt", &content).unwrap();
    let (_, tool) = motif_tools::read::register().into_parts();
    let result = common::call_tool(&tool, r#"{"file_path":"_t_256k.txt"}"#);
    assert!(!result.is_empty(), "Should read large file");
    fs::remove_file("_t_256k.txt").ok();
}

#[test]
fn test_read_offset_beyond_length() {
    fs::write("_t_short.txt", "short").unwrap();
    let (_, tool) = motif_tools::read::register().into_parts();
    let result = common::call_tool(&tool, r#"{"file_path":"_t_short.txt","offset":100}"#);
    // Should not panic, should show empty or range message
    assert!(!result.contains("panicked"));
    fs::remove_file("_t_short.txt").ok();
}

#[test]
fn test_edit_old_string_at_limit() {
    let long = "x".repeat(9_999);
    fs::write("_t_longold.txt", &long).unwrap();
    let (_, r_tool) = motif_tools::read::register().into_parts();
    let (_, e_tool) = motif_tools::edit::register().into_parts();
    common::call_tool(&r_tool, r#"{"file_path":"_t_longold.txt"}"#);
    let args = format!(r#"{{"file_path":"_t_longold.txt","old_string":"{}","new_string":"done"}}"#, long);
    let result = common::call_tool(&e_tool, &args);
    assert!(result.contains("Edited"), "9,999 char old_string: {}", result);
    fs::remove_file("_t_longold.txt").ok();
}

#[test]
fn test_write_very_long_path() {
    let long_name = "a".repeat(200);
    let path = format!("_t_deep_{}", long_name);
    let (_, tool) = motif_tools::write::register().into_parts();
    let args = format!(r#"{{"file_path":"{}","content":"ok"}}"#, path);
    let result = common::call_tool(&tool, &args);
    assert!(!result.contains("panicked"));
    // Cleanup: try to remove if it was created
    fs::remove_file(&path).ok();
}
