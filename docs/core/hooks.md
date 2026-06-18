# AgentHook

Agent 生命周期的可扩展点。15 个方法，全部默认空操作，按需实现。

## Trait 定义

```rust
#[async_trait]
pub trait AgentHook: Send + Sync {
    // Run 级 (3)
    async fn before_run(&self, ctx: &mut RunContext) -> Result<()> { Ok(()) }
    async fn after_run(&self, ctx: &mut RunContext) -> Result<()> { Ok(()) }
    async fn on_finally(&self, ctx: &mut RunContext) -> Result<()> { Ok(()) }

    // Iteration 级 (2)
    async fn before_llm(&self, ctx: &mut HookContext) -> Result<()> { Ok(()) }
    async fn after_llm(&self, ctx: &mut HookContext) -> Result<()> { Ok(()) }

    // Tool 级 (2)
    async fn before_tools(&self, ctx: &mut HookContext) -> Result<()> { Ok(()) }
    async fn after_tools(&self, ctx: &mut HookContext) -> Result<()> { Ok(()) }

    // Message 级 (1)
    async fn on_message(&self, msg: &TimedMessage) -> Result<bool> { Ok(true) }

    // Stop 级 (1)
    async fn on_stop_check(&self, ctx: &mut HookContext, should_stop: bool) -> Result<bool> {
        Ok(should_stop)
    }

    // Stream 级 (4)
    fn wants_streaming(&self) -> bool { true }
    async fn on_stream_delta(&self, delta: &str) -> Result<()> { Ok(()) }
    async fn on_stream_end(&self, resuming: bool) -> Result<()> { Ok(()) }
    async fn on_reasoning_delta(&self, delta: &str) -> Result<()> { Ok(()) }

    // Error (1)
    async fn on_error(&self, ctx: &mut HookContext, error: &Error) -> Result<()> { Ok(()) }

    // Content (1)
    fn finalize_content(&self, content: &str) -> String { content.to_string() }
}
```

## 执行顺序

```
chat() 或 chat_stream()
  │
  ├─ before_run        ← 初始化
  │
  ├─ [loop]
  │   ├─ before_llm    ← 记忆注入、上下文压缩
  │   ├─ [LLM call]
  │   │   ├─ on_stream_delta  ← 逐字流式输出
  │   │   ├─ on_reasoning_delta ← 推理过程（思考模式）
  │   │   └─ on_stream_end    ← resuming: true=tools follow, false=final
  │   ├─ on_message     ← 过滤每条消息（助手/工具/继续）
  │   ├─ after_llm     ← 响应审计、token 统计
  │   ├─ before_tools  ← 权限校验
  │   ├─ [tool execution]
  │   ├─ after_tools   ← 结果缓存
  │   ├─ on_stop_check ← 覆盖退出决定（Ralph Loop gate）
  │   └─ [stop?]
  │
  ├─ finalize_content   ← 输出后处理（管道）
  ├─ on_error           ← 降级处理
  ├─ after_run          ← 收尾观察
  └─ on_finally         ← 保证清理（资源刷新/连接关闭）
```

## 全表

| # | Hook | 类型 | 用途 |
|---|------|------|------|
| 1 | `before_run` | Run | 初始化：加载配置、注册资源 |
| 2 | `after_run` | Run | 收尾：汇总统计、记录日志 |
| 3 | `on_finally` | Run | **保证执行**：刷新缓冲区、关闭连接 |
| 4 | `before_llm` | Iteration | 记忆检索、上下文压缩、请求审计 |
| 5 | `after_llm` | Iteration | 响应观察、token 统计、内容审计 |
| 6 | `before_tools` | Tool | 权限校验、参数审核 |
| 7 | `after_tools` | Tool | 结果缓存、副作用追踪 |
| 8 | `on_message` | Message | 过滤/脱敏：返回 false 丢弃消息 |
| 9 | `on_stop_check` | Stop | **覆盖退出**：返回 false 继续循环 |
| 10 | `wants_streaming` | Stream | Hook 声明是否需要流式 |
| 11 | `on_stream_delta` | Stream | 流式内容增量（UI 渲染） |
| 12 | `on_stream_end` | Stream | 流结束信号（resuming: bool） |
| 13 | `on_reasoning_delta` | Reasoning | 推理过程增量（DeepSeek 思考模式） |
| 14 | `on_error` | Error | 降级处理、错误恢复 |
| 15 | `finalize_content` | Content | 输出后处理（管道链） |

## 和 nanobot / CC 对比

| | nanobot | CC | Motif |
|---|:--:|:--:|:--:|
| 通用生命周期 | 5 | 8 | **7** (before_run, after_run, on_finally, before_llm, after_llm, on_error, finalize_content) |
| 工具级 | 1 | 6 (产品专用) | **2** (before_tools, after_tools) |
| 消息过滤 | ❌ | ❌ | **✅ on_message** |
| 退出闸门 | ❌ | ❌ | **✅ on_stop_check** |
| 流式 | 4 | 3 | **4** (wants_streaming, on_stream_delta, on_stream_end, on_reasoning_delta) |
| 总数 | 13 | 27 (12 通用+15 产品) | **15** |

## 错误隔离

Agent 遍历 hooks 时，每个 hook 的错误独立捕获 `tracing::warn!`。一个 hook 失败不影响其他。

## 注册

```rust
agent
    .hook(MyMemoryHook)
    .hook(MyAuditHook)
    .hook(StreamPrinter);
```

多个 hook 按注册顺序调用。`finalize_content` 按序管道连接。
