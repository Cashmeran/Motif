//! Provider streaming and Anthropic format tests using mock HTTP.

use motif::*;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Spawn a tiny HTTP server that returns a fixed response, capturing the request body.
struct CapturingServer {
    addr: String,
    body_checks: Arc<std::sync::Mutex<Vec<String>>>,
}

impl CapturingServer {
    async fn new(response_body: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let checks = Arc::new(std::sync::Mutex::new(Vec::new()));
        let c = checks.clone();
        let resp = response_body;
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 16384];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                c.lock().unwrap().push(request);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
                    resp.len(), resp
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        Self {
            addr,
            body_checks: checks,
        }
    }

    fn request_bodies(&self) -> Vec<String> {
        self.body_checks.lock().unwrap().clone()
    }
}

// ── Streaming tests ──

#[tokio::test]
async fn test_stream_content_deltas() {
    // SSE: two content deltas then [DONE]
    let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\" World\"}}]}\n\ndata: [DONE]\n\n";
    let server = CapturingServer::new(sse.to_string()).await;
    let provider = OpenAIProvider::new(&server.addr, "sk-test", "test-model");
    let stream = provider
        .call_stream(&[Message::user("hi")], &[])
        .await
        .unwrap();
    let mut rx = stream.receiver;
    let mut content = String::new();
    loop {
        match rx.recv().await {
            Some(StreamEvent::Content(c)) => content.push_str(&c),
            Some(StreamEvent::Finish(_)) => break,
            Some(StreamEvent::Thinking(_)) => {}
            None => break,
        }
    }
    assert_eq!(content, "Hello World", "Should assemble SSE deltas");
}

#[tokio::test]
async fn test_stream_finish_reason() {
    let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"x\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
    let server = CapturingServer::new(sse.to_string()).await;
    let provider = OpenAIProvider::new(&server.addr, "sk-test", "test-model");
    let stream = provider
        .call_stream(&[Message::user("hi")], &[])
        .await
        .unwrap();
    let mut finish_seen = false;
    let mut rx = stream.receiver;
    while let Some(event) = rx.recv().await {
        if matches!(event, StreamEvent::Finish(_)) {
            finish_seen = true;
        }
    }
    assert!(finish_seen, "Should receive Finish event");
}

// ── Anthropic format tests ──

#[tokio::test]
async fn test_anthropic_text_response() {
    let body = r#"{"id":"msg_1","type":"message","role":"assistant","content":[{"type":"text","text":"Hello from Claude"}],"stop_reason":"end_turn","usage":{"input_tokens":10,"output_tokens":5}}"#;
    let server = CapturingServer::new(body.to_string()).await;
    let provider = OpenAIProvider::new(&server.addr, "sk-test", "test-model").with_anthropic();
    let result = provider.call(&[Message::user("hi")], &[]).await.unwrap();
    assert_eq!(result.message.content, "Hello from Claude");
    assert!(matches!(result.finish_reason, FinishReason::Stop));
}

#[tokio::test]
async fn test_anthropic_tool_use_response() {
    let body = r#"{"id":"msg_2","type":"message","role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"search","input":{"query":"Rust"}}],"stop_reason":"tool_use","usage":{"input_tokens":20,"output_tokens":10}}"#;
    let server = CapturingServer::new(body.to_string()).await;
    let provider = OpenAIProvider::new(&server.addr, "sk-test", "test-model").with_anthropic();
    let result = provider
        .call(&[Message::user("search Rust")], &[])
        .await
        .unwrap();
    assert!(result.message.content.is_empty());
    let tc = result.message.tool_calls.unwrap();
    assert_eq!(tc.len(), 1);
    assert_eq!(tc[0].function.name, "search");
    assert_eq!(tc[0].id, "toolu_1");
    assert!(matches!(result.finish_reason, FinishReason::ToolCalls));
}

#[tokio::test]
async fn test_anthropic_system_prompt_top_level() {
    let body = r#"{"id":"msg_3","type":"message","role":"assistant","content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn","usage":{"input_tokens":10,"output_tokens":2}}"#;
    let server = CapturingServer::new(body.to_string()).await;
    let provider = OpenAIProvider::new(&server.addr, "sk-test", "test-model").with_anthropic();
    provider
        .call(
            &[Message::system("you are helpful"), Message::user("hi")],
            &[],
        )
        .await
        .unwrap();
    // Verify the request body has `system` field at top level, not in messages
    let bodies = server.request_bodies();
    let req_body = bodies.last().unwrap();
    assert!(
        req_body.contains("\"system\""),
        "Anthropic format: system should be top-level field"
    );
    assert!(
        req_body.contains("you are helpful"),
        "System prompt should be in request body"
    );
}

#[tokio::test]
async fn test_anthropic_tools_as_input_schema() {
    let body = r#"{"id":"msg_4","type":"message","role":"assistant","content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn","usage":{"input_tokens":10,"output_tokens":2}}"#;
    let server = CapturingServer::new(body.to_string()).await;
    let provider = OpenAIProvider::new(&server.addr, "sk-test", "test-model").with_anthropic();
    let tool_def = ToolDefinition::new(
        "search",
        "Search the web",
        Parameters::new(serde_json::json!({
            "type": "object", "properties": {"q": {"type": "string"}}, "required": ["q"]
        })),
    );
    provider
        .call(&[Message::user("hi")], &[tool_def])
        .await
        .unwrap();
    let bodies = server.request_bodies();
    let req_body = bodies.last().unwrap();
    assert!(
        req_body.contains("input_schema"),
        "Anthropic tools use input_schema not parameters"
    );
}
