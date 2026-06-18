# Error

统一的错误类型，`thiserror` 派生。

## 枚举定义

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("LLM API error ({status}): {body}")]
    ApiError { status: u16, body: String },

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Tool '{name}' not found. Available: {available:?}")]
    ToolNotFound { name: String, available: Vec<String> },

    #[error("{0}")]
    Custom(String),
}
```

## 变体说明

| 变体 | 何时出现 | 恢复建议 |
|------|---------|---------|
| `ApiError` | Provider 收到非 2xx 状态码 | 检查 api key、余额、base_url |
| `Http` | 网络不可达、超时 | retry 策略自动处理 429/5xx |
| `Json` | API 响应解析失败 | 罕见；检查 API 版本兼容性 |
| `ToolNotFound` | LLM 调用了未注册的工具名 | `external_tools` 的 handler 返回错误信息 |
| `Custom` | 用户自定义错误 | `Error::from` + `thiserror` |

## Result 别名

```rust
pub type Result<T> = std::result::Result<T, Error>;
```

## Clone 实现

`reqwest::Error` 和 `serde_json::Error` 不实现 `Clone`。手动 `impl Clone for Error` 将它们转为 `Custom` 或直接复制。

## 使用

```rust
fn my_func() -> motif::Result<String> {
    Err(Error::Custom("something went wrong".into()))
}
```
