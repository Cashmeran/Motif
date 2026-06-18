# History

管理 Agent 的对话记忆。所有历史操作通过同一个 trait 进行，后端可替换。

## Trait 定义

```rust
pub trait History: Send + Sync {
    /// 追加一条消息
    fn add(&mut self, message: TimedMessage);
    /// 按时间顺序返回所有消息
    fn get_all(&self) -> &[TimedMessage];
    /// 清空所有消息
    fn clear(&mut self);
}
```

## TimedMessage

```rust
pub struct TimedMessage {
    pub message: Message,           // System | User | Assistant | Tool
    pub timestamp: SystemTime,      // 消息添加时间
    pub elapsed: Duration,          // LLM 调用耗时
}
```

## 内建实现

### InfiniteHistory

无限内存增长，默认实现。

```rust
let mut h = InfiniteHistory::new();
h.add(TimedMessage::new(Message::user("hello")));
let all = h.get_all(); // [hello]
```

**适用**：1M 上下文模型，或者对话轮次少于 20 的场景。

### BoundedHistory

容量限制，超出时丢弃最旧的非系统消息。系统消息（第一条）永久保留。

```rust
let mut h = BoundedHistory::new(60); // 最多 60 条
h.add(TimedMessage::new(Message::system("prompt")));
h.add(TimedMessage::new(Message::user("a")));
h.add(TimedMessage::new(Message::user("b")));
// ...添加更多，最旧的用户消息被丢弃
```

**适用**：小上下文模型，或者需要严格控制 token 的场景。

## FileHistory（motif-session）

磁盘持久化实现，在 `motif-session` crate 中。

```rust
// 可选依赖 motif-session
use motif_session::FileHistory;

let h = FileHistory::new(None);           // 新会话，自动生成 ID
let old = FileHistory::load("abc123").unwrap(); // 恢复旧会话
```

详见 [session.md](../session.md)。

## 替换指南

| 场景 | 实现 |
|------|------|
| SQLite 持久化 | 实现 `History`，`add()` → INSERT，`get_all()` → SELECT |
| Token 感知裁剪 | 实现 `History`，`add()` 时累计 token 数，超出预算时裁剪 |
| 远程存储 | 实现 `History`，通过 HTTP/GRPC 读写 |
| 多级记忆 | 实现 `History`，内部维护短期+长期两个缓冲区 |
