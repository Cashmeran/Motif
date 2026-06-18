# Tool System

定义工具接口、执行器、注册机制。支持函数、方法、外部工具三种来源。

## 核心 Trait

### Tool

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    /// 执行工具。接收 JSON 字符串参数，返回字符串结果。
    async fn call(&self, args: String) -> String;

    /// 并发安全性分类。默认 ConcurrentSafe。
    fn concurrency_safety(&self) -> ConcurrencySafety { ConcurrentSafe }
}
```

### ConcurrencySafety

```rust
pub enum ConcurrencySafety {
    ConcurrentSafe,    // 可并行执行（只读操作）
    ConcurrentUnsafe,  // 必须串行执行（写操作、有状态工具）
}
```

### ToolExecutor

```rust
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    fn register(&mut self, name: String, tool: Arc<dyn Tool>);
    async fn execute(&self, calls: Vec<ToolCall>) -> Vec<ToolResult>;
    fn has(&self, name: &str) -> bool;
}
```

### ToolArgs

```rust
pub trait ToolArgs: schemars::JsonSchema + for<'a> Deserialize<'a> {
    const TOOL_NAME: &'static str;
    const TOOL_DESCRIPTION: &'static str;
    fn definition() -> ToolDefinition { ... }
}
```

由 `#[tool]` 宏自动生成，不需要手动实现。

## Executor

内置默认执行器，支持并行/串行模式。

```rust
impl Executor {
    pub fn parallel() -> Self;    // 并发安全的工具并行，不安全的串行
    pub fn sequential() -> Self;  // 全部串行
}
```

`parallel()` 将调用按 `ConcurrencySafety` 分区——safe 组并行（`join_all`），unsafe 组串行。输出按原始顺序排列。

## 工具注册

### 方式一：ToolDef builder（无宏）

```rust
let echo = ToolDef::new("echo", "Echo back input")
    .param::<String>("text", "Text to echo")
    .build(|args: String| async move { format!("echo: {}", args) });

agent.tool(echo);
```

### 方式二：#[tool] 宏（函数）

```rust
#[tool]
async fn search(query: String) -> String {
    format!("Results for: {}", query)
}

agent.tool_fn(search);
```

展开等价于：自动生成 `SearchArgs` struct、`ToolArgs` 实现、JSON Schema。

### 方式三：#[tool] 宏（impl 块）

```rust
#[tool]
impl Database {
    async fn query(self, sql: String) -> String { ... }
}

agent.bind(db.clone(), Database::query);
```

### 方式四：external_tools（远程/MCP）

```rust
let defs = vec![ToolDefinition::new("remote", "Remote tool", schema)];
agent.external_tools(defs, |name, args| {
    // 转发到 MCP 服务器
    mcp_client.call_tool(&name, &args)
});
```

## 宏属性

`#[tool]` 支持以下属性：

| 属性 | 位置 | 说明 |
|------|------|------|
| `#[tool(name = "x")]` | fn | 自定义工具名 |
| `#[serde(rename = "x")]` | 参数 | 自定义参数名 |

```rust
#[tool(name = "web_search")]
async fn search(
    #[serde(rename = "searchTerm")] query: String,
) -> String { ... }
```

## 返回值约定

- 成功：返回结果字符串
- 失败：返回 `"Error: ..."` 或以 `"Error:"` 开头的字符串
- Tool not found：Executor 返回 `"Tool 'X' not found. Available: [...]"`
- `#[tool]` 参数解析失败：返回 `"[Invalid arguments: ...]. Check and retry."`

## 替换指南

| 场景 | 实现 |
|------|------|
| 沙箱执行 | 实现 `ToolExecutor`，在 Docker/WASM 中运行工具 |
| 远程执行 | 实现 `ToolExecutor`，通过 RPC 转发 |
| 带超时的执行 | 实现 `ToolExecutor`，用 `tokio::time::timeout` 包裹 |
| 自定义执行顺序 | 实现 `ToolExecutor::execute()` |
