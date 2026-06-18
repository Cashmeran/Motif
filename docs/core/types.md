# Types

核心数据模型。所有类型实现 `Serialize + Deserialize + Clone + Debug`。

## Message 枚举

Agent 和 LLM 之间的消息抽象。

```rust
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    System(SystemMessage),
    User(UserMessage),
    Assistant(AssistantMessage),
    Tool(ToolMessage),
}
```

### SystemMessage

```rust
pub struct SystemMessage { pub content: String }
```

### UserMessage

```rust
pub struct UserMessage { pub content: String }
```

### AssistantMessage

```rust
pub struct AssistantMessage {
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>, // 仅 OpenAI 工具调用模式
}
```

### ToolMessage

```rust
pub struct ToolMessage {
    pub tool_call_id: String,  // 对应 AssistantMessage 中的 ToolCall.id
    pub content: String,       // 工具执行结果
}
```

### 构造器

```rust
Message::system("You are...")
Message::user("Hello")
```

## ToolCall

```rust
pub struct ToolCall {
    pub id: String,
    pub call_type: String,       // "function"
    pub function: FunctionCall,
}

pub struct FunctionCall {
    pub name: String,            // 工具名
    pub arguments: String,       // JSON 参数字符串
}
```

## ToolDefinition

发给 LLM 的工具 schema 定义。

```rust
pub struct ToolDefinition {
    pub tool_type: String,       // "function"
    pub function: ToolFunction,
}

pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: Parameters,   // JSON Schema
}

pub struct Parameters(Value);     // #[serde(transparent)]
```

## ToolResult

```rust
pub struct ToolResult {
    pub tool_message: ToolMessage,
    pub timestamp: SystemTime,    // 执行开始时间
    pub elapsed: Duration,        // 执行耗时
}
```

## TimedMessage

```rust
pub struct TimedMessage {
    pub message: Message,
    pub timestamp: SystemTime,    // 添加时间
    pub elapsed: Duration,        // 关联操作耗时
}

impl TimedMessage {
    pub fn new(message: Message) -> Self { ... }
}
```

## LLMResponse

```rust
pub struct LLMResponse {
    pub message: AssistantMessage,
    pub finish_reason: FinishReason,
    pub usage: Option<TokenUsage>,  // API 返回时填充
}
```

## FinishReason

```rust
pub enum FinishReason {
    Stop,           // 正常完成
    Length,         // 因长度限制截断
    ToolCalls,      // 需要执行工具调用
    ContentFilter,  // 内容被过滤
    Custom(String), // 其他原因
}
```

## TokenUsage

```rust
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
```

## StreamEvent

```rust
pub enum StreamEvent {
    Content(String),       // 流式文本增量
    Finish(FinishReason),  // 流结束
}
```

## Parameters

```rust
#[serde(transparent)]
pub struct Parameters(Value);

impl Parameters {
    pub fn new(schema: Value) -> Self;
    pub fn from_type<T: JsonSchema>() -> Self; // 从 #[tool] 宏生成的类型中自动生成模式
}
```
