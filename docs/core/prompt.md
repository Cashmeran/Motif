# Prompt

3 层缓存系统提示词，指纹保护。

## 架构

```
┌─ L0: 9 节身份定义（缓存，永不变化）──────────────────────┐
│  Meta · Identity · Rhythm · Voice · Honesty · Safety    │
│  Tool Use · Hallucination · Execution                   │
├─ L1: 工具 JSON（缓存，注册工具时重建）────────────────────│
│  ## Available Tools                                     │
│  ```json [{...}]```                                     │
├─ L2: PromptBuilder 扩展（每轮重建）───────────────────────│
│  技能 · 文件树 · Git 状态 · 项目上下文                      │
└─────────────────────────────────────────────────────────┘
日期/模型 → 注入用户消息开头，不进入系统提示（保缓存稳定）
```

## Prompt struct

```rust
pub struct Prompt { /* 内部：RwLock 缓存 + 指纹 */ }

impl Prompt {
    pub fn new() -> Self;
    pub fn freeze_tools(&self, json: &str);  // 冻结工具定义到 L1
    pub fn build(&self, extensions: &[String]) -> String; // 构建完整提示词
}
```

## PromptBuilder trait

外部组件实现此 trait，每轮 LLM 调用前追加提示词块。

```rust
pub trait PromptBuilder: Send + Sync {
    fn build(&self) -> Option<String>;
}
```

示例：

```rust
struct GitStatus;
impl PromptBuilder for GitStatus {
    fn build(&self) -> Option<String> {
        let status = Command::new("git").args(["status", "--short"]).output().ok()?;
        Some(format!("# Git Status\n{}", String::from_utf8_lossy(&status.stdout)))
    }
}
agent.prompt_builder(GitStatus);
```

## runtime_context()

注入用户消息开头（不在系统提示中），保持 L0+L1 缓存稳定。

```rust
// 输出格式：
// [Runtime Context] Current time: 2026-06-19 15:30 CST. Model: deepseek-v4-pro.
pub fn runtime_context(model: &str) -> String;
```

Agent::chat() 自动调用此函数，拼接在用户输入前。

## 9 个 L0 节

| 节 | 标题 | 内容要点 |
|----|------|---------|
| S1 | Meta | 规则优先级、意图判断、不泄露内部结构 |
| S2 | Identity | 你是谁、协作者定位、不负面假设用户 |
| S3 | Rhythm | 不套话开头/结尾、不重播历史、语言匹配 |
| S4 | Voice | 最短回答、像人一样对话、不等不挽留 |
| S5 | Honesty | 不编造、不确定就查、诚实纠正、不包装 |
| S6 | Safety | 默认帮助、不暴露秘钥、不破坏操作、不依赖 |
| S7 | Tool Use | 精确调用、并行/串行、错误分析、避免循环 |
| S8 | Hallucination | 可自信犯错、区分记忆与工具结果 |
| S9 | Execution | 确定范围、工具优先、立即行动、不重复死路 |

完整文本见 `motif/src/prompt.rs` 中的 `S1` 到 `S9` 常量。

## 缓存策略

- L0 指纹基于 9 节内容的哈希，永不变化
- L1 指纹基于工具 JSON 的哈希，`freeze_tools()` 触发重建
- L2 不缓存，每轮重建
- 读/写锁（RwLock），毒化安全（`unwrap_or_else(|e| e.into_inner())`）
