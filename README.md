# Motif

A Rust agent core in 2,500 lines. Library-first. Trait-isolated. Ships with a CLI.

```bash
cargo install motif
motif
```

```rust
use motif::*;

#[tool]
async fn search(query: String) -> String {
    format!("Results for {}", query)
}

let mut agent = Agent::new(OpenAIProvider::new(
    "https://api.deepseek.com/v1", "sk-...", "deepseek-chat",
))
.model("deepseek-chat")
.tool_fn(search);

let response = agent.chat("Search Rust agent frameworks").await?;
```

## What is this

Motif is the loop, the tools, the history, and the prompt. Nothing else.

No built-in file operations. No code search. No memory system. No web UI. Each of those is a separate crate (or a 20-line `PromptBuilder` impl) that depends on Motif, not the other way around.

The core is 9 source files. You can read all of them in under an hour.

## What makes it different

### 1. Stop conditions are a configurable policy — not hardcoded

`OnText` (finish without tool calls), `AfterNTools` (N tool results), `OnStuck` (same call repeated), `Never` (you drive the loop), and `Custom` (your predicate).

```rust
// A verification loop without touching the core:
agent.stop_when(StopCondition::Custom(Arc::new(|resp, _history| {
    resp.message.content.contains("VERIFIED")
})));
```

No other lightweight agent has this. tiny-loop hardcodes the exit condition. nanobot runs until max iterations. Aegis exits on text or error. Motif lets you define what "done" means.

### 2. Nine lifecycle hooks — all of them are no-ops until you need them

`before_llm`, `after_llm`, `before_tools`, `after_tools`, `before_run`, `after_run`, `on_error`, `on_stream_delta`, `finalize_content`.

Want logging? `before_llm` + `tracing`. Want memory injection? `before_llm` + your retrieval code. Want post-processing? `finalize_content`. Each hook is a one-liner. Error isolation by default — one hook failing doesn't block the others.

### 3. The prompt is 3 layers, each cached independently

L0 (identity) is 9 sections — Meta, Identity, Rhythm, Voice, Honesty, Safety, Tool Use, Hallucination, Execution. Fingerprint-cached. L1 (tools JSON) sits on top, rebuilt when tools change. L2 (PromptBuilder extensions) is per-turn. Date goes in the user message, not the prompt — keeps L0+L1 cache-stable.

### 4. Every dependency is a trait. Swap any of them.

| trait | what you'd replace it with |
|-------|--------------------------|
| `LLMProvider` | Anthropic, Ollama, a mock, a router |
| `History` | capped, SQLite-backed, token-aware |
| `Tool` / `ToolExecutor` | sandboxed, remote, MCP bridge |
| `AgentHook` | logging, memory, guardrails |
| `PromptBuilder` | file tree, git status, skill list |

### 5. `#[tool]` macros on functions, methods, and impl blocks

```rust
#[tool]
async fn weather(city: String) -> String { ... }

#[tool]
impl Database {
    async fn query(self, sql: String) -> String { ... }
}

agent.tool_fn(weather);
agent.bind(db, Database::query);
```

Schema generation is automatic — doc comments become JSON descriptions, types become parameters.

## Architecture

```
src/
├── agent.rs      Agent loop + step()/run()/chat() + 5 stop conditions
├── provider.rs   LLMProvider trait + OpenAIProvider + retry + SSE streaming
├── tool.rs       Tool trait + Executor + ConcurrencySafety + ToolDef
├── history.rs    History trait + InfiniteHistory + BoundedHistory
├── prompt.rs     3-layer cached prompt + PromptBuilder trait
├── hooks.rs      AgentHook (9 methods, error-isolated)
├── types.rs      Message, ToolCall, LLMResponse, TokenUsage
├── error.rs      Error enum
├── lib.rs        Re-exports
└── main.rs       CLI binary
```

## Comparison

| | Motif | tiny-loop | nanobot | Aegis |
|---|-------|-----------|---------|-------|
| Language | Rust | Rust | Python | Rust |
| Core size | ~2,500 lines | ~920 lines | ~15,000 lines | ~165,000 lines |
| Stop conditions | 5 configurable | 1 hardcoded | 1 hardcoded | 1 hardcoded |
| Hooks | 9 methods | none | 12 methods | none |
| Tool macro | `#[tool]` on fn/impl | `#[tool]` on fn | decorator | manual trait |
| Prompt caching | 3-layer fingerprint | none | Jinja2 (no cache) | 3-layer SHA256 |
| Provider retry | 429/5xx | none | 3 modes | yes |
| Trait injection | all 6 core types | provider+tool+history | plugin system | 40-param constructor |
| CLI | built-in | examples/ | gateway | built-in |

## Tests

51 mock + 13 live (real DeepSeek API). Zero unsafe.

```bash
cargo test                  # 51 mock
MOTIF_API_KEY=sk-... cargo test -- --ignored   # +13 live
```

## Quick start

```bash
cargo install motif
motif                    # first run: enter your API key, saved to ~/.motif/config.json
```

Then just talk.

To use a different provider, edit `~/.motif/config.json`:

```json
{
  "api_key": "sk-...",
  "base_url": "https://api.openai.com/v1",
  "model": "gpt-4o-mini"
}
```

## License

MIT
