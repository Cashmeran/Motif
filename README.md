# Motif

2500 行的 Rust agent 核心。库优先，全 trait 注入，带 CLI。

```bash
cargo install motif
motif
```

```rust
use motif::*;

#[tool]
async fn search(query: String) -> String {
    format!("搜索 {} 的结果", query)
}

let mut agent = Agent::new(OpenAIProvider::new(
    "https://api.deepseek.com/v1", "sk-...", "deepseek-chat",
))
.model("deepseek-chat")
.tool_fn(search);

let response = agent.chat("搜索 Rust agent 框架").await?;
```

## 这是什么

Motif 只做一件事：循环、工具、历史、提示词。别的什么都不做。

没有内建文件操作。没有代码搜索。没有记忆系统。没有 Web UI。这些每个都是独立的 crate（或者一个 20 行的 `PromptBuilder` 实现），依赖 Motif 而不是反过来。

核心 9 个源文件，一小时能读完。

## 独特之处

### 1. 终止条件是可配置策略——不是硬编码

`OnText`（无工具调用时停）、`AfterNTools`（N 条工具结果后停）、`OnStuck`（重复相同调用时停）、`Never`（你自己控制循环）、`Custom`（你的谓词）。

```rust
// 不加核心代码，实现验证循环：
agent.stop_when(StopCondition::Custom(Arc::new(|resp, _history| {
    resp.message.content.contains("通过验证")
})));
```

没有其他轻量 agent 做到了这一点。tiny-loop 硬编码了退出条件。nanobot 跑到 max_iterations 为止。Aegis 文本或出错就退出。Motif 让你自己定义"完成"是什么。

### 2. 九个生命周期 Hook——全是空操作，直到你需要

`before_llm`、`after_llm`、`before_tools`、`after_tools`、`before_run`、`after_run`、`on_error`、`on_stream_delta`、`finalize_content`。

要日志？`before_llm` + `tracing`。要记忆注入？`before_llm` + 检索。要后处理？`finalize_content`。一个 Hook 报错不影响其他。

### 3. 提示词是 3 层，各自独立缓存

L0（身份）是 9 节——元规则、角色、节奏、语调、诚实、安全、工具使用、幻觉预防、执行纪律。指纹缓存。L1（工具 JSON）搭在上面，工具变化时重建。L2（PromptBuilder 扩展）每轮动态。日期放用户消息里，不动系统提示——保 L0+L1 缓存稳定。

### 4. 所有依赖都是 trait，全部可替换

| trait | 你会换成什么 |
|-------|------------|
| `LLMProvider` | Anthropic、Ollama、mock、路由器 |
| `History` | 有界、SQLite、token 感知 |
| `Tool` / `ToolExecutor` | 沙箱、远程、MCP 桥 |
| `AgentHook` | 日志、记忆、安全护栏 |
| `PromptBuilder` | 文件树、git 状态、技能列表 |

### 5. `#[tool]` 宏支持函数、方法和 impl 块

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

Schema 自动生成——doc 注释变成 JSON 描述，类型变成参数。

## 架构

```
src/
├── agent.rs      Agent 循环 + step()/run()/chat() + 5 种终止条件
├── provider.rs   LLMProvider trait + OpenAIProvider + 重试 + SSE 流
├── tool.rs       Tool trait + Executor + ConcurrencySafety + ToolDef
├── history.rs    History trait + InfiniteHistory + BoundedHistory
├── prompt.rs     3 层缓存提示词 + PromptBuilder trait
├── hooks.rs      AgentHook（9 个方法，错误隔离）
├── types.rs      Message、ToolCall、LLMResponse、TokenUsage
├── error.rs      Error 枚举
├── lib.rs        重新导出
└── main.rs       CLI 二进制文件
```

## 对比

| | Motif | tiny-loop | nanobot | Aegis |
|---|-------|-----------|---------|-------|
| 语言 | Rust | Rust | Python | Rust |
| 核心大小 | ~2,500 行 | ~920 行 | ~15,000 行 | ~165,000 行 |
| 终止条件 | 5 种可配置 | 1 种硬编码 | 1 种硬编码 | 1 种硬编码 |
| Hook | 9 个方法 | 无 | 12 个方法 | 无 |
| 工具宏 | `#[tool]` fn/impl | `#[tool]` fn | decorator | 手动 trait |
| 提示词缓存 | 3 层指纹 | 无 | Jinja2（不缓存） | 3 层 SHA256 |
| Provider 重试 | 429/5xx | 无 | 3 种模式 | 有 |
| Trait 注入 | 全部 6 个核心 | provider+tool+history | 插件系统 | 40 参数构造器 |
| CLI | 内建 | examples/ | gateway | 内建 |

## 测试

51 mock + 13 live（真实 DeepSeek API）。零 unsafe。

```bash
cargo test                      # 51 mock
MOTIF_API_KEY=sk-... cargo test -- --ignored   # +13 live
```

## 安装

**一行命令安装：**

Windows PowerShell:
```powershell
irm https://raw.githubusercontent.com/Cashmeran/Motif/main/install.ps1 | iex
```

Linux / macOS:
```bash
curl -fsSL https://raw.githubusercontent.com/Cashmeran/Motif/main/install.sh | bash
```

**或从源码安装：**
```bash
cargo install --git https://github.com/Cashmeran/Motif.git
```

## 使用

```bash
motif                        # 首次运行输入 API key，保存至 ~/.motif/config.json
```

直接对话。

换 provider 就编辑 `~/.motif/config.json`：

```json
{
  "api_key": "sk-...",
  "base_url": "https://api.openai.com/v1",
  "model": "gpt-4o-mini"
}
```

## License

MIT
