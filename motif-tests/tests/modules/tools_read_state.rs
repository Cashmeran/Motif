use crate::common;
use motif_tools;
use std::fs;

#[test] fn test_read_then_edit() {
    fs::write("_trs.txt", "orig").unwrap();
    let (_, r) = motif_tools::read::register().into_parts();
    let (_, e) = motif_tools::edit::register().into_parts();
    common::call_tool(&r, r#"{"file_path":"_trs.txt"}"#);
    let res = common::call_tool(&e, r#"{"file_path":"_trs.txt","old_string":"orig","new_string":"mod"}"#);
    assert!(res.contains("Edited"), "{}", res);
    fs::remove_file("_trs.txt").ok();
}
#[test] fn test_write_new_no_read() {
    let (_, w) = motif_tools::write::register().into_parts();
    let res = common::call_tool(&w, r#"{"file_path":"_trs_new.txt","content":"fresh"}"#);
    assert!(res.contains("Wrote") || res.contains("bytes"), "{}", res);
    fs::remove_file("_trs_new.txt").ok();
}
