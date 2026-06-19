use crate::common;
use motif_tools;

#[test] fn test_bash_echo() {
    let (_, t) = motif_tools::bash::register().into_parts();
    let r = common::call_tool(&t, r#"{"command":"echo hello","timeout_ms":5000}"#);
    if cfg!(target_os = "windows") { assert!(r.contains("hello") || r.contains("echo"), "{}", r); }
    else { assert!(r.contains("hello"), "{}", r); }
}
#[test] fn test_bash_dollar_blocked() {
    let (_, t) = motif_tools::bash::register().into_parts();
    let r = common::call_tool(&t, r#"{"command":"echo $HOME","timeout_ms":5000}"#);
    assert!(r.contains("not allowed"), "{}", r);
}
