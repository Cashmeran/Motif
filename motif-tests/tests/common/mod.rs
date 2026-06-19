use std::sync::Mutex;
use motif::*;

pub struct MockProvider {
    responses: Mutex<Vec<LLMResponse>>,
    call_count: Mutex<usize>,
    pub last_messages: Mutex<Vec<Message>>,
    pub last_tools: Mutex<Vec<ToolDefinition>>,
}
impl MockProvider {
    pub fn new(responses: Vec<LLMResponse>) -> Self {
        Self { responses: Mutex::new(responses), call_count: Mutex::new(0), last_messages: Mutex::new(vec![]), last_tools: Mutex::new(vec![]) }
    }
    pub fn call_count(&self) -> usize { *self.call_count.lock().unwrap() }
}
#[async_trait::async_trait]
impl LLMProvider for MockProvider {
    async fn call(&self, messages: &[Message], tools: &[ToolDefinition]) -> motif::Result<LLMResponse> {
        let mut count = self.call_count.lock().unwrap();
        let idx = *count; *count += 1;
        *self.last_messages.lock().unwrap() = messages.to_vec();
        *self.last_tools.lock().unwrap() = tools.to_vec();
        Ok(self.responses.lock().unwrap()[idx].clone())
    }
}

pub fn text(content: &str) -> LLMResponse {
    LLMResponse { message: AssistantMessage { content: content.to_string(), tool_calls: None }, finish_reason: FinishReason::Stop, usage: None }
}
pub fn tool_call(name: &str, args: &str) -> LLMResponse {
    LLMResponse { message: AssistantMessage { content: String::new(),
        tool_calls: Some(vec![ToolCall { id: format!("call_{}", name), call_type: "function".into(),
        function: FunctionCall { name: name.into(), arguments: args.into() } }]) },
        finish_reason: FinishReason::ToolCalls, usage: None }
}
pub fn length_response() -> LLMResponse {
    LLMResponse { message: AssistantMessage { content: "incomplete".into(), tool_calls: None },
        finish_reason: FinishReason::Length, usage: None }
}
pub fn call_tool(tool: &std::sync::Arc<dyn Tool>, args: &str) -> String {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(tool.call(args.to_string()))
}

pub struct TempFile { pub path: String }
impl TempFile {
    pub fn new(name: &str, content: &str) -> Self {
        std::fs::write(name, content).ok();
        Self { path: name.to_string() }
    }
}
impl Drop for TempFile { fn drop(&mut self) { let _ = std::fs::remove_file(&self.path); } }
