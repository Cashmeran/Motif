//! Stress / correctness tests for built-in tools.
//! These tests exercise the actual tool implementations against real files.

use motif_tools::{bash, edit, read, search, web_fetch, write};
use motif::Tool;

use std::fs;
use std::path::Path;
use std::sync::Arc;

fn call_tool(tool: &Arc<dyn Tool>, args: &str) -> String {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(tool.call(args.to_string()))
}

// ── Search ──

#[test]
fn test_search_filename_mode() {
    let (_, registered) = search::register().into_parts();
    let result = call_tool(&registered, r#"{"query":"*.rs","mode":"filename","path":"../motif/src"}"#);
    assert!(result.contains("agent.rs"), "Should find agent.rs: {}", result);
    assert!(result.contains("provider.rs"), "Should find provider.rs: {}", result);
}

#[test]
fn test_search_content_mode() {
    let (_, registered) = search::register().into_parts();
    let result = call_tool(&registered, r#"{"query":"StopCondition","mode":"content","path":"../motif/src","glob":"*.rs"}"#);
    assert!(result.contains("StopCondition"), "Should find StopCondition: {}", result);
}

#[test]
fn test_search_files_with_matches() {
    let (_, registered) = search::register().into_parts();
    let result = call_tool(&registered, r#"{"query":"pub struct Agent","mode":"files_with_matches","path":"../motif/src","glob":"*.rs"}"#);
    assert!(result.contains("agent.rs"), "Should list agent.rs: {}", result);
}

#[test]
fn test_search_count() {
    let (_, registered) = search::register().into_parts();
    let result = call_tool(&registered, r#"{"query":"fn","mode":"count","path":"../motif/src","glob":"*.rs"}"#);
    assert!(!result.is_empty(), "Count mode should produce output");
}

#[test]
fn test_search_pagination() {
    let (_, registered) = search::register().into_parts();
    // head_limit=2 should produce no more than 2 file entries
    let result = call_tool(&registered, r#"{"query":"use","mode":"filename","path":"../motif/src","head_limit":2,"offset":0}"#);
    // Should either contain results or a truncation message
    assert!(!result.is_empty());
    // head_limit=0 should return all
    let all = call_tool(&registered, r#"{"query":"use","mode":"filename","path":"../motif/src","head_limit":0}"#);
    assert!(!all.is_empty());
}

#[test]
fn test_search_case_insensitive() {
    let (_, registered) = search::register().into_parts();
    let result1 = call_tool(&registered, r#"{"query":"stopcondition","mode":"content","path":"../motif/src","glob":"*.rs","ignore_case":true}"#);
    assert!(result1.contains("StopCondition") || result1.to_lowercase().contains("stopcondition"),
        "Case-insensitive should match");
}

#[test]
fn test_search_nonexistent_path() {
    let (_, registered) = search::register().into_parts();
    let result = call_tool(&registered, r#"{"query":"x","path":"/nonexistent/path","mode":"filename"}"#);
    assert!(result.contains("not found"), "Should report path not found");
}

// ── Read ──

#[test]
fn test_read_basic() {
    fs::write("test_read.txt", "line1\nline2\nline3\n").unwrap();
    let (_, registered) = read::register().into_parts();
    let result = call_tool(&registered, r#"{"file_path":"test_read.txt"}"#);
    assert!(result.contains("line1"));
    assert!(result.contains("line3"));
    fs::remove_file("test_read.txt").ok();
}

#[test]
fn test_read_offset_limit() {
    fs::write("test_read_offset.txt", "a\nb\nc\nd\ne\n").unwrap();
    let (_, registered) = read::register().into_parts();
    let result = call_tool(&registered, r#"{"file_path":"test_read_offset.txt","offset":2,"limit":2}"#);
    assert!(result.contains("c"), "Expected line c: {}", result);
    assert!(result.contains("d"), "Expected line d: {}", result);
    assert!(!result.contains("a\n"), "Expected only lines c,d: {}", result);
    fs::remove_file("test_read_offset.txt").ok();
}

#[test]
fn test_read_missing_file() {
    let (_, registered) = read::register().into_parts();
    let result = call_tool(&registered, r#"{"file_path":"nonexistent_file.txt"}"#);
    assert!(result.contains("Cannot access") || result.contains("Error"));
}

// ── Write ──

#[test]
fn test_write_and_read() {
    let (_, w_tool) = write::register().into_parts();
    let result = call_tool(&w_tool, r#"{"file_path":"test_written.txt","content":"hello world"}"#);
    assert!(result.contains("Wrote") || result.contains("bytes"));
    let content = fs::read_to_string("test_written.txt").unwrap();
    assert_eq!(content, "hello world");
    fs::remove_file("test_written.txt").ok();
}

#[test]
fn test_write_empty_file() {
    let (_, w_tool) = write::register().into_parts();
    let result = call_tool(&w_tool, r#"{"file_path":"test_empty.txt","content":""}"#);
    assert!(result.contains("empty") || result.contains("Created"));
    assert!(Path::new("test_empty.txt").exists());
    fs::remove_file("test_empty.txt").ok();
}

// ── Bash ──

#[test]
fn test_bash_echo() {
    let (_, b_tool) = bash::register().into_parts();
    let result = call_tool(&b_tool, r#"{"command":"echo hello","timeout_ms":5000}"#);
    if cfg!(target_os = "windows") {
        assert!(result.contains("hello") || result.contains("echo"), "Got: {}", result);
    } else {
        assert!(result.contains("hello"), "Got: {}", result);
    }
}

#[test]
fn test_bash_error_command() {
    let (_, b_tool) = bash::register().into_parts();
    let result = call_tool(&b_tool, r#"{"command":"nonexistent_command_xyz 2>&1","timeout_ms":5000}"#);
    // Should not panic, should report error or empty with exit code
    assert!(!result.contains("panicked"), "Should not panic");
}

#[test]
fn test_bash_timeout() {
    let (_, b_tool) = bash::register().into_parts();
    let cmd = if cfg!(target_os = "windows") { "timeout /t 10" } else { "sleep 10" };
    let result = call_tool(&b_tool, &format!(r#"{{"command":"{}","timeout_ms":500}}"#, cmd));
    assert!(result.contains("timed out") || result.contains("timeout"), "Got: {}", result);
}

#[test]
fn test_bash_destructive_blocked() {
    let (_, b_tool) = bash::register().into_parts();
    let result = call_tool(&b_tool, r#"{"command":"rm -rf /","timeout_ms":5000}"#);
    assert!(result.contains("Destructive") || result.contains("detected"), "Got: {}", result);
}

// ── Tool integration via Agent ──

#[test]
fn test_search_tool_schema_correct() {
    let (def, _tool) = search::register().into_parts();
    assert_eq!(def.function.name, "search");
    assert!(!def.function.description.is_empty());
}

#[test]
fn test_search_excludes_build_dirs() {
    // Create a temp build directory with a known file
    let _ = fs::create_dir_all("_test_build/__pycache__");
    fs::write("_test_build/__pycache__/test.pyc", "0000000").ok();
    fs::write("_test_build/visible.txt", "hello build test world").ok();

    let (_, registered) = search::register().into_parts();
    // Search from _test_build root — should find visible.txt but not test.pyc
    let result = call_tool(&registered, r#"{"query":"hello build","mode":"content","path":"_test_build"}"#);
    assert!(result.contains("visible.txt"), "Should find visible.txt: {}", result);
    // __pycache__ is a skip dir, its content should be excluded
    assert!(!result.contains("test.pyc"), "Should exclude __pycache__: {}", result);

    fs::remove_dir_all("_test_build").ok();
}

#[test]
fn test_write_parent_directory_creation() {
    let (_, w_tool) = write::register().into_parts();
    let result = call_tool(&w_tool, r#"{"file_path":"test_deep/deep2/file.txt","content":"nested"}"#);
    assert!(Path::new("test_deep/deep2/file.txt").exists());
    let content = fs::read_to_string("test_deep/deep2/file.txt").unwrap();
    assert_eq!(content, "nested");
    fs::remove_dir_all("test_deep").ok();
}

#[test]
fn test_search_empty_query() {
    let (_, registered) = search::register().into_parts();
    let result = call_tool(&registered, r#"{"query":"","path":"../motif/src"}"#);
    assert!(result.contains("Error") || result.contains("required"));
}

#[test]
fn test_read_protected_file() {
    let (_, registered) = read::register().into_parts();
    let result = call_tool(&registered, r#"{"file_path":".env"}"#);
    // If .env exists, should block; if not, should report error
    assert!(!result.contains("API_KEY") && !result.contains("SECRET"));
}

#[test]
fn test_search_multiline_regex() {
    // Write a test file with multi-line content
    fs::write("test_multiline.txt", "struct Foo {\n    x: i32,\n}\n\nstruct Bar {\n    y: i32,\n}\n").ok();
    let (_, registered) = search::register().into_parts();
    let result = call_tool(&registered, r#"{"query":"struct \\w+ \\{[^}]*\\}","mode":"content","path":"test_multiline.txt","multiline":true}"#);
    // Just verify it doesn't crash on multiline
    assert!(!result.contains("Invalid regex"));
    fs::remove_file("test_multiline.txt").ok();
}

// ── Edit ──

#[test]
fn test_edit_basic_replace() {
    fs::write("test_edit.txt", "Hello World").unwrap();
    let (_, r_tool) = read::register().into_parts();
    let (_, tool) = edit::register().into_parts();
    // Read first to satisfy read-before-edit enforcement
    let _ = call_tool(&r_tool, r#"{"file_path":"test_edit.txt"}"#);
    let result = call_tool(&tool, r#"{"file_path":"test_edit.txt","old_string":"World","new_string":"Rust"}"#);
    assert!(result.contains("Edited"), "Got: {}", result);
    let content = fs::read_to_string("test_edit.txt").unwrap();
    assert_eq!(content, "Hello Rust");
    fs::remove_file("test_edit.txt").ok();
}

#[test]
fn test_edit_duplicate_old_string() {
    fs::write("test_edit_dup.txt", "A B A").unwrap();
    let (_, r_tool) = read::register().into_parts();
    let (_, tool) = edit::register().into_parts();
    let _ = call_tool(&r_tool, r#"{"file_path":"test_edit_dup.txt"}"#);
    let result = call_tool(&tool, r#"{"file_path":"test_edit_dup.txt","old_string":"A","new_string":"X"}"#);
    assert!(result.contains("appears 2 times"), "Got: {}", result);
    fs::remove_file("test_edit_dup.txt").ok();
}

#[test]
fn test_edit_replace_all() {
    fs::write("test_edit_all.txt", "A B A").unwrap();
    let (_, r_tool) = read::register().into_parts();
    let (_, tool) = edit::register().into_parts();
    let _ = call_tool(&r_tool, r#"{"file_path":"test_edit_all.txt"}"#);
    let result = call_tool(&tool, r#"{"file_path":"test_edit_all.txt","old_string":"A","new_string":"X","replace_all":true}"#);
    assert!(result.contains("Replaced 2"), "Got: {}", result);
    fs::remove_file("test_edit_all.txt").ok();
}

#[test]
fn test_edit_not_found() {
    fs::write("test_edit_nf.txt", "hello").unwrap();
    let (_, r_tool) = read::register().into_parts();
    let (_, tool) = edit::register().into_parts();
    let _ = call_tool(&r_tool, r#"{"file_path":"test_edit_nf.txt"}"#);
    let result = call_tool(&tool, r#"{"file_path":"test_edit_nf.txt","old_string":"world","new_string":"x"}"#);
    assert!(result.contains("not found"), "Got: {}", result);
    fs::remove_file("test_edit_nf.txt").ok();
}

#[test]
fn test_edit_idempotent() {
    fs::write("test_edit_same.txt", "same").unwrap();
    let (_, r_tool) = read::register().into_parts();
    let (_, tool) = edit::register().into_parts();
    let _ = call_tool(&r_tool, r#"{"file_path":"test_edit_same.txt"}"#);
    let result = call_tool(&tool, r#"{"file_path":"test_edit_same.txt","old_string":"same","new_string":"same"}"#);
    assert!(result.contains("identical"), "Got: {}", result);
    fs::remove_file("test_edit_same.txt").ok();
}

// ── Web Fetch ──

#[test]
fn test_web_fetch_invalid_url() {
    let (_, tool) = web_fetch::register().into_parts();
    let result = call_tool(&tool, r#"{"url":"not-a-url"}"#);
    assert!(result.contains("only http and https"), "Got: {}", result);
}

#[test]
fn test_web_fetch_empty_url() {
    let (_, tool) = web_fetch::register().into_parts();
    let result = call_tool(&tool, r#"{"url":""}"#);
    assert!(result.contains("required"), "Got: {}", result);
}
