use motif::*;

#[test]
fn test_message_roundtrip() {
    let original = Message::user("hello world");
    let json = serde_json::to_string(&original).unwrap();
    let parsed: Message = serde_json::from_str(&json).unwrap();
    match parsed {
        Message::User(u) => assert_eq!(u.content, "hello world"),
        _ => panic!("wrong variant after roundtrip"),
    }
}

#[test]
fn test_assistant_message_with_tool_calls() {
    let tc = ToolCall {
        id: "call_x".into(),
        call_type: "function".into(),
        function: FunctionCall {
            name: "search".into(),
            arguments: r#"{"q":"rust"}"#.into(),
        },
    };
    let msg = Message::Assistant(AssistantMessage {
        content: "using tool".into(),
        tool_calls: Some(vec![tc.clone()]),
    });
    let json = serde_json::to_string(&msg).unwrap();
    let parsed: Message = serde_json::from_str(&json).unwrap();
    match parsed {
        Message::Assistant(a) => {
            assert_eq!(a.content, "using tool");
            assert!(a.tool_calls.unwrap()[0].function.name == "search");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn test_tool_def_schema_roundtrip() {
    let def = ToolDefinition::new(
        "greet",
        "Say hello",
        Parameters::new(serde_json::json!({
            "type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]
        })),
    );
    assert_eq!(def.function.name, "greet");
    let json = serde_json::to_string(&def).unwrap();
    let parsed: ToolDefinition = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.function.name, "greet");
}
