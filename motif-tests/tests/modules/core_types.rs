//! Tests for core types: Message serialization, ToolCall, ToolDefinition, parameters.

use motif::*;

#[test]
fn test_message_system_serialization() {
    let m = Message::system("you are helpful");
    let json = serde_json::to_string(&m).unwrap();
    assert!(json.contains("system"));
}

#[test]
fn test_message_user_serialization() {
    let m = Message::user("hello");
    let json = serde_json::to_string(&m).unwrap();
    assert!(json.contains("user"));
}

#[test]
fn test_message_assistant_serialization() {
    let m = Message::assistant("hi", None);
    let json = serde_json::to_string(&m).unwrap();
    assert!(json.contains("assistant"));
}

#[test]
fn test_message_tool_serialization() {
    let m = Message::Tool(ToolMessage { tool_call_id: "call_1".into(), content: "result".into() });
    let json = serde_json::to_string(&m).unwrap();
    assert!(json.contains("tool"));
}

#[test]
fn test_message_deserialization() {
    let json = r#"{"role":"user","content":"hello"}"#;
    let m: Message = serde_json::from_str(json).unwrap();
    match m {
        Message::User(ref u) => assert_eq!(u.content, "hello"),
        _ => panic!("expected User variant"),
    }
}

#[test]
fn test_tool_call_serialization() {
    let tc = ToolCall {
        id: "call_x".into(),
        call_type: "function".into(),
        function: FunctionCall {
            name: "search".into(),
            arguments: r#"{"q":"test"}"#.into(),
        },
    };
    let json = serde_json::to_string(&tc).unwrap();
    assert!(json.contains("search"));
    assert!(json.contains("call_x"));
}

#[test]
fn test_tool_definition_schema() {
    let def = ToolDefinition::new("greet", "Say hi", Parameters::new(serde_json::json!({
        "type": "object",
        "properties": {},
        "required": []
    })));
    assert_eq!(def.function.name, "greet");
    assert!(def.function.description.contains("hi"));
}

#[test]
fn test_finish_reason_stop() {
    let fr = FinishReason::Stop;
    let json = serde_json::to_string(&fr).unwrap();
    assert!(json.contains("stop"));
}

#[test]
fn test_finish_reason_tool_calls() {
    let fr = FinishReason::ToolCalls;
    let json = serde_json::to_string(&fr).unwrap();
    assert!(json.contains("tool_calls"));
}

#[test]
fn test_token_usage() {
    let tu = TokenUsage { prompt_tokens: 10, completion_tokens: 5, total_tokens: 15 };
    assert_eq!(tu.total_tokens, 15);
}

#[test]
fn test_timed_message_creation() {
    let tm = TimedMessage::new(Message::user("test"));
    match tm.message {
        Message::User(ref u) => assert_eq!(u.content, "test"),
        _ => panic!("wrong variant"),
    }
}
