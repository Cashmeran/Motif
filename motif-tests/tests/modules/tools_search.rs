//! Search tool tests: 4 modes, pagination, edge cases.

use crate::common;
use motif::*;
use motif_tools;
use std::fs;
use std::sync::Arc;

#[test]
fn test_search_filename_mode() {
    let (_, tool) = motif_tools::search::register().into_parts();
    let result = common::call_tool(&tool, r#"{"query":"*.rs","mode":"filename","path":"../motif/src"}"#);
    assert!(result.contains("agent.rs"), "Should find agent.rs: {}", result);
    assert!(result.contains("provider.rs"), "Should find provider.rs: {}", result);
}

#[test]
fn test_search_content_mode() {
    let (_, tool) = motif_tools::search::register().into_parts();
    let result = common::call_tool(&tool, r#"{"query":"StopCondition","mode":"content","path":"../motif/src","glob":"*.rs"}"#);
    assert!(result.contains("StopCondition"), "Should find StopCondition: {}", result);
}

#[test]
fn test_search_files_with_matches() {
    let (_, tool) = motif_tools::search::register().into_parts();
    let result = common::call_tool(&tool, r#"{"query":"pub struct Agent","mode":"files_with_matches","path":"../motif/src","glob":"*.rs"}"#);
    assert!(result.contains("agent.rs"), "Should list agent.rs: {}", result);
}

#[test]
fn test_search_count() {
    let (_, tool) = motif_tools::search::register().into_parts();
    let result = common::call_tool(&tool, r#"{"query":"fn","mode":"count","path":"../motif/src","glob":"*.rs"}"#);
    assert!(!result.is_empty(), "Count mode should produce output");
}

#[test]
fn test_search_pagination() {
    let (_, tool) = motif_tools::search::register().into_parts();
    let result = common::call_tool(&tool, r#"{"query":"use","mode":"filename","path":"../motif/src","head_limit":2,"offset":0}"#);
    assert!(!result.is_empty());
    let all = common::call_tool(&tool, r#"{"query":"use","mode":"filename","path":"../motif/src","head_limit":0}"#);
    assert!(!all.is_empty());
}

#[test]
fn test_search_case_insensitive() {
    let (_, tool) = motif_tools::search::register().into_parts();
    let result = common::call_tool(&tool, r#"{"query":"stopcondition","mode":"content","path":"../motif/src","glob":"*.rs","ignore_case":true}"#);
    assert!(result.contains("StopCondition") || result.to_lowercase().contains("stopcondition"),
        "Case-insensitive should match");
}

#[test]
fn test_search_nonexistent_path() {
    let (_, tool) = motif_tools::search::register().into_parts();
    let result = common::call_tool(&tool, r#"{"query":"x","path":"/nonexistent/path","mode":"filename"}"#);
    assert!(result.contains("not found"), "Should report path not found");
}

#[test]
fn test_search_tool_schema_correct() {
    let (def, _tool) = motif_tools::search::register().into_parts();
    assert_eq!(def.function.name, "search");
    assert!(!def.function.description.is_empty());
}

#[test]
fn test_search_excludes_build_dirs() {
    let _ = fs::create_dir_all("_test_build/__pycache__");
    fs::write("_test_build/__pycache__/test.pyc", "0000000").ok();
    fs::write("_test_build/visible.txt", "hello build test world").ok();
    let (_, tool) = motif_tools::search::register().into_parts();
    let result = common::call_tool(&tool, r#"{"query":"hello build","mode":"content","path":"_test_build"}"#);
    assert!(result.contains("visible.txt"), "Should find visible.txt: {}", result);
    assert!(!result.contains("test.pyc"), "Should exclude __pycache__: {}", result);
    fs::remove_dir_all("_test_build").ok();
}

#[test]
fn test_search_empty_query() {
    let (_, tool) = motif_tools::search::register().into_parts();
    let result = common::call_tool(&tool, r#"{"query":"","path":"../motif/src"}"#);
    assert!(result.contains("Error") || result.contains("required"));
}

#[test]
fn test_search_multiline_regex() {
    fs::write("_t_ml.txt", "struct Foo {\n    x: i32,\n}\n\nstruct Bar {\n    y: i32,\n}\n").ok();
    let (_, tool) = motif_tools::search::register().into_parts();
    let result = common::call_tool(&tool, r#"{"query":"struct \\w+ \\{[^}]*\\}","mode":"content","path":"_t_ml.txt","multiline":true}"#);
    assert!(!result.contains("Invalid regex"));
    fs::remove_file("_t_ml.txt").ok();
}

#[test]
fn test_search_globstar_pattern() {
    let (_, tool) = motif_tools::search::register().into_parts();
    let result = common::call_tool(&tool, r#"{"query":"*.rs","mode":"filename","path":"../motif/src","glob":"*.rs"}"#);
    assert!(result.contains("agent.rs"), "Globstar search: {}", result);
}
