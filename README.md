# Motif

Minimal, extensible Rust agent core.

```rust
use motif::*;

#[tokio::main]
async fn main() -> motif::Result<()> {
    let provider = OpenAIProvider::new(
        "https://api.deepseek.com/v1", "sk-...", "deepseek-chat");

    #[tool]
    async fn search(query: String) -> String {
        format!("Results: {}", query)
    }

    let mut agent = Agent::new(provider)
        .model("deepseek-chat")
        .tool_fn(search);

    let response = agent.chat("Search Rust tutorials").await?;
    println!("{}", response);
    Ok(())
}
```

## Features

- **6 trait-isolated cores** — Agent loop, Provider, Tools, History, Hooks, Prompt
- **`#[tool]` proc macro** — annotate any async function as a tool
- **3-layer prompt** — L0 identity (9 sections, cached), L1 tools JSON, L2 extensions
- **9 lifecycle hooks** — before/after LLM calls, tool execution, run boundaries
- **5 stop conditions** — OnText, AfterNTools, OnStuck, Never, Custom predicate
- **Retry + streaming** — 429/5xx retry, SSE streaming with byte-level UTF-8 safety
- **Token tracking** — real API usage captured and accumulated
- **10 deps** — tokio, serde, reqwest, async-trait, thiserror, futures, schemars, tracing, chrono, motif-macros

## Quick Start

```bash
cargo add motif
```

```rust
use motif::*;

let provider = OpenAIProvider::new("https://api.openai.com/v1", "sk-...", "gpt-4o");
let mut agent = Agent::new(provider);

let response = agent.chat("Hello!").await?;
```

## Architecture

```
src/
├── agent.rs     Agent + step/run/chat + StopCondition
├── provider.rs  LLMProvider trait + OpenAIProvider
├── tool.rs      Tool trait + Executor + ToolDef builder
├── history.rs   History trait + InfiniteHistory + BoundedHistory
├── prompt.rs    Prompt (3-layer cached) + PromptBuilder
├── hooks.rs     AgentHook (9 lifecycle methods)
├── types.rs     Message, ToolCall, LLMResponse, TokenUsage, etc.
├── error.rs     Error enum + Clone
└── lib.rs       Re-exports
```

## License

MIT
