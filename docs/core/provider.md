# LLMProvider

定义模型调用抽象。核心只定义接口，实现完全在外部。

## Trait 定义

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// 非流式调用——Agent 内部决策用
    async fn call(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LLMResponse>;

    /// 流式调用——UI 渲染用。默认 fallback 到 call()
    async fn call_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LLMStream> { ... }
}
```

## 返回类型

### LLMResponse

```rust
pub struct LLMResponse {
    pub message: AssistantMessage,   // content + 可选的 tool_calls
    pub finish_reason: FinishReason, // Stop | Length | ToolCalls | ContentFilter | Custom
    pub usage: Option<TokenUsage>,   // API 返回的真实 token 消耗
}
```

### LLMStream

```rust
pub struct LLMStream {
    pub receiver: tokio::sync::mpsc::Receiver<StreamEvent>,
}

pub enum StreamEvent {
    Content(String),    // 文本增量
    Thinking(String),   // 推理/思考增量（DeepSeek 等推理模型）
    Finish(FinishReason), // 流结束
}
```

## OpenAIProvider

内置一个 OpenAI 兼容协议的实现，支持所有 `/v1/chat/completions` 端点。

### 构造与配置

```rust
let provider = OpenAIProvider::new(
    "https://api.deepseek.com",  // base_url
    "sk-...",                    // api_key
    "deepseek-v4-pro",           // model
)
.with_thinking("max")           // DeepSeek 思考模式
.with_body("temperature", 0.0)  // 额外请求参数
.with_retry(3)                  // 重试次数
.with_client(custom_client);    // 自定义 HTTP 客户端
```

### 重试策略

- HTTP 429（速率限制）→ 重试
- HTTP 5xx（服务器错误）→ 重试
- HTTP 4xx（其他客户端错误）→ 不重试，立即返回错误
- 网络错误（连接失败/超时）→ 重试
- 默认重试 2 次，间隔 1 秒递增

### Token 追踪

`OpenAIProvider` 自动从 API 响应提取 `usage` 字段，填入 `LLMResponse.usage`。Agent 在 `step()` 中累加到 `total_tokens`。

### Anthropic 格式

```rust
let provider = OpenAIProvider::new(
    "https://api.deepseek.com/anthropic", "sk-...", "deepseek-v4-pro",
).with_anthropic();
```

支持 DeepSeek Anthropic 端点（`/anthropic/messages`）。消息自动转换：
- System prompt → 顶层 `system` 字段
- 工具 schema → `input_schema` 格式
- 工具结果 → `tool_result` content block
- Auth → `x-api-key` header

### 思考模式（DeepSeek）

```rust
let provider = OpenAIProvider::new(...).with_thinking("max");
// 请求体自动注入：
// "thinking": {"type": "enabled"}
// "reasoning_effort": "max"
```

## 自定义 Provider

```rust
struct AnthropicProvider { ... }

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn call(&self, messages: &[Message], tools: &[ToolDefinition]) -> Result<LLMResponse> {
        // 1. 构建 Anthropic 格式的请求体
        // 2. 发送 HTTP 请求
        // 3. 解析响应为 LLMResponse
    }
}
```

## 替换指南

| 场景 | 实现 |
|------|------|
| Anthropic Claude | 实现 `LLMProvider`，将 Message 转为 Anthropic Messages API 格式 |
| Ollama 本地 | 实现 `LLMProvider`，调 `http://localhost:11434` |
| 多模型路由 | 实现 `LLMProvider`，内部根据消息长度/复杂度分发到不同 provider |
| Mock 测试 | 返回预制的 `LLMResponse` |
