# Agent

Agent 是 Motif 唯一的状态持有者。它组合了 `LLMProvider`、`History`、`ToolExecutor`、`AgentHook`、`Prompt` 和 `PromptBuilder`，并实现了核心循环。

## 构造

```rust
let agent = Agent::new(provider)   // 必选：LLMProvider
    .history(BoundedHistory::new(60))   // 可选：默认 InfiniteHistory
    .executor(Executor::parallel())     // 可选：默认 Executor::parallel()
    .hook(MyHook)                        // 可选：可多次调用
    .prompt_builder(MyBuilder)          // 可选：可多次调用
    .model("deepseek-v4-pro")          // 运行时上下文注入
    .max_iterations(50)                // 安全上限，默认 100
    .stop_when(StopCondition::OnText); // 终止策略
```

## 字段

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `provider` | `Arc<dyn LLMProvider>` | 构造时必传 | LLM 调用抽象 |
| `history` | `Box<dyn History>` | `InfiniteHistory::new()` | 对话记忆 |
| `executor` | `Box<dyn ToolExecutor>` | `Executor::parallel()` | 工具执行策略 |
| `hooks` | `Vec<Box<dyn AgentHook>>` | `vec![]` | 生命周期回调 |
| `prompt` | `Prompt` | `Prompt::new()` | 3 层缓存系统提示词 |
| `prompt_builders` | `Vec<Box<dyn PromptBuilder>>` | `vec![]` | 每轮注入扩展 |
| `tool_definitions` | `Vec<ToolDefinition>` | `vec![]` | 工具 schema 缓存 |
| `model` | `String` | `""` | 运行时上下文使用的模型名 |
| `stop_condition` | `StopCondition` | `OnText` | 控制何时终止循环 |
| `max_iterations` | `usize` | `0`（无限） | 安全防护上限，0 表示不限制。配合 `OnStuck` 检测死循环 |
| `total_tokens` | `u64` | `0` | 累计 token 消耗 |

## 核心方法

### `async fn step(&mut self) -> Result<Option<String>>`

执行一个完整的 LLM 往返：构建提示词 → 调用 LLM → 执行工具 → 追加历史 → 检查终止。

- `Ok(Some(text))` — 循环应该停止，返回最终文本
- `Ok(None)` — 继续循环
- `Err(e)` — 错误，停止循环

外部可以调用 `step()` 手动控制循环：

```rust
loop {
    match agent.step().await? {
        Some(result) => { println!("{}", result); break; }
        None => continue,
    }
}
```

### `async fn run(&mut self) -> Result<String>`

内部调用 `step()` 直到 `StopCondition` 触发，同时跑 `before_run` / `after_run` hooks 和 `on_error`。

### `async fn chat(&mut self, content: impl Into<String>) -> Result<String>`

在用户消息前自动注入 `runtime_context`（日期+模型名），然后调用 `run()`。

### `fn total_tokens_used(&self) -> u64`

从 Provider 返回的 `usage.total_tokens` 中累计。用于上下文预算监控。

## StopCondition （终止条件）

```rust
pub enum StopCondition {
    OnText,                                    // 无工具调用时停（默认）
    AfterNTools(usize),                        // N 个工具结果后停
    OnStuck { max_repeats: usize },             // 相同调用重复 N 次停
    Never,                                     // 永远不停（外部控制）
    Custom(Arc<dyn Fn(&LLMResponse, &[TimedMessage]) -> bool>), // 任意逻辑
}
```

### 示例

```rust
// 验证循环：直到输出通过验证
agent.stop_when(StopCondition::Custom(Arc::new(|resp, _| {
    resp.message.content.contains("VERIFIED")
})));

// 最多 3 轮工具调用
agent.stop_when(StopCondition::AfterNTools(3));

// 卡住检测
agent.stop_when(StopCondition::OnStuck { max_repeats: 3 });
```

## 恢复逻辑

Agent 内置两个自动恢复机制：

| 机制 | 触发条件 | 行为 |
|------|---------|------|
| 空响应重试 | 无工具调用且 content 为空 | 最多重试 2 次 |
| Length 续写 | `finish_reason == Length` 且 content 非空 | 追加 "continue"，最多 3 次 |

默认参数：`max_empty_retries = 2`、`max_length_continues = 3`。

## 替换指南

| 替换什么 | 怎么替换 |
|---------|---------|
| LLM Provider | 实现 `LLMProvider` trait，传 `Agent::new(my_provider)` |
| 对话记忆 | 实现 `History` trait，传 `.history(my_history)` |
| 工具执行 | 实现 `ToolExecutor` trait，传 `.executor(my_executor)` |
| 终止策略 | `StopCondition::Custom(...)` |
| 生命周期 | 实现 `AgentHook` trait，传 `.hook(my_hook)`（可多次） |
| 提示词扩展 | 实现 `PromptBuilder` trait，传 `.prompt_builder(my_builder)` |
