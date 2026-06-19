use crate::common;
use motif_tools;
use std::fs;

#[test]
fn test_curly_to_straight() {
    fs::write("_sq_c2s.txt", "He said \u{201c}hello\u{201d} world").unwrap();
    let (_, r) = motif_tools::read::register().into_parts();
    let (_, e) = motif_tools::edit::register().into_parts();
    common::call_tool(&r, r#"{"file_path":"_sq_c2s.txt"}"#);
    let res = common::call_tool(
        &e,
        r#"{"file_path":"_sq_c2s.txt","old_string":"He said \"hello\" world","new_string":"X"}"#,
    );
    assert!(res.contains("Edited"), "{}", res);
    fs::remove_file("_sq_c2s.txt").ok();
}
#[test]
fn test_no_normalization_needed() {
    fs::write("_sq_plain.txt", "plain text").unwrap();
    let (_, r) = motif_tools::read::register().into_parts();
    let (_, e) = motif_tools::edit::register().into_parts();
    common::call_tool(&r, r#"{"file_path":"_sq_plain.txt"}"#);
    let res = common::call_tool(
        &e,
        r#"{"file_path":"_sq_plain.txt","old_string":"plain text","new_string":"replaced"}"#,
    );
    assert!(res.contains("Edited"), "{}", res);
    fs::remove_file("_sq_plain.txt").ok();
}
