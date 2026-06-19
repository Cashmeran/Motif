use motif::*;

#[test] fn test_message_system() {
    let m = Message::system("hello");
    let json = serde_json::to_string(&m).unwrap();
    assert!(json.contains("system"));
}
#[test] fn test_message_user() {
    let m = Message::user("hi");
    let json = serde_json::to_string(&m).unwrap();
    assert!(json.contains("user"));
}
#[test] fn test_tool_call_serialization() {
    let tc = ToolCall {
        id: "c1".into(), call_type: "function".into(),
        function: FunctionCall { name: "f".into(), arguments: "{}".into() },
    };
    let json = serde_json::to_string(&tc).unwrap();
    assert!(json.contains("c1"));
}
#[test] fn test_finish_reason_stop() {
    assert!(serde_json::to_string(&FinishReason::Stop).unwrap().contains("stop"));
}
#[test] fn test_token_usage() {
    let tu = TokenUsage { prompt_tokens: 5, completion_tokens: 3, total_tokens: 8 };
    assert_eq!(tu.total_tokens, 8);
}
