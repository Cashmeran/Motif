//! AgentHook implementations — decoupled from config and CLI logic.

use motif::AgentHook;
use std::io::Write;

// ── StreamPrinter ──

/// Prints streaming content deltas to stdout as they arrive.
/// Registered by default in `config::make_agent`.
pub struct StreamPrinter;

#[async_trait::async_trait]
impl AgentHook for StreamPrinter {
    async fn on_stream_delta(&self, delta: &str) -> motif::Result<()> {
        print!("{}", delta);
        std::io::stdout().flush().ok();
        Ok(())
    }
}

// ── ContextInjection ──

/// Injects environment context (git branch, cwd, OS, time) into
/// the first user message via `before_llm`.  This gives the agent
/// automatic awareness of its execution environment without the
/// user needing to describe it.
pub struct ContextInjection;

#[async_trait::async_trait]
impl AgentHook for ContextInjection {
    async fn before_llm(&self, ctx: &mut HookContext) -> motif::Result<()> {
        let mut context = String::new();

        // Git branch
        if let Ok(output) = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .output()
        {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !branch.is_empty() {
                context.push_str(&format!("  git branch: {}\n", branch));
            }
        }

        // Current directory
        if let Ok(cwd) = std::env::current_dir() {
            context.push_str(&format!("  cwd: {}\n", cwd.display()));
        }

        // OS info
        context.push_str(&format!("  os: {}\n", std::env::consts::OS));

        // Inject context into the last user message
        if let Some(tm) = ctx.messages.iter_mut().rev().find(|m| matches!(m.message, motif::Message::User(_))) {
            if let motif::Message::User(ref mut u) = tm.message {
                u.content = format!(
                    "<environment>\n{}<\\environment>\n\n{}",
                    context, u.content
                );
            }
        }

        Ok(())
    }
}
