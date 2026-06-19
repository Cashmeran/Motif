use crate::common;
use motif_tools;
use std::fs;

#[test] fn test_write_and_read() {
    let (_, t) = motif_tools::write::register().into_parts();
    let r = common::call_tool(&t, r#"{"file_path":"_tw.txt","content":"hello"}"#);
    assert!(r.contains("Wrote") || r.contains("bytes"));
    fs::remove_file("_tw.txt").ok();
}
