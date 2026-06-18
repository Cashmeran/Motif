# CLI

命令行接口，位于 `motif-cli` crate。独立的终端程序，消费 `motif` 核心。

## 安装

```bash
cargo install --git https://github.com/Cashmeran/Motif.git motif-cli
```

## 配置

首次运行时提示输入 API key，保存至 `~/.motif/config.json`。之后直接编辑该文件：

```json
{
  "api_key": "sk-...",
  "base_url": "https://api.deepseek.com",
  "model": "deepseek-v4-pro",
  "thinking_effort": "max",
  "extra_body": {
    "temperature": 0.7
  }
}
```

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `api_key` | —（必填） | API 密钥 |
| `base_url` | `https://api.deepseek.com` | 兼容 OpenAI 协议的基础 URL |
| `model` | `deepseek-v4-pro` | 模型标识符 |
| `thinking_effort` | `null` | DeepSeek 思考模式：`"high"` 或 `"max"` |
| `extra_body` | `null` | 任意附加请求参数字典 |

## 架构

```
motif-cli/src/
├── main.rs        ← 主循环（35 行）
├── config.rs      ← 配置加载 + agent 创建
├── commands.rs    ← Command trait + Registry
├── keybind.rs     ← 快捷键骨架
└── cmd/           ← 命令实现
    ├── help.rs
    ├── clear.rs
    ├── status.rs
    ├── list.rs
    └── load.rs
```

## 命令

| 命令 | 说明 |
|------|------|
| `/help` | 显示可用命令 |
| `/clear` | 新建会话（替换 Agent） |
| `/status` | 显示模型名、Token 数、消息数 |
| `/list` | 列出历史会话 |
| `/load <id>` | 恢复指定会话 |

## 命令系统

```rust
pub trait Command: Send + Sync {
    fn name(&self) -> &'static str;
    fn desc(&self) -> &'static str;
    async fn run(&self, agent: &mut Agent, args: &str, cfg: &Config, reg: &Registry) -> Outcome;
}

pub enum Outcome { Continue, Exit, PassToAgent(String) }
```

添加命令：在 `cmd/` 下创建文件，在 `commands.rs` 中注册一行。

## 快捷键

骨架已就位（`keybind.rs`）。当前无自定义绑定。`Ctrl+C` 退出，`Ctrl+D` 退出。

## 扩展

CLI 默认注册 3 个工具：`search`、`read`、`write`。取消注册编辑 `config.rs` 中的 `make_agent`。

要注册 `bash` 工具：
```rust
.tool(motif_tools::bash::register()) // 需手动添加
```
