use crate::common;
use motif_tools;
use std::fs;

#[test] fn test_write_large() {
    let data = "x".repeat(500_000);
    let (_, t) = motif_tools::write::register().into_parts();
    let args = format!(r#"{{"file_path":"_sio_big.txt","content":"{}"}}"#, data);
    let r = common::call_tool(&t, &args);
    assert!(r.contains("Wrote") || r.contains("bytes"), "{}", r);
    fs::remove_file("_sio_big.txt").ok();
}
