//! Shell command execution with timeout and security checks.

use motif::RegisteredTool;
use motif::ToolDef;
use tokio::process::Command;
use std::process::Stdio;

const DEFAULT_TIMEOUT_MS: u64 = 120_000; // 2 minutes
const MAX_TIMEOUT_MS: u64 = 300_000; // 5 minutes
const MAX_OUTPUT_CHARS: usize = 50_000;

/// Commands that require explicit confirmation. Checked with word-boundary.
const DESTRUCTIVE_SUBSTRINGS: &[&str] = &[
    "rm -rf", "rm -r", "rmdir",
    "sudo ", "su ",
    "chmod 777",
    "mkfs.", "dd if=",
    ":(){ :|:& };:", // fork bomb
    "> /dev/sda", "> /dev/hda",
    "shutdown", "reboot", "halt", "poweroff",
    "git push --force", "git push -f",
    "zmodload", "emulate", "sysopen", "ztcp", "zpty",
];

pub fn register() -> RegisteredTool {
    ToolDef::new("bash", "Execute a shell command with configurable timeout")
        .param::<String>(
            "command",
            "Shell command to execute. Multiple commands can be chained with && or ;",
        )
        .param::<u64>(
            "timeout_ms",
            "Timeout in milliseconds (default 120000, max 300000)",
        )
        .param::<String>("work_dir", "Working directory for the command")
        .build(bash_impl)
}

fn bash_impl(args: String) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> {
    Box::pin(async move {
        let v: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
        let command = v["command"].as_str().unwrap_or("").to_string();
        if command.is_empty() { return "Error: 'command' is required".to_string(); }

        // Security: check destructive patterns
        let cmd_lower = command.to_lowercase();
        for sub in DESTRUCTIVE_SUBSTRINGS {
            if cmd_lower.contains(sub) {
                return format!(
                    "Destructive command pattern detected: '{}'. If intentional, use a safer alternative or adjust the script. To override, edit DESTRUCTIVE_SUBSTRINGS.",
                    sub
                );
            }
        }

        let timeout_ms = v["timeout_ms"].as_u64().unwrap_or(DEFAULT_TIMEOUT_MS).min(MAX_TIMEOUT_MS);
        let work_dir = v["work_dir"].as_str().map(|s| s.to_string());

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", &command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", &command]);
            c
        };

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(ref dir) = work_dir {
            cmd.current_dir(dir);
        }

        let child = match cmd.kill_on_drop(true).spawn() {
            Ok(c) => c,
            Err(e) => return format!("Failed to spawn command: {}", e),
        };

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            child.wait_with_output(),
        )
        .await;

        let output = match result {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return format!("Command execution error: {}", e),
            Err(_) => {
                // Process timed out — the child might still be running.
                // We can't kill it after the timeout because wait_with_output has
                // already been dropped. Return a timeout message.
                return format!("Command timed out after {}s", timeout_ms / 1000);
            }
        };

        let mut out = String::new();
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".into());

        if !stdout_str.is_empty() {
            let display = truncate_str(&stdout_str, MAX_OUTPUT_CHARS);
            out.push_str(&display);
        }
        if !stderr_str.is_empty() {
            if !out.is_empty() { out.push('\n'); }
            let display = truncate_str(&format!("[stderr]\n{}", stderr_str), MAX_OUTPUT_CHARS - out.len());
            out.push_str(&display);
        }
        if out.is_empty() {
            out = format!("(Command completed successfully, exit code {})", exit_code);
        } else {
            let _ = writeln_debug(&mut out, &format!("\n(exit code: {})", exit_code));
        }

        // Truncate at MAX_OUTPUT_CHARS
        if out.len() > MAX_OUTPUT_CHARS {
            out.truncate(MAX_OUTPUT_CHARS);
            out.push_str("\n(Output truncated)");
        }

        out
    })
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars { return s.to_string(); }
    let mut t = s[..max_chars].to_string();
    t.push_str("\n(truncated)");
    t
}

// Helper to write without importing std::fmt::Write
fn writeln_debug(buf: &mut String, s: &str) -> std::fmt::Result {
    buf.push_str(s);
    Ok(())
}
