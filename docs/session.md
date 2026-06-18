# Session (FileHistory)

磁盘持久化会话历史，位于 `motif-session` crate。实现 `History` trait。

## 安装

```bash
cargo add motif-session
```

## 架构

```
~/.motif/sessions/
├── a1b2c3d4e5f6.jsonl    ← 每行 JSON：一条 TimedMessage
├── b2c3d4e5f6a7.jsonl
├── index.json             ← 快速索引 [{id, date, count, first}]
└── latest                 ← 符号链接 → 当前会话文件
```

## 使用

### 新建会话

```rust
use motif_session::FileHistory;

let agent = Agent::new(provider)
    .history(FileHistory::new(None));  // 自动生成 12 位 session ID
```

### 恢复会话

```rust
let history = FileHistory::load("a1b2c3d4e5f6").unwrap();
let agent = Agent::new(provider).history(history);
```

### 列出会话

```rust
let sessions = FileHistory::list();
for s in sessions {
    println!("{} {} {} msgs", s["id"], s["date"], s["count"]);
}
```

## 文件格式 (JSONL)

每行一个 JSON 对象，LF 分隔：

```
{"_meta":true,"created":"2026-06-19 15:00"}
{"message":{"role":"user","content":"[Runtime Context] ..."},"timestamp":{...},"elapsed":{...}}
{"message":{"role":"assistant","content":"Hello!"},"timestamp":{...},"elapsed":{...}}
```

- 第 1 行：元数据（`_meta: true`）
- 后续行：`TimedMessage` 序列化
- 加载时过滤元数据行和空行

## 原子性

- 每个 `add()` 写入一行 + `flush()`
- `index.json` 覆写
- Unix：`latest` 使用符号链接
- Windows：`latest` 写入包含目标文件名的纯文本文件

## API

```rust
impl FileHistory {
    pub fn new(session_id: Option<&str>) -> Self;
    pub fn load(id: &str) -> Option<Self>;
    pub fn list() -> Vec<serde_json::Value>;
    pub fn session_id(&self) -> &str;
}

impl History for FileHistory { ... }
```
