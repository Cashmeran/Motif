//! Read-write state enforcement tests: read-before-edit/write.

use crate::common;
use motif::*;
use motif_tools;
use std::fs;
use std::sync::Arc;

#[test]
fn test_read_then_edit_allowed() {
    fs::write("_t_rs_ok.txt", "original").unwrap();
    let (_, r_tool) = motif_tools::read::register().into_parts();
    let (_, e_tool) = motif_tools::edit::register().into_parts();
    // Read first → should allow edit
    common::call_tool(&r_tool, r#"{"file_path":"_t_rs_ok.txt"}"#);
    let result = common::call_tool(&e_tool, r#"{"file_path":"_t_rs_ok.txt","old_string":"original","new_string":"modified"}"#);
    assert!(result.contains("Edited"), "Read-then-edit should work: {}", result);
    fs::remove_file("_t_rs_ok.txt").ok();
}

#[test]
fn test_edit_without_read_blocked() {
    fs::write("_t_rs_no.txt", "secret").unwrap();
    let (_, e_tool) = motif_tools::edit::register().into_parts();
    let result = common::call_tool(&e_tool, r#"{"file_path":"_t_rs_no.txt","old_string":"secret","new_string":"x"}"#);
    assert!(result.contains("has not been read"), "Should block: {}", result);
    fs::remove_file("_t_rs_no.txt").ok();
}

#[test]
fn test_write_existing_without_read_blocked() {
    fs::write("_t_rs_wr.txt", "old data").unwrap();
    let (_, w_tool) = motif_tools::write::register().into_parts();
    let result = common::call_tool(&w_tool, r#"{"file_path":"_t_rs_wr.txt","content":"new data"}"#);
    assert!(result.contains("has not been read"), "Should block write without read: {}", result);
    fs::remove_file("_t_rs_wr.txt").ok();
}

#[test]
fn test_write_new_file_allowed() {
    let (_, w_tool) = motif_tools::write::register().into_parts();
    // New file — no read needed
    let result = common::call_tool(&w_tool, r#"{"file_path":"_t_rs_new.txt","content":"fresh"}"#);
    assert!(result.contains("Wrote") || result.contains("bytes"), "New file should work: {}", result);
    fs::remove_file("_t_rs_new.txt").ok();
}
