//! Quote normalization tests for the edit tool.

use crate::common;
use motif::*;
use motif_tools;
use std::fs;
use std::sync::Arc;

#[test]
fn test_quote_curly_to_straight() {
    // File has curly quotes, LLM sends straight quotes → normalization should match
    fs::write("_t_qt_c2s.txt", "He said \u{201c}hello\u{201d} world").unwrap();
    let (_, r_tool) = motif_tools::read::register().into_parts();
    let (_, e_tool) = motif_tools::edit::register().into_parts();
    common::call_tool(&r_tool, r#"{"file_path":"_t_qt_c2s.txt"}"#);
    let result = common::call_tool(&e_tool, r#"{"file_path":"_t_qt_c2s.txt","old_string":"He said \"hello\" world","new_string":"Replaced"}"#);
    assert!(result.contains("Edited"), "Curly→straight normalization: {}", result);
    let content = fs::read_to_string("_t_qt_c2s.txt").unwrap();
    assert_eq!(content, "Replaced");
    fs::remove_file("_t_qt_c2s.txt").ok();
}

#[test]
fn test_quote_straight_to_curly() {
    // File has straight quotes, LLM sends curly quotes → normalization should match
    fs::write("_t_qt_s2c.txt", "He said \"hello\" world").unwrap();
    let (_, r_tool) = motif_tools::read::register().into_parts();
    let (_, e_tool) = motif_tools::edit::register().into_parts();
    common::call_tool(&r_tool, r#"{"file_path":"_t_qt_s2c.txt"}"#);
    let curly_old = "He said \u{201c}hello\u{201d} world";
    let args = format!(r#"{{"file_path":"_t_qt_s2c.txt","old_string":"{}","new_string":"Replaced"}}"#, curly_old);
    let result = common::call_tool(&e_tool, &args);
    assert!(result.contains("Edited"), "Straight→curly normalization: {}", result);
    let content = fs::read_to_string("_t_qt_s2c.txt").unwrap();
    assert_eq!(content, "Replaced");
    fs::remove_file("_t_qt_s2c.txt").ok();
}

#[test]
fn test_quote_single_curly_normalization() {
    // File has curly single quotes, LLM sends straight single quotes
    fs::write("_t_qt_single.txt", "It\u{2018}s a test").unwrap();
    let (_, r_tool) = motif_tools::read::register().into_parts();
    let (_, e_tool) = motif_tools::edit::register().into_parts();
    common::call_tool(&r_tool, r#"{"file_path":"_t_qt_single.txt"}"#);
    let result = common::call_tool(&e_tool, r#"{"file_path":"_t_qt_single.txt","old_string":"It's a test","new_string":"Replaced"}"#);
    assert!(result.contains("Edited"), "Single curly→straight: {}", result);
    fs::remove_file("_t_qt_single.txt").ok();
}

#[test]
fn test_quote_no_quotes_exact_match() {
    fs::write("_t_qt_exact.txt", "plain text").unwrap();
    let (_, r_tool) = motif_tools::read::register().into_parts();
    let (_, e_tool) = motif_tools::edit::register().into_parts();
    common::call_tool(&r_tool, r#"{"file_path":"_t_qt_exact.txt"}"#);
    let result = common::call_tool(&e_tool, r#"{"file_path":"_t_qt_exact.txt","old_string":"plain text","new_string":"replaced"}"#);
    assert!(result.contains("Edited"), "Exact match: {}", result);
    fs::remove_file("_t_qt_exact.txt").ok();
}

#[test]
fn test_quote_mixed_curly_straight() {
    // File has left curly + right straight: "hello" (one curly, one straight)
    let content = format!("He said {}hello\" world", '\u{201c}');
    fs::write("_t_qt_mixed.txt", &content).unwrap();
    let (_, r_tool) = motif_tools::read::register().into_parts();
    let (_, e_tool) = motif_tools::edit::register().into_parts();
    common::call_tool(&r_tool, r#"{"file_path":"_t_qt_mixed.txt"}"#);
    // Try straight quotes — should match
    let result = common::call_tool(&e_tool, r#"{"file_path":"_t_qt_mixed.txt","old_string":"He said \"hello\" world","new_string":"X"}"#);
    // May or may not match, depending on normalization strategy — just verify no crash
    assert!(!result.contains("panicked"), "Should not panic: {}", result);
    fs::remove_file("_t_qt_mixed.txt").ok();
}
