use crate::common;
use motif_tools;
use std::fs;

#[test] fn test_search_filename_mode() {
    let (_, t) = motif_tools::search::register().into_parts();
    let r = common::call_tool(&t, r#"{"query":"*.rs","mode":"filename","path":"../motif/src"}"#);
    assert!(r.contains("agent.rs"), "{}", r);
}
#[test] fn test_search_nonexistent_path() {
    let (_, t) = motif_tools::search::register().into_parts();
    let r = common::call_tool(&t, r#"{"query":"x","path":"/no/path","mode":"filename"}"#);
    assert!(r.contains("not found"), "{}", r);
}
#[test] fn test_search_empty_query() {
    let (_, t) = motif_tools::search::register().into_parts();
    let r = common::call_tool(&t, r#"{"query":""}"#);
    assert!(r.contains("Error") || r.contains("required"));
}
#[test] fn test_search_tool_schema() {
    let (def, _) = motif_tools::search::register().into_parts();
    assert_eq!(def.function.name, "search");
}
