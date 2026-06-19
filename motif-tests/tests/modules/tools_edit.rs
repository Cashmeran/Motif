use crate::common;
use motif_tools;
use std::fs;

#[test]
fn test_edit_basic() {
    fs::write("_te_b.txt", "Hello World").unwrap();
    let (_, r) = motif_tools::read::register().into_parts();
    let (_, e) = motif_tools::edit::register().into_parts();
    common::call_tool(&r, r#"{"file_path":"_te_b.txt"}"#);
    let res = common::call_tool(
        &e,
        r#"{"file_path":"_te_b.txt","old_string":"World","new_string":"Rust"}"#,
    );
    assert!(res.contains("Edited"), "{}", res);
    assert_eq!(fs::read_to_string("_te_b.txt").unwrap(), "Hello Rust");
    fs::remove_file("_te_b.txt").ok();
}
#[test]
fn test_edit_without_read_blocked() {
    fs::write("_te_nr.txt", "data").unwrap();
    let (_, e) = motif_tools::edit::register().into_parts();
    let res = common::call_tool(
        &e,
        r#"{"file_path":"_te_nr.txt","old_string":"data","new_string":"x"}"#,
    );
    assert!(res.contains("has not been read"), "{}", res);
    fs::remove_file("_te_nr.txt").ok();
}
#[test]
fn test_edit_not_found() {
    fs::write("_te_nf.txt", "hi").unwrap();
    let (_, r) = motif_tools::read::register().into_parts();
    let (_, e) = motif_tools::edit::register().into_parts();
    common::call_tool(&r, r#"{"file_path":"_te_nf.txt"}"#);
    let res = common::call_tool(
        &e,
        r#"{"file_path":"_te_nf.txt","old_string":"zzz_nope","new_string":"b"}"#,
    );
    assert!(res.contains("not found"), "{}", res);
    fs::remove_file("_te_nf.txt").ok();
}
