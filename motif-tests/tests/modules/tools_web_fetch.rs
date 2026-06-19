use crate::common;
use motif_tools;

#[test]
fn test_web_fetch_rejects_invalid_url() {
    let (_, t) = motif_tools::web_fetch::register().into_parts();
    let r = common::call_tool(&t, r#"{"url":"not-a-url"}"#);
    assert!(r.contains("only http and https"), "{}", r);
}

#[test]
fn test_web_fetch_rejects_empty() {
    let (_, t) = motif_tools::web_fetch::register().into_parts();
    let r = common::call_tool(&t, r#"{"url":""}"#);
    assert!(r.contains("required"), "{}", r);
}

#[test]
fn test_web_fetch_rejects_file_url() {
    let (_, t) = motif_tools::web_fetch::register().into_parts();
    let r = common::call_tool(&t, r#"{"url":"file:///etc/passwd"}"#);
    assert!(r.contains("only http and https"), "{}", r);
}

#[test]
fn test_web_fetch_rejects_ftp() {
    let (_, t) = motif_tools::web_fetch::register().into_parts();
    let r = common::call_tool(&t, r#"{"url":"ftp://example.com/file"}"#);
    assert!(r.contains("only http and https"), "{}", r);
}
