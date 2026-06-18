# Motif 项目架构

## 原则

**核心只做一件事：定义接口。** 零 I/O，零工具，零 CLI 依赖。

依赖箭头永远指向核心。核心不指向任何人。

```
motif-tools  ──→  motif  ←──  motif-cli
                    ↑
               motif-session
```

---

## 文件结构

```
Motif/                              # workspace（只有 Cargo.toml）
│
├── motif/                          # 纯核心 crate（零 I/O / 零工具 / 零 CLI）
│   ├── Cargo.toml                  # 8 个依赖，无 rustyline/dirs/chrono/fs
│   ├── src/
│   │   ├── lib.rs                  # pub use re-export all
│   │   ├── agent.rs                # Agent + step/run/chat + StopCondition (5种)
│   │   ├── provider.rs             # LLMProvider trait + OpenAIProvider
│   │   ├── tool.rs                 # Tool trait + Executor + ToolDef + ToolArgs
│   │   ├── history.rs              # History trait + InfiniteHistory + BoundedHistory
│   │   ├── prompt.rs               # Prompt (3层缓存) + PromptBuilder trait
│   │   ├── hooks.rs                # AgentHook trait (9生命周期)
│   │   ├── types.rs                # Message / ToolCall / LLMResponse / TokenUsage
│   │   └── error.rs                # Error + Result
│   └── tests/
│       └── integration.rs
│
├── macros/                         # proc-macro crate（Rust 硬约束）
│   ├── Cargo.toml                  # proc-macro = true
│   └── src/lib.rs                  # #[tool] macro（fn + impl + name attr）
│
├── motif-cli/                      # CLI 产品（独立 crate）
│   ├── Cargo.toml                  # 依赖 motif + rustyline + dirs + serde + …
│   └── src/
│       ├── main.rs                 # 主循环 + agent 创建
│       ├── config.rs               # ~/.motif/config.json 读写
│       ├── commands.rs             # Command trait + Registry
│       ├── keybind.rs              # 快捷键骨架
│       └── cmd/                    # 各命令
│           ├── mod.rs
│           ├── clear.rs
│           ├── help.rs
│           ├── status.rs
│           ├── list.rs
│           └── load.rs
│
├── motif-tools/                    # 通用工具包（独立 crate）
│   ├── Cargo.toml                  # 依赖 motif + regex
│   └── src/
│       ├── lib.rs                  # pub use
│       ├── search.rs               # grep + glob 合一
│       ├── read.rs                 # 文件读取（offset/limit）
│       ├── write.rs                # 文件写入（保护文件检测）
│       └── bash.rs                 # 命令执行（超时 + 危险命令检测）
│
└── motif-session/                  # 会话持久化（独立 crate）
    ├── Cargo.toml                  # 依赖 motif + chrono + dirs
    └── src/
        └── lib.rs                  # FileHistory（JSONL 增量写 + fsync）
```

---

## 不存在的 crate（刻意不做）

| crate | 为什么不 |
|-------|---------|
| `motif-mcp` | MCP 客户端太沉（OAuth/传输/重连），不是 Motif 该做的事 |
| `motif-skill` | Skill 解析简单到不需要框架——PromptBuilder 闭包三行 |
| `motif-memory` | 记忆检索是 Hook 的实现，不需要专用 crate |
| `motif-tui` | TUI 是产品层的事，不是框架层 |
| `motif-desktop` | 桌面端和核心无关 |
| `motif-anthropic` | Anthropic SDK 成熟的绑定还没有 |

---

## 依赖债务表

| crate | 依赖数 | 为什么这么多 |
|-------|:--:|------|
| `motif` | **8** | async-trait, chrono, futures, reqwest, schemars, serde, thiserror, tokio |
| `motif-cli` | +4 | +rustyline, dirs, serde, serde_json |
| `motif-tools` | +1 | +regex |
| `motif-session` | +1 | +chrono, dirs（chrono 已由 motif 依赖） |

---

## 外挂清单（只开接口，不写实现）

| 接口 | trait | 谁来实现 |
|------|-------|---------|
| MCP 桥接 | Tool（via external_tools） | 用户自己 |
| Skill 注入 | PromptBuilder | 用户自己 |
| 上下文压缩 | AgentHook（before_llm） | 用户自己 |
| 记忆检索 | AgentHook（before_llm） | 用户自己 |
| 日志/监控 | AgentHook（所有生命周期） | 用户自己 |
| 沙箱执行 | ToolExecutor | 用户自己 |
| 多 Provider 路由 | LLMProvider | 用户自己 |
