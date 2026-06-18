# Motif

Rust agent 核心库，2500 行。库优先，全 trait 注入。附带 CLI。

```bash
cargo install --git https://github.com/Cashmeran/Motif.git
```

## 使用

```bash
motif
```

首次运行时输入 API key，自动保存至 `~/.motif/config.json`，之后无需重复配置。更换 provider 直接编辑该文件：

```json
{
  "api_key": "sk-...",
  "base_url": "https://api.openai.com/v1",
  "model": "gpt-4o-mini"
}
```

作为库使用：

```rust
use motif::*;

#[tool]
async fn search(query: String) -> String {
    format!("搜索结果：{}", query)
}

let mut agent = Agent::new(OpenAIProvider::new(
    "https://api.deepseek.com/v1", "sk-...", "deepseek-chat",
))
.model("deepseek-chat")
.tool_fn(search);

let response = agent.chat("搜索 Rust agent 框架").await?;
```

## 设计原则

Motif 只包含 agent 核心：循环、工具、历史、提示词。文件操作、代码搜索、记忆系统、Web UI 均为独立 crate，依赖 Motif 而非反之。核心共 9 个源文件。

## 核心特性

### 可配置的终止条件（5 种）

`OnText`、`AfterNTools`、`OnStuck`、`Never`、`Custom`。tiny-loop 硬编码退出条件，nanobot 仅按 max_iterations 退出，Aegis 在文本或出错时退出。Motif 允许外部定义"任务完成"的判定逻辑。

```rust
agent.stop_when(StopCondition::Custom(Arc::new(|resp, _history| {
    resp.message.content.contains("VERIFIED")
})));
```

### 生命周期 Hook（9 个方法）

`before_llm`、`after_llm`、`before_tools`、`after_tools`、`before_run`、`after_run`、`on_error`、`on_stream_delta`、`finalize_content`。全部默认空操作，按需实现。单个 hook 的错误不影响其他 hook 执行。

### 三层提示词缓存

L0：9 节身份定义（元规则、角色、节奏、语调、诚实、安全、工具使用、幻觉预防、执行纪律），指纹缓存，永不变化。L1：工具 JSON，注册时重建。L2：PromptBuilder 扩展，每轮注入。日期注入用户消息而非系统提示，保持 L0+L1 缓存稳定。

### 全部 trait 注入

| trait | 可替换为 |
|-------|---------|
| `LLMProvider` | Anthropic、Ollama、mock、路由 |
| `History` | 有界、SQLite、token 感知 |
| `Tool` / `ToolExecutor` | 沙箱、远程、MCP 桥 |
| `AgentHook` | 日志、记忆、护栏 |
| `PromptBuilder` | 文件树、git 状态、技能列表 |

### `#[tool]` 过程宏

支持函数、方法和 impl 块。doc 注释自动生成 JSON Schema 描述。

```rust
#[tool]
async fn weather(city: String) -> String { ... }

#[tool]
impl Database {
    async fn query(self, sql: String) -> String { ... }
}

agent.tool_fn(weather);
agent.bind(db, Database::query);
```

## 架构

```
src/
├── agent.rs      Agent loop + step/run/chat + 5 种 StopCondition
├── provider.rs   LLMProvider trait + OpenAIProvider + retry + SSE streaming
├── tool.rs       Tool trait + Executor + ConcurrencySafety + ToolDef
├── history.rs    History trait + InfiniteHistory + BoundedHistory
├── prompt.rs     3 层缓存 Prompt + PromptBuilder trait
├── hooks.rs      AgentHook (9 方法，错误隔离)
├── types.rs      Message, ToolCall, LLMResponse, TokenUsage
├── error.rs      Error 枚举
├── lib.rs        公共 API 导出
└── main.rs       CLI
```

## 对比（轻量级 Agent 框架）

| | Motif | tiny-loop | nanobot | pi-mono |
|---|-------|-----------|---------|---------|
| 语言 | Rust | Rust | Python | TypeScript |
| 代码量 | ~2,500 行 | ~920 行 | ~15,000 行 | ~120,000 行 |
| 定位 | 通用核心库 | 通用核心库 | 全栈 agent | 编码 agent |
| 终止条件 | 5 种可配置 | 1 种 | 1 种 | 1 种 |
| Hook 系统 | 9 方法 | 无 | 12 方法 | 有 |
| 工具宏 | `#[tool]` fn+impl | `#[tool]` fn | decorator | 手动注册 |
| 提示词缓存 | 3 层指纹 | 无 | Jinja2 | 无 |
| Provider | OpenAI 系列 | OpenAI 系列 | 10+ provider | 10+ provider |
| Trait 注入 | 全部 6 核心 | 3 个 trait | 插件系统 | DI 容器 |
| CLI | 内建 | examples/ | gateway | TUI

## 测试

51 mock + 13 live（真实 DeepSeek API 调用）。零 unsafe。

```bash
cargo test                                    # 51 mock
MOTIF_API_KEY=sk-... cargo test -- --ignored  # +13 live
```

## License

MIT
