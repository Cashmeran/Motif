//! Provider retry tests — verify retry on 429/5xx, no retry on 4xx.

use motif::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Tiny HTTP server that returns configurable responses in sequence.
struct RetryServer {
    addr: String,
    request_count: Arc<AtomicUsize>,
}

impl RetryServer {
    /// Spawn a server that returns the responses in order.
    /// responses: list of (status_code, body) tuples.
    async fn new(responses: Vec<(u16, &'static str)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();
        let resp = responses;
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = match listener.accept().await {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let idx = c.fetch_add(1, Ordering::SeqCst);
                let (status, body) = if idx < resp.len() {
                    resp[idx]
                } else {
                    resp.last().copied().unwrap_or((200, "{}"))
                };
                let body = if body == "{}" {
                    format!(
                        r#"{{"choices":[{{"message":{{"content":"retry_ok_{}"}},"finish_reason":"stop"}}]}}"#,
                        idx
                    )
                } else {
                    body.to_string()
                };
                let response = format!(
                    "HTTP/1.1 {}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
                    status, body.len(), body
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
                let _ = tokio::time::timeout(std::time::Duration::from_millis(100), async {
                    let mut buf = [0u8; 4096];
                    let _ = socket.read(&mut buf).await;
                })
                .await;
            }
        });
        // Wait briefly for server to start
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        Self {
            addr,
            request_count: counter,
        }
    }
}

#[tokio::test]
async fn test_provider_retry_on_429() {
    let server = RetryServer::new(vec![
        (429, r#"{"error":"rate limited"}"#),
        (429, r#"{"error":"rate limited"}"#),
        (200, "{}"),
    ])
    .await;

    let provider = OpenAIProvider::new(&server.addr, "sk-test", "test-model").with_retry(3);
    let result = provider.call(&[], &[]).await.unwrap();
    assert!(
        result.message.content.contains("retry_ok"),
        "Should succeed after retry: {:?}",
        result.message.content
    );
    assert!(
        server.request_count.load(Ordering::SeqCst) >= 2,
        "Should have made multiple requests"
    );
}

#[tokio::test]
async fn test_provider_no_retry_on_400() {
    let server = RetryServer::new(vec![(400, r#"{"error":"bad request"}"#)]).await;

    let provider = OpenAIProvider::new(&server.addr, "sk-test", "test-model").with_retry(2);
    let result = provider.call(&[], &[]).await;
    assert!(result.is_err(), "400 should not be retried");
    assert_eq!(
        server.request_count.load(Ordering::SeqCst),
        1,
        "Only 1 request for 4xx"
    );
}

#[tokio::test]
async fn test_provider_retry_on_503() {
    let server = RetryServer::new(vec![
        (503, r#"{"error":"service unavailable"}"#),
        (200, "{}"),
    ])
    .await;

    let provider = OpenAIProvider::new(&server.addr, "sk-test", "test-model").with_retry(3);
    let result = provider.call(&[], &[]).await.unwrap();
    assert!(result.message.content.contains("retry_ok"));
    assert!(server.request_count.load(Ordering::SeqCst) >= 2);
}

#[tokio::test]
async fn test_provider_retry_exhausted() {
    let server = RetryServer::new(vec![
        (429, r#"{"error":"rate limited"}"#),
        (429, r#"{"error":"rate limited"}"#),
        (429, r#"{"error":"rate limited"}"#),
    ])
    .await;

    let provider = OpenAIProvider::new(&server.addr, "sk-test", "test-model").with_retry(2);
    let result = provider.call(&[], &[]).await;
    assert!(result.is_err(), "Should fail after exhausting retries");
}
