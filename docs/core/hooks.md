# AgentHook

Agent 生命周期的可扩展点。所有方法默认空操作，按需实现。

## Trait 定义

```rust
#[async_trait]
pub trait AgentHook: Send + Sync {
    // Run 级
    async fn before_run(&self, ctx: &mut RunContext) -> Result<()> { Ok(()) }
    async fn after_run(&self, ctx: &mut RunContext) -> Result<()> { Ok(()) }

    // Iteration 级
    async fn before_llm(&self, ctx: &mut HookContext) -> Result<()> { Ok(()) }
    async fn after_llm(&self, ctx: &mut HookContext) -> Result<()> { Ok(()) }

    // Tool 级
    async fn before_tools(&self, ctx: &mut HookContext) -> Result<()> { Ok(()) }
    async fn after_tools(&self, ctx: &mut HookContext) -> Result<()> { Ok(()) }

    // Error
    async fn on_error(&self, ctx: &mut HookContext, error: &Error) -> Result<()> { Ok(()) }

    // Content
    fn finalize_content(&self, content: &str) -> String { content.to_string() }
}
```

## 执行顺序

```
before_run
  ├─ before_llm → LLM call → after_llm
  ├─ before_tools → tool execution → after_tools
  │   (每轮循环，直到 StopCondition 触发)
  ├─ finalize_content（后处理输出文本）
  └─ on_error（任何 step 返回错误时）
after_run
```

## Context 类型

### HookContext（每轮）

```rust
pub struct HookContext {
    pub iteration: usize,          // 第几轮（从 1 开始）
    pub messages: Vec<TimedMessage>, // 当前历史快照
    pub response: Option<LLMResponse>, // LLM 响应（after_llm 后填充）
    pub tool_calls: Vec<ToolCall>,   // 本轮工具调用
    pub tool_results: Vec<ToolResult>, // 本轮工具结果
    pub final_content: Option<String>, // 最终输出
    pub stop_reason: Option<String>,   // 终止原因
}
```

### RunContext（整个 run）

```rust
pub struct RunContext {
    pub final_content: Option<String>,
    pub stop_reason: Option<String>,
    pub error: Option<Error>,
}
```

## 错误隔离

Agent 内部遍历 hooks 时，每个 hook 的错误被单独捕获并 `tracing::warn!`。一个 hook 失败不影响其他 hook。

## 注册

```rust
agent
    .hook(MyLoggingHook)
    .hook(MyMemoryHook);
```

多个 hook 按注册顺序调用。

## 典型用例

| Hook | 用途 |
|------|------|
| `before_llm` | 记忆检索：搜索相关上下文并注入 `ctx.messages` |
| `after_llm` | 内容安全检查：扫描是否有敏感输出 |
| `before_tools` | 权限校验：检查工具调用是否合法 |
| `after_tools` | 结果缓存：将工具结果存入持久化层 |
| `on_error` | 降级处理：错误时尝试恢复 |
| `finalize_content` | 后处理：去除敏感内容、格式化 |
| `before_run` + `after_run` | 指标采集：记录整个对话耗时和 token 消耗 |
