//! Write tool tests.

use crate::common;
use motif::*;
use motif_tools;
use std::fs;
use std::path::Path;
use std::sync::Arc;

#[test]
fn test_write_and_read() {
    let (_, tool) = motif_tools::write::register().into_parts();
    let result = common::call_tool(
        &tool,
        r#"{"file_path":"_t_wr.txt","content":"hello world"}"#,
    );
    assert!(result.contains("Wrote") || result.contains("bytes"));
    let content = fs::read_to_string("_t_wr.txt").unwrap();
    assert_eq!(content, "hello world");
    fs::remove_file("_t_wr.txt").ok();
}

#[test]
fn test_write_empty_file() {
    let (_, tool) = motif_tools::write::register().into_parts();
    let result = common::call_tool(&tool, r#"{"file_path":"_t_wr_empty.txt","content":""}"#);
    assert!(result.contains("empty") || result.contains("Created"));
    assert!(Path::new("_t_wr_empty.txt").exists());
    fs::remove_file("_t_wr_empty.txt").ok();
}

#[test]
fn test_write_parent_directory_creation() {
    let (_, tool) = motif_tools::write::register().into_parts();
    common::call_tool(
        &tool,
        r#"{"file_path":"_t_deep/deep2/file.txt","content":"nested"}"#,
    );
    assert!(Path::new("_t_deep/deep2/file.txt").exists());
    let content = fs::read_to_string("_t_deep/deep2/file.txt").unwrap();
    assert_eq!(content, "nested");
    fs::remove_dir_all("_t_deep").ok();
}
