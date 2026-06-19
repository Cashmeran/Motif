use crate::common;
use motif_tools;
use std::fs;

#[test] fn test_read_basic() {
    fs::write("_tr.txt", "line1
line2
").unwrap();
    let (_, t) = motif_tools::read::register().into_parts();
    let r = common::call_tool(&t, r#"{"file_path":"_tr.txt"}"#);
    assert!(r.contains("line1"));
    fs::remove_file("_tr.txt").ok();
}
