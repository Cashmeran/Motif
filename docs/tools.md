# Tools

4 个通用工具，位于 `motif-tools` crate。每个导出 `register() -> RegisteredTool` 函数。

## 安装

```bash
cargo add motif-tools
```

## 注册

```rust
use motif_tools::{search, read, write, bash};

agent
    .tool(search::register())
    .tool(read::register())
    .tool(write::register());
    // bash::register() 默认不注册（安全问题），手动添加
```

---

## search —— grep + glob 合一

文件内容搜索和文件名匹配。

### 参数

| 参数 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `query` | string | —（必填） | 内容搜索时为正则，文件名搜索时为 glob |
| `mode` | string | `"filename"` | `"content"` / `"files_with_matches"` / `"count"` / `"filename"` |
| `path` | string | `"."` | 搜索起始目录 |
| `glob` | string | — | 文件名过滤（如 `"*.rs"`） |
| `ignore_case` | bool | `true` | 大小写不敏感 |
| `multiline` | bool | `false` | 多行正则匹配 |
| `head_limit` | int | `250` | 最大结果数（0 = 无限） |
| `offset` | int | `0` | 分页起始偏移 |
| `before_context` | int | `0` | 匹配前展示行数 |
| `after_context` | int | `0` | 匹配后展示行数 |
| `line_numbers` | bool | `true` | 显示行号 |

### 模式说明

| 模式 | 行为 |
|------|------|
| `content` | 显示匹配行及上下文 |
| `files_with_matches` | 只列文件名，按修改时间降序 |
| `count` | 每文件显示匹配数 |
| `filename` | glob 文件名匹配，按修改时间降序 |

### 排除规则

自动跳过以下目录和文件：

| 类别 | 项目 |
|------|------|
| VCS | `.git`, `.svn`, `.hg` |
| 构建 | `target`, `node_modules`, `build`, `dist`, `vendor` |
| 缓存 | `__pycache__`, `.cache`, `.next`, `.nuxt` |
| 隐藏 | 任何以 `.` 开头的目录 |
| 二进制 | `exe`, `dll`, `so`, `dylib`, `png`, `jpg`, `pdf`, `zip`, `class`, `pyc`, `wasm`, `mp3`, `mp4` 等 |

搜索深度上限：30 层。

---

## read —— 文件读取

带分页和安全性检查的文本读取。

### 参数

| 参数 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `file_path` | string | —（必填） | 文件路径 |
| `offset` | int | `0` | 起始行号（0-索引） |
| `limit` | int | `2000` | 最大行数 |

### 安全限制

- 最大文件大小：256KB
- 最大行数：2000
- 禁止读取：`.env`、`.gitconfig`、`id_rsa`、`.bashrc` 等
- 禁止读取设备文件：`/dev/zero`、`/dev/random`、`/proc/*/fd/*`
- 拒绝路径遍历：包含 `..` 的路径

---

## write —— 文件写入

带安全保护的文件创建/覆写。

### 参数

| 参数 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `file_path` | string | —（必填） | 写入路径 |
| `content` | string | —（必填） | 写入内容 |

### 行为

- 自动创建父目录
- 禁止写入保护文件（与 read 相同列表）
- 最大内容：1MB
- 拒绝路径遍历

---

## bash —— 命令执行

带超时和安全检测的 Shell 命令执行。

### 参数

| 参数 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `command` | string | —（必填） | 要执行的命令 |
| `timeout_ms` | int | `120000` | 超时（毫秒），最大 300000 |
| `work_dir` | string | — | 工作目录 |

### 安全检测

自动拒绝以下模式：

| 模式 | 说明 |
|------|------|
| `rm -rf`、`rm -r` | 递归删除 |
| `sudo`、`su` | 提权操作 |
| `chmod 777` | 开放权限 |
| `mkfs`、`dd if=` | 磁盘操作 |
| `shutdown`、`reboot` | 系统控制 |
| `git push --force` | 强制推送 |
| fork bomb (`(){ :\|:& };:`) | 资源耗尽 |

### 行为

- Windows：`cmd /C`，Unix：`sh -c`
- `kill_on_drop`：Agent 停止时自动杀子进程
- 输出截断：50K 字符后截断
- 超时：返回 `"Command timed out after Xs"`
