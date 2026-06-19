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
#[test] fn test_mtime_change_after_read_blocked() {
    fs::write("_trs_mt.txt", "original").unwrap();
    let (_, r) = motif_tools::read::register().into_parts();
    let (_, e) = motif_tools::edit::register().into_parts();
    // Read to allow editing
    common::call_tool(&r, r#"{"file_path":"_trs_mt.txt"}"#);
    // Modify file externally (simulating mtime change)
    std::thread::sleep(std::time::Duration::from_millis(10));
    fs::write("_trs_mt.txt", "modified externally").unwrap();
    // Edit should be rejected because mtime changed since read
    let res = common::call_tool(&e, r#"{"file_path":"_trs_mt.txt","old_string":"modified externally","new_string":"x"}"#);
    assert!(res.contains("modified since") || res.contains("has not been read"),
        "Should reject edit after external modification: {}", res);
    fs::remove_file("_trs_mt.txt").ok();
}
