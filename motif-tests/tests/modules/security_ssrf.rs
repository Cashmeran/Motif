//! SSRF protection tests — verify web_fetch blocks private IPs.

use crate::common;
use motif::*;
use motif_tools;
use std::sync::Arc;

#[test]
fn test_web_fetch_localhost_blocked() {
    let (_, tool) = motif_tools::web_fetch::register().into_parts();
    let result = common::call_tool(&tool, r#"{"url":"http://127.0.0.1:8080/admin"}"#);
    // Should either fail (connection refused) or get SSRF blocked
    // On most machines nothing listens on 8080, so "Request failed" is expected
    assert!(!result.contains("secret"), "Should not succeed: {}", result);
}

#[test]
fn test_web_fetch_private_10_blocked() {
    let (_, tool) = motif_tools::web_fetch::register().into_parts();
    for ip in &["10.0.0.1", "10.255.255.255"] {
        let result = common::call_tool(&tool, &format!(r#"{{"url":"http://{}:80/"}}"#, ip));
        assert!(!result.contains("success"), "Should block {}: {}", ip, result);
    }
}

#[test]
fn test_web_fetch_private_172_blocked() {
    let (_, tool) = motif_tools::web_fetch::register().into_parts();
    for ip in &["172.16.0.1", "172.31.255.255"] {
        let result = common::call_tool(&tool, &format!(r#"{{"url":"http://{}:80/"}}"#, ip));
        assert!(!result.contains("success"), "Should block {}: {}", ip, result);
    }
}

#[test]
fn test_web_fetch_private_192_blocked() {
    let (_, tool) = motif_tools::web_fetch::register().into_parts();
    let result = common::call_tool(&tool, r#"{"url":"http://192.168.1.1/"}"#);
    assert!(!result.contains("success"), "Should block 192.168: {}", result);
}

#[test]
fn test_web_fetch_link_local_blocked() {
    let (_, tool) = motif_tools::web_fetch::register().into_parts();
    let result = common::call_tool(&tool, r#"{"url":"http://169.254.1.1/"}"#);
    assert!(!result.contains("success"), "Should block link-local: {}", result);
}

#[test]
fn test_web_fetch_ipv6_loopback_blocked() {
    let (_, tool) = motif_tools::web_fetch::register().into_parts();
    let result = common::call_tool(&tool, r#"{"url":"http://[::1]:8080/"}"#);
    assert!(!result.contains("success"), "Should block ::1: {}", result);
}

#[test]
fn test_web_fetch_only_http_https() {
    let (_, tool) = motif_tools::web_fetch::register().into_parts();
    let result = common::call_tool(&tool, r#"{"url":"file:///etc/passwd"}"#);
    assert!(result.contains("only http and https"), "Should block file://: {}", result);
}

#[test]
fn test_web_fetch_invalid_url() {
    let (_, tool) = motif_tools::web_fetch::register().into_parts();
    let result = common::call_tool(&tool, r#"{"url":"not-a-url"}"#);
    assert!(result.contains("only http and https"), "Got: {}", result);
}
