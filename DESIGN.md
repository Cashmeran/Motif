# Motif — 设计计划书

> **⚠️ 历史档案** —— 本文档记录 v0.1 的初始设计，仅保留作参考。  
> v0.1 范围中标注"排除"的特性（`#[tool]` proc-macro、流式 streaming、方法级 bind、  
> Anthropic 格式、FileHistory、CLI、motif-tools 等）已全部在 v0.2 中实现。  
> 当前代码结构已从单 crate 演化为 workspace（motif + macros + motif-cli + motif-tools + motif-session）。  
> 最新状态以源码和 `docs/` 下的文档为准。
>
> 版本: v0.1  
> 日期: 2026-06-17  
> 定位: 极简、可扩展的 Rust Agent 核心库  
> 状态: 已实现（代码已远超此文档范围）

---

## 一、起源与教训

Motif 诞生于两次失败的经验总结：

| 项目 | 规模 | 失败原因 |
|------|------|---------|
| 蜂群系统 (Swarm Engine) | ~5,200行 Python | 多Agent协调复杂度固有，边际收益递减 |
| DeepSeek-Aegis | ~165,000行 Rust | 规模失控，外围功能与核心耦合，个人无法维护 |

**核心洞察**: 轻量化核心 + 外挂扩展是唯一可持续的架构。Core做减法，扩展做加法。

---

## 二、参考项目调研结论

### 2.1 tiny-loop (Rust) — 主要参考

| 维度 | 设计 |
|------|------|
| 源文件 | 8个 (agent, llm, tool, history, types, error + macros) |
| Agent结构 | `history: Box<dyn History>` + `llm: Box<dyn LLMProvider>` + `executor: Box<dyn ToolExecutor>` |
| 工具注册 | `#[tool]` 过程宏 → 生成 Args struct + ToolArgs impl |
| 三种来源 | `.tool(fn)` / `.bind(ins, method)` / `.external(defs, exec)` |
| Loop控制 | `step()` 返回 `Option<String>`，外部控制循环 |

**待改进**: 无Hook系统、无SystemPrompt Builder链、无流式支持、终止条件硬编码。

### 2.2 nanobot (Python) — Hook系统参考

12个生命周期方法的 AgentHook + CompositeHook 错误隔离 + AgentRunSpec 配置模式。
**应避免**: AgentLoop 1844行过于庞大，状态机+消息总线+文件追踪+工作区隔离全耦合。

### 2.3 论文调研 — 验证我们的设计

**简单循环是对的**: LangGraph/CrewAI/AutoGen/Claude Code/Letta 六大框架对照。循环本身不构成差异化——差异化在状态管理、终止条件、上下文压缩和子任务委托。

**自反思有害（除非用工具验证）**: Snorkel AI 2025 实证——简单任务上自批判让准确率从 98% 跌到 57%。验证必须是外挂工具，不内建。

**工具按需发现**: MCP-Zero (2025) 证明 agent 主动请求工具比全量预加载节省 98% token。

**显式计划优于隐式上下文**: 70个系统调研结论——任务分解应作为结构化产出，不是埋进 prompt。

**卡住检测**: Focused ReAct (2024) 实证——检测重复动作+每步重申目标，小模型准确率提升 530%。

---

## 三、六个核心模块

| # | 模块 | 职责 | trait/类型 |
|---|------|------|-----------|
| ① | Agent Loop | 生命周期管理，step/run/chat | `Agent` struct |
| ② | System Prompt | 身份定义，Builder链追加 | `SystemPrompt` + Builder |
| ③ | Tool System | 工具注册、执行调度 | `Tool`, `ToolExecutor`, `ToolDef` |
| ④ | Message State | 对话历史累积 | `History` trait, `InfiniteHistory` |
| ⑤ | LLM Provider | 大模型调用抽象，流式+非流式 | `LLMProvider` trait |
| ⑥ | Loop Hooks | 全生命周期可观察/可拦截 | `AgentHook` trait |

---

## 四、项目结构 (单Crate，平级目录)

```
motif/
├── Cargo.toml
├── src/
│   ├── lib.rs          # prelude, 公开API
│   ├── agent.rs        # Agent struct, step/run/chat, stop_when
│   ├── provider.rs     # LLMProvider trait + OpenAI实现
│   ├── tool.rs         # Tool trait, ToolExecutor, ToolDef, 注册方法
│   ├── history.rs      # History trait, InfiniteHistory
│   ├── prompt.rs       # SystemPrompt, Builder链
│   ├── hooks.rs        # AgentHook trait, CompositeHook
│   ├── types.rs        # Message, ToolCall, ToolResult, LLMResponse, FinishReason
│   └── error.rs        # Error enum, Result type
└── tests/
    └── integration.rs
```

---

## 五、关键Trait设计

### 5.1 LLMProvider

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn call(&self, messages: &[Message], tools: &[ToolDefinition]) -> Result<LLMResponse>;
    async fn call_stream(&self, messages: &[Message], tools: &[ToolDefinition]) -> Result<LLMStream>;
    fn supports_streaming(&self) -> bool { false }
}
```

### 5.2 History

```rust
pub trait History: Send + Sync {
    fn add(&mut self, message: TimedMessage);
    fn add_batch(&mut self, messages: Vec<TimedMessage>) { /* default loop */ }
    fn get_all(&self) -> &[TimedMessage];
    fn clear(&mut self);
}
```

### 5.3 Tool + ToolExecutor

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    async fn call(&self, args: String) -> String;
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    fn add(&mut self, name: String, tool: Arc<dyn Tool>);
    async fn execute(&self, calls: Vec<ToolCall>) -> Vec<ToolResult>;
}
```

### 5.4 AgentHook

```rust
#[async_trait]
pub trait AgentHook: Send + Sync {
    async fn before_run(&self, ctx: &mut RunContext) -> Result<()> { Ok(()) }
    async fn after_run(&self, ctx: &mut RunContext) -> Result<()> { Ok(()) }
    async fn before_llm(&self, ctx: &mut HookContext) -> Result<()> { Ok(()) }
    async fn after_llm(&self, ctx: &mut HookContext) -> Result<()> { Ok(()) }
    async fn before_tools(&self, ctx: &mut HookContext) -> Result<()> { Ok(()) }
    async fn after_tools(&self, ctx: &mut HookContext) -> Result<()> { Ok(()) }
    async fn on_stream_delta(&self, delta: &str) -> Result<()> { Ok(()) }
    async fn on_error(&self, ctx: &mut HookContext, error: &Error) -> Result<()> { Ok(()) }
    fn finalize_content(&self, content: &str) -> String { content.to_string() }
}
```

### 5.5 stop_when

```rust
impl Agent {
    pub fn stop_when(mut self, condition: StopCondition) -> Self;
}

pub enum StopCondition {
    OnText,                          // finish_reason != ToolCalls (默认)
    AfterNTools(usize),              // 执行N轮工具后停止
    OnStuck { max_repeats: usize },  // 连续重复动作早停 (Focused ReAct)
    Never,                           // 永不自动停止
    Custom(Arc<dyn Fn(&LLMResponse, &[TimedMessage]) -> bool + Send + Sync>),
}
```

---

## 六、v0.1 范围

**目标**: 最小可验证核心

| 包含 | 排除 |
|------|------|
| 六个核心模块完整实现 | CLI 程序 |
| `LLMProvider` trait + OpenAI兼容实现 | 方法级 `#[tool]`，具体Hook实现 |
| `Tool` trait + `InfiniteHistory` | 压缩/Capped History，子Agent |
| `AgentHook` trait + `CompositeHook` | 复杂的Loop策略 |
| `stop_when` 含 `OnStuck` | 流式streaming (v0.1用call，v0.2加call_stream) |
| `Agent::step()`, `run()`, `chat()` | `#[tool]` 过程宏 |
| 完整单元测试 + 集成测试 | 性能基准 |

### 验收标准

1. `cargo test` 全部通过
2. 集成测试验证:
   - 注册工具 → agent.chat("用工具做X") → 工具被调用
   - 注册 external tool → 外部工具结果注入
   - stop_when → 自定义终止条件生效
   - hook → 生命周期回调被触发
   - LLM mock → 完整 Agent Loop 不依赖真实 API

---

## 七、设计原则

1. **核心不假设使用场景** — 只做Agent循环，不做编码/聊天/审查
2. **Trait隔离一切** — 每个能力一个trait，注入不绑定
3. **step() 是原子单位** — 外部控制循环，内部不藏策略
4. **Hook全覆盖** — 所有外挂通过Hook插入
5. **默认即用，皆可替换** — 合理默认值 + 全部可配置
6. **零超出依赖** — tokio + serde + serde_json + reqwest + async-trait + thiserror + schemars + futures + tracing，每个都有明确理由
