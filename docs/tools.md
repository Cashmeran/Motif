# Tools

6 个通用工具，位于 `motif-tools` crate。每个导出 `register() -> RegisteredTool` 函数。

## 安装

```bash
cargo add motif-tools
```

## 注册

```rust
use motif_tools::{search, read, write, edit, web_fetch, bash};

agent
    .tool(search::register())
    .tool(read::register())
    .tool(write::register())
    .tool(edit::register())
    .tool(web_fetch::register());
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

### Glob 语法

- `*` — 匹配单个路径组件内的任意字符（不跨 `/`）
- `**` — 跨目录边界匹配（`**/` 匹配零或多级目录，`**` 匹配一切）
- `?` — 匹配单个非 `/` 字符
- `{a,b,c}` — 花括号展开（如 `*.{rs,toml}` 匹配 `.rs` 和 `.toml`）
- `glob` 参数无路径分隔符时仅匹配文件名；包含 `/` 或 `**` 时匹配完整路径

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
- 每次成功读取自动记录 mtime，用于 edit/write 的**读后编辑强制**

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
- **读后编辑强制**：已存在的文件必须被 read 工具读过后才能写入（新文件不受限）

---

## edit —— 精确字符串替换

基于唯一匹配的安全文件编辑。`old_string` 必须在文件中恰好出现一次，否则拒绝编辑。

### 参数

| 参数 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `file_path` | string | —（必填） | 编辑文件路径 |
| `old_string` | string | —（必填） | 要替换的精确字符串，必须在文件中唯一 |
| `new_string` | string | —（必填） | 替换后的字符串 |
| `replace_all` | bool | `false` | 替换所有匹配项（绕过唯一性检查） |

### 特殊行为

- **空 old_string**：完整覆写文件（等同于 write）
- **old_string == new_string**：幂等拒绝，不做任何修改
- **唯一性检查**：非 `replace_all` 模式下，`old_string` 出现 ≠ 1 次则拒绝
- **引号规范化**：自动处理直引号/弯引号的差异（`"` ↔ `\u{201c}` / `\u{201d}`，`'` ↔ `\u{2018}`），双向尝试

### 安全限制

- 最大文件大小：1 MiB
- 最大 old_string 长度：10,000 字符
- **读后编辑强制**：文件必须先被 read 工具读取才能编辑
- 拒绝路径遍历

---

## web_fetch —— HTTP 内容获取

HTTP GET 请求，自动提取和格式化内容。HTML 页面转为纯文本，JSON 响应美化为缩进格式。

### 参数

| 参数 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `url` | string | —（必填） | HTTP/HTTPS URL |
| `timeout_ms` | int | `15000` | 超时（毫秒） |

### 内容处理

| 输入类型 | 处理方式 |
|----------|---------|
| HTML | 标签剥离，保留块级换行，HTML 实体解码 |
| JSON | `serde_json::to_string_pretty` 美化 |
| 纯文本 | 原样返回 |

### 安全保护

- **SSRF 防护**：解析到私有 IP（loopback、private、link-local）时拒绝
- **跨域重定向拦截**：最终 URL 的 host 与原始 host 不同时拒绝
- 最大响应体：1 MiB
- 最大重定向次数：5

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

**破坏性模式拦截**（大小写不敏感）：`rm -rf`、`rm -r`、`rmdir`、`sudo`、`su`、`chmod 777`、`mkfs.`、`dd if=`、`shutdown`、`reboot`、`halt`、`poweroff`、`git push --force`/`-f`、fork bomb、`> /dev/sda`、Zsh 危险内置（`zmodload`、`emulate`、`sysopen`、`ztcp`、`zpty`）。

**未引用元字符检测**（逐字符追踪引号状态）：

| 模式 | 检测对象 |
|------|---------|
| `$VAR`、`$()`、`${}` | 未引用的 Shell 变量/命令展开 |
| `` `cmd` `` | 反引号命令替换 |
| `?`、`*` | 未引用的 glob 通配符 |

单引号内的元字符安全放行（如 `'*.rs'`、`'$VAR'`），转义的也放行（`\$var`）。双引号内的 `$` 仍会展开，不豁免。

### 行为

- Windows：`cmd /C`，Unix：`sh -c`
- `kill_on_drop`：Agent 停止时自动杀子进程
- 输出截断：50K 字符后截断
- 超时：返回 `"Command timed out after Xs"`
