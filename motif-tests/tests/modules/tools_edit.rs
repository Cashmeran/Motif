//! Edit tool tests: replacement, dedup, quote normalization, read-before-edit.

use crate::common;
use motif::*;
use motif_tools;
use std::fs;
use std::sync::Arc;

#[test]
fn test_edit_basic_replace() {
    fs::write("_t_ed.txt", "Hello World").unwrap();
    let (_, r_tool) = motif_tools::read::register().into_parts();
    let (_, e_tool) = motif_tools::edit::register().into_parts();
    common::call_tool(&r_tool, r#"{"file_path":"_t_ed.txt"}"#);
    let result = common::call_tool(&e_tool, r#"{"file_path":"_t_ed.txt","old_string":"World","new_string":"Rust"}"#);
    assert!(result.contains("Edited"), "Got: {}", result);
    let content = fs::read_to_string("_t_ed.txt").unwrap();
    assert_eq!(content, "Hello Rust");
    fs::remove_file("_t_ed.txt").ok();
}

#[test]
fn test_edit_duplicate_old_string() {
    fs::write("_t_ed_dup.txt", "A B A").unwrap();
    let (_, r_tool) = motif_tools::read::register().into_parts();
    let (_, e_tool) = motif_tools::edit::register().into_parts();
    common::call_tool(&r_tool, r#"{"file_path":"_t_ed_dup.txt"}"#);
    let result = common::call_tool(&e_tool, r#"{"file_path":"_t_ed_dup.txt","old_string":"A","new_string":"X"}"#);
    assert!(result.contains("appears 2 times"), "Got: {}", result);
    fs::remove_file("_t_ed_dup.txt").ok();
}

#[test]
fn test_edit_replace_all() {
    fs::write("_t_ed_all.txt", "A B A").unwrap();
    let (_, r_tool) = motif_tools::read::register().into_parts();
    let (_, e_tool) = motif_tools::edit::register().into_parts();
    common::call_tool(&r_tool, r#"{"file_path":"_t_ed_all.txt"}"#);
    let result = common::call_tool(&e_tool, r#"{"file_path":"_t_ed_all.txt","old_string":"A","new_string":"X","replace_all":true}"#);
    assert!(result.contains("Replaced 2"), "Got: {}", result);
    fs::remove_file("_t_ed_all.txt").ok();
}

#[test]
fn test_edit_not_found() {
    fs::write("_t_ed_nf.txt", "hello").unwrap();
    let (_, r_tool) = motif_tools::read::register().into_parts();
    let (_, e_tool) = motif_tools::edit::register().into_parts();
    common::call_tool(&r_tool, r#"{"file_path":"_t_ed_nf.txt"}"#);
    let result = common::call_tool(&e_tool, r#"{"file_path":"_t_ed_nf.txt","old_string":"world","new_string":"x"}"#);
    assert!(result.contains("not found"), "Got: {}", result);
    fs::remove_file("_t_ed_nf.txt").ok();
}

#[test]
fn test_edit_idempotent() {
    fs::write("_t_ed_same.txt", "same").unwrap();
    let (_, r_tool) = motif_tools::read::register().into_parts();
    let (_, e_tool) = motif_tools::edit::register().into_parts();
    common::call_tool(&r_tool, r#"{"file_path":"_t_ed_same.txt"}"#);
    let result = common::call_tool(&e_tool, r#"{"file_path":"_t_ed_same.txt","old_string":"same","new_string":"same"}"#);
    assert!(result.contains("identical"), "Got: {}", result);
    fs::remove_file("_t_ed_same.txt").ok();
}

#[test]
fn test_edit_without_read_blocked() {
    fs::write("_t_ed_noread.txt", "content").unwrap();
    let (_, e_tool) = motif_tools::edit::register().into_parts();
    let result = common::call_tool(&e_tool, r#"{"file_path":"_t_ed_noread.txt","old_string":"content","new_string":"x"}"#);
    assert!(result.contains("has not been read"), "Should block edit without prior read: {}", result);
    fs::remove_file("_t_ed_noread.txt").ok();
}
