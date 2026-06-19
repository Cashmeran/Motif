# Motif 全量测试套件 — 设计文档

> 版本: v0.1
> 日期: 2026-06-19
> 定位: 独立测试 crate，全面覆盖
> 状态: 设计中

---

## 一、动机

当前测试状态：

| 维度 | 现状 | 缺口 |
|------|------|------|
| 总计 | 123 tests (110 mock + 13 live) | — |
| 零测试 | CLI、macros、types.rs、error.rs、read_state.rs | 5 个模块全黑 |
| 严重不足 | provider (1 test)、streaming (0)、Anthropic (0)、并发 (0) | 4 个核心路径裸奔 |
| 深度不足 | hooks 仅测 3/15、executor 仅测 parallel 不测 sequential | 边界/错误路径未覆盖 |

---

## 二、架构

新建 `motif-tests/` 作为 workspace 成员：

```
motif-tests/
├── Cargo.toml              ← publish = false, 依赖所有 5 个 crate
├── tests/
│   ├── common/mod.rs       ← 共享测试工具
│   ├── core/               ← 核心 crate 集成测试
│   │   ├── agent.rs        agent 生命周期、停止条件、hooks 集成
│   │   ├── provider.rs     mock HTTP: call/stream/retry/Anthropic 格式
│   │   ├── hooks.rs        15 个 hook 端到端测试
│   │   ├── types.rs        序列化/反序列化/边界值
│   │   ├── error.rs        Display/Clone/错误转换
│   │   └── prompt.rs       并发 RwLock、缓存失效、Clone 行为
│   ├── tools/              ← motif-tools 集成测试
│   │   ├── search.rs       搜索：4 种模式、分页、排除、globstar
│   │   ├── read.rs         读取：offset/limit、保护文件、设备文件
│   │   ├── write.rs        写入：创建目录、保护文件、读后编辑
│   │   ├── edit.rs         编辑：唯一性、replace_all、引号规范化、读后编辑
│   │   ├── bash.rs         命令：超时、破坏性模式、元字符检测
│   │   ├── web_fetch.rs    HTTP：SSRF、跨域重定向、HTML→text
│   │   └── read_state.rs  并发读写竞争、mtime 变更检测
│   ├── security/           ← 安全专项测试
│   │   ├── bash_injection.rs  注入绕过（编码/混淆/换行/通配符）
│   │   ├── path_traversal.rs  极端路径遍历（双编码、符号链接）
│   │   ├── ssrf.rs            全私有 IP 段（IPv4/IPv6）、DNS rebinding
│   │   └── quote_norm.rs      引号全排列、混合引号、单双引号嵌套
│   ├── stress/             ← 压力/并发测试
│   │   ├── concurrent.rs      tokio::spawn 多 agent、读后编辑竞争
│   │   ├── large_io.rs        大文件读写（256KB 边界）、超长参数
│   │   └── long_running.rs    500 次迭代、OnStuck 边界、内存增长
│   ├── cli/                ← CLI 集成测试
│   │   ├── commands.rs        命令路由、参数解析
│   │   └── config.rs          配置加载、JSON 解析错误
│   ├── macros/             ← proc-macro 测试
│   │   └── tool_macro.rs      #[tool] fn/impl/name/rename、编译失败
│   └── live/               ← 真实 API 测试（#[ignore]）
│       └── api.rs             基础对话、工具调用、流式、token 统计
```

---

## 三、共享测试工具 (`tests/common/mod.rs`)

### MockProvider
```rust
pub struct MockProvider {
    responses: Mutex<Vec<LLMResponse>>,  // 顺序返回
    call_count: Mutex<usize>,
    last_request: Mutex<Option<(Vec<Message>, Vec<ToolDefinition>)>>,
}

impl MockProvider {
    pub fn new(responses: Vec<LLMResponse>) -> Self;
    pub fn call_count(&self) -> usize;
    pub fn last_request(&self) -> Option<(Vec<Message>, Vec<ToolDefinition>)>;
}
```

### Text 工厂函数
```rust
pub fn text(content: &str) -> LLMResponse { ... }
pub fn tool_call(name: &str, args: &str, id: &str) -> LLMResponse { ... }
pub fn length_response() -> LLMResponse { ... }  // FinishReason::Length
```

### TempFile 工具
```rust
pub struct TempFile { path: String }
impl TempFile {
    pub fn new(name: &str, content: &str) -> Self;
    pub fn path(&self) -> &str;
    pub fn read(&self) -> String;
}
impl Drop for TempFile { fn drop(&mut self) { fs::remove_file(&self.path).ok(); } }
```

### TempDir 工具（并发测试用）
```rust
pub struct TempDir { path: String }
impl TempDir { pub fn new(prefix: &str) -> Self; pub fn path(&self) -> &str; }
impl Drop for TempDir { fn drop(&mut self) { fs::remove_dir_all(&self.path).ok(); } }
```

---

## 四、各模块测试清单

### 4.1 core/agent.rs — 迁移 + 补充

**从 motif/tests/integration.rs 迁移**（25 个 mock 测试）：
- test_full_agent_lifecycle
- test_multiple_tools_in_one_turn
- test_external_tool_integration
- test_stop_condition_after_n_tools
- test_custom_stop_condition
- test_system_prompt_injected
- test_prompt_builder_extension
- test_tool_macro_registration
- test_tool_impl_block
- test_empty_user_message
- test_unicode_in_tool_args
- test_tool_returns_error_string
- test_tool_receives_malformed_json
- test_multi_round_conversation
- test_stop_condition_never_requires_external_control
- test_on_stuck_exact_boundary
- test_empty_response_retry_limit
- test_length_continuation
- test_tool_not_found_message_includes_available
- test_many_tools_registered
- test_many_parallel_tool_calls
- test_mixed_concurrency_safety
- test_agent_reuse_same_history
- test_bounded_history_with_agent
- test_bounded_history_preserves_system

**新增**：
- test_hooks_all_lifecycle_called — 注册一个记录型 hook，验证 15 个方法全部被调用
- test_on_message_filter_discards — hook::on_message 返回 false 则消息不入历史
- test_on_stop_check_gate — hook::on_stop_check 返回 false 则覆盖退出判定
- test_max_iterations_zero_unlimited — max_iterations=0 时循环由 stop_condition 控制
- test_agent_stream_chat_path — chat_stream() 端到端（mock provider 返回流）
- test_provider_returns_error — LLMProvider::call 返回 Err 时 agent 行为
- test_tool_executor_sequential — Sequential executor 保持调用顺序

### 4.2 core/provider.rs — 全新

使用 mock HTTP server（`tiny_http` 或内联 TCP listener）：

**重试逻辑**：
- test_retry_on_429_within_limit — 前 2 次 429、第 3 次 200
- test_retry_on_5xx — 502/503 重试
- test_no_retry_on_4xx — 400/401/403 不重试
- test_retry_exhausted — 3 次全 429 → 返回错误
- test_retry_with_exponential_backoff — 间隔递增验证

**请求体构建**：
- test_openai_body_format — 标准 OpenAI 格式（messages、tools、model）
- test_anthropic_body_format — Anthropic 格式（system 顶层、tool_use/tool_result）
- test_thinking_mode_body — with_thinking("max") 注入 thinking 字段
- test_extra_body_fields — with_body() 注入自定义字段

**流式**：
- test_stream_content_deltas — SSE Content delta 合并为完整文本
- test_stream_thinking_deltas — SSE Thinking delta → StreamEvent::Thinking
- test_stream_finish — SSE [DONE] 后 receiver 关闭
- test_stream_fallback_to_call — 无 streaming 实现时自动降级

**Anthropic 响应解析**：
- test_parse_anthropic_text_response — content[{type: "text", text: "..."}]
- test_parse_anthropic_tool_use — content[{type: "tool_use", ...}]
- test_parse_anthropic_error — Anthropic error 格式解析

### 4.3 core/hooks.rs — 全新

- test_before_run_after_run_pair — 配对调用（run 开始/结束各一次）
- test_on_finally_called_on_error — LLM 错误后 on_finally 仍被调用
- test_before_llm_modifies_context — HookContext 可变引用
- test_before_tools_after_tools_pair — 工具执行前后配对
- test_on_stream_delta_accumulates — 流式增量累加
- test_on_stream_end_with_resuming — Length finish → resume → on_stream_end(true)
- test_on_reasoning_delta — DeepSeek reasoning deltas
- test_wants_streaming_controls — wants_streaming=false 时走非流式
- test_finalize_content_pipeline — 多个 hook 依次加工内容
- test_on_error_receives_error_object — error 对象正确传递
- test_hook_error_isolation — 一个 hook panic 不影响后续 hook
- test_multi_hook_registration — 注册多个 hook 各自独立

### 4.4 core/types.rs — 全新

- test_message_serialization — 全部 Message 变体序列化为正确 JSON
- test_message_deserialization — JSON 反序列化 → Message
- test_tool_call_serialization — ToolCall 含 function name + arguments
- test_tool_definition_schema — Parameters 生成正确 JSON Schema
- test_finish_reason_display — 5 种 FinishReason 字符串化
- test_stream_event_variants — Content/Thinking/Finish 枚举匹配
- test_token_usage_defaults — TokenUsage 零值初始化

### 4.5 core/error.rs — 全新

- test_error_api_display — ApiError: status + body 格式化
- test_error_http_display — reqwest::Error 转换后 Display
- test_error_json_display — serde_json::Error 转换后 Display
- test_error_tool_not_found_display — ToolNotFound: name + available
- test_error_custom_display — Custom 字符串透传
- test_error_clone — Clone 后比较 Display 输出

### 4.6 core/prompt.rs — 全新

- test_concurrent_build — 10 个线程同时 build()，无 panic
- test_cache_invalidation_on_freeze — freeze_tools 后缓存失效重建
- test_clone_prompt_no_cache — 克隆后的 Prompt 缓存为空
- test_large_extension — 1KB 扩展文本正确拼接
- test_unicode_in_tool_json — Unicode 工具名/描述不破坏缓存

### 4.7 tools/ — 迁移 + 补充

从 motif-tools/tests/tool_tests.rs 迁移 42 个已有测试，新增：
- search: test_globstar_two_levels、test_globstar_root_only
- read: test_large_file_boundary (256KB exact)、test_offset_beyond_length
- edit: test_old_string_longer_than_file、test_concurrent_edit_same_file
- web_fetch: test_redirect_chain_exhausted、test_content_type_dispatch_json

### 4.8 tools/read_state.rs — 全新

- test_record_and_check_success — 读后编辑放行
- test_check_without_record_blocked — 未读不准编辑
- test_mtime_changed_after_record_blocked — 读取后被外部修改 → 拒绝
- test_concurrent_record_access — 多线程同时 record_read/check_read 无竞态

### 4.9 security/bash_injection.rs — 全新

- test_dollar_brace_default — ${IFS} 变量改写
- test_subshell_nested — $($(cmd)) 嵌套
- test_newline_escape — backslash-newline 续行
- test_hex_encoded — echo $'\x63\x61\x74' /etc/passwd
- test_backtick_nested — 嵌套反引号
- test_dollar_at_star — $@ / $* 展开
- test_path_masquerade — ../../with/../bypass 尝试

### 4.10 security/ssrf.rs — 全新

- test_all_private_ipv4_ranges — 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 127.0.0.0/8, 169.254.0.0/16
- test_ipv6_loopback — ::1 拒绝
- test_public_ips_allowed — 1.1.1.1, 8.8.8.8, 208.67.222.222
- test_dns_rebinding — host 重定向到 private IP（mock DNS）

### 4.11 security/path_traversal.rs — 全新

- test_double_encoding — %2e%2e%2f → ../
- test_unicode_normalization — ．．／ U+FF0E 全角点
- test_absolute_path — /etc/passwd（Unix）、C:\Windows\System32（Windows）
- test_null_byte — path\0/secret
- test_symlink_following — 符号链接指向敏感目录（Unix only, #[cfg(unix)]）

### 4.12 stress/concurrent.rs — 全新

- test_10_agents_parallel — 10 个 Agent 各自独立运行
- test_100_parallel_tool_calls — 100 个 read 并发，验证无死锁
- test_read_state_concurrent — 50 线程同时 record/check
- test_provider_shared_client — 共享 reqwest::Client 多线程

### 4.13 stress/large_io.rs — 全新

- test_256kb_read — 读取恰好 256KB 文件
- test_write_1mb — 写入 1MB 文件
- test_10000_line_read — 读取 2000 行以上
- test_very_long_old_string — 9999 字符 old_string
- test_path_300_chars — 超长路径参数

### 4.14 stress/long_running.rs — 全新

- test_500_iterations — Agent 运行 500 步，验证无内存泄漏
- test_on_stuck_after_3_repeat — 连续 3 次重复 → OnStuck 停止
- test_max_iterations_default — 默认 0（无限）不意外终止

### 4.15 cli/commands.rs — 全新

- test_help_command_registered — /help 返回可用命令
- test_clear_creates_new_session — /clear 后消息归零
- test_status_shows_model — /status 输出含 model 名
- test_config_masks_api_key — /config 不显示完整 key
- test_list_no_panic — /list 无会话时不 panic
- test_delete_nonexistent — /delete 不存在的 ID 提示 not found
- test_export_nonexistent — /export 不存在的 ID 提示 not found
- test_load_nonexistent — /load 不存在的 ID 提示 not found

### 4.16 cli/config.rs — 全新

- test_config_load_valid_json — 有效 JSON 正确解析
- test_config_default_values — 缺失字段回退默认值
- test_config_invalid_json — 无效 JSON 处理

### 4.17 macros/tool_macro.rs — 全新

**编译通过测试**：
- test_tool_macro_on_function — #[tool] async fn 生成 ToolArgs + 可注册
- test_tool_macro_on_impl_block — #[tool] impl MyStruct 生成多个方法
- test_tool_macro_with_name_attribute — #[tool(name = "custom_name")]
- test_tool_macro_serde_rename — #[serde(rename = "lowercase")]
- test_tool_macro_doc_comment_as_description — /// 注释 → JSON Schema description
- test_tool_macro_multiple_params — 多个参数的 schema 生成

**编译通过测试**（增补）：
- test_tool_macro_return_type_is_string — 返回 String 类型编译+运行

**编译失败测试**（2 项，依赖 trybuild crate，放入 macros/tests/ui/{pass,fail}/ 目录）：

- ui/fail/non_async_fn.rs — 同步函数拒绝编译
- ui/fail/non_string_return.rs — 返回值非 String 拒绝编译

**trybuild 集成**: macros/Cargo.toml 添加 `[dev-dependencies] trybuild = "1"`，在 macros/tests/ui_tests.rs 中一行调用 `trybuild::TestCases::new().compile_fail("tests/ui/fail/*.rs").pass("tests/ui/pass/*.rs")`

### 4.18 live/api.rs — 迁移 + 补充

从 motif/tests/integration.rs 迁移 13 个 #[ignore] 测试，新增：
- test_live_anthropic_format — ANTHROPIC_FORMAT=1 时 Anthropic 端点
- test_live_streaming_content — chat_stream() 真实流式
- test_live_concurrent_two_agents — 2 个 Agent 并发 real API

---

## 五、迁移策略

原有测试不删除，分两步走：

1. **新建 motif-tests crate**，写全部新测试（core/、security/、stress/、cli/、macros/、live/ 的 NEW 部分）
2. **迁移已有测试**：从 motif/tests/integration.rs 迁移 25 mock + 13 live，从 motif-tools/tests/tool_tests.rs 迁移 42 个
3. **删除原文件**，保持 `#[cfg(test)]` 模块不动

最终测试分布：

| 位置 | 数量 | 类型 |
|------|------|------|
| 源码内 `#[cfg(test)]` | ~45 | 内部函数单元测试 |
| motif-tests/tests/ | ~180 | 集成/安全/压力/CLI/macro |
| motif-tests/live/ | ~16 | 真实 API（#[ignore]） |
| **总计** | **~240** | |

---

## 六、依赖

```toml
# motif-tests/Cargo.toml
[dev-dependencies]
motif = { path = "../motif" }
motif-tools = { path = "../motif-tools" }
motif-cli = { path = "../motif-cli" }
motif-session = { path = "../motif-session" }
macros = { path = "../macros" }
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

不引入第三方测试框架（wiremock、proptest 等），mock HTTP 用 tokio 内联实现。  
例外：`macros` crate 引入 `trybuild` 用于编译失败测试。

---

## 七、不做什么

- 不引入 property-based testing（proptest/quickcheck）——当前阶段不需要
- 不引入 benchmark（criterion）——性能基准等 v0.3
- 不修改任何源码的 `#[cfg(test)]` 模块——保持不动
- 不引入 code coverage CI——手动用 cargo tarpaulin
