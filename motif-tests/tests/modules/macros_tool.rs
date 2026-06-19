//! Proc-macro tests: #[tool] on functions, impl blocks, name/rename attributes.

use motif::*;
use motif_tools;
use std::sync::Arc;

// ── Basic #[tool] on function ──

#[tool]
async fn test_tool_greet(name: String) -> String {
    format!("Hello, {}", name)
}

#[tokio::test]
async fn test_tool_macro_on_function() {
    let def = test_tool_greet::definition();
    assert_eq!(def.function.name, "test_tool_greet");

    let tool = test_tool_greet::register();
    let (_, t) = tool.into_parts();
    let result = t.call(r#"{"name":"World"}"#.into()).await;
    assert!(result.contains("Hello"));
}

// ── #[tool] with name attribute ──

#[tool(name = "custom_greeter")]
async fn test_tool_with_name(msg: String) -> String {
    msg
}

#[tokio::test]
async fn test_tool_macro_with_name_attribute() {
    let def = test_tool_with_name::definition();
    assert_eq!(def.function.name, "custom_greeter");
}

// ── #[tool] on impl block ──

struct TestDb;
#[tool]
impl TestDb {
    async fn query(&self, sql: String) -> String {
        format!("result: {}", sql)
    }
}

#[tokio::test]
async fn test_tool_macro_on_impl_block() {
    let tool = TestDb::register_query();
    let (def, t) = tool.into_parts();
    assert!(def.function.name.contains("query"));
    let result = t.call(r#"{"sql":"SELECT 1"}"#.into()).await;
    assert!(result.contains("result"));
}

// ── #[tool] with serde rename ──

#[tool]
async fn test_tool_rename_param(
    #[serde(rename = "queryText")]
    query_text: String,
) -> String {
    query_text
}

#[tokio::test]
async fn test_tool_macro_serde_rename() {
    let tool = test_tool_rename_param::register();
    let (_, t) = tool.into_parts();
    let result = t.call(r#"{"queryText":"hello"}"#.into()).await;
    assert_eq!(result, "hello");
}

// ── #[tool] doc comment becomes description ──

#[tool]
/// Search the database for matching records.
/// Returns formatted results.
async fn test_tool_with_doc(query: String) -> String {
    query
}

#[test]
fn test_tool_macro_doc_comment_as_description() {
    let def = test_tool_with_doc::definition();
    assert!(def.function.description.contains("Search"));
}

// ── #[tool] multiple params ──

#[tool]
async fn test_tool_multi_params(a: String, b: i64, c: bool) -> String {
    format!("{}/{}/{}", a, b, c)
}

#[tokio::test]
async fn test_tool_macro_multiple_params() {
    let tool = test_tool_multi_params::register();
    let (_, t) = tool.into_parts();
    let result = t.call(r#"{"a":"x","b":42,"c":true}"#.into()).await;
    assert_eq!(result, "x/42/true");
}
