//! File writing with safety checks.

use crate::core::tool::RegisteredTool;
use crate::core::tool::ToolDef;
use crate::tools::PROTECTED_FILES;
use std::path::Path;

const MAX_WRITE_SIZE: usize = 1_048_576; // 1MB

pub fn register() -> RegisteredTool {
    ToolDef::new("write", "Write content to a file. Creates parent directories as needed.")
        .param::<String>("file_path", "Path to write to")
        .param::<String>("content", "Content to write")
        .build(write_impl)
}

fn write_impl(args: String) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> {
    Box::pin(async move {
        let v: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
        let file_path = v["file_path"].as_str().unwrap_or("").to_string();
        let content = v["content"].as_str().unwrap_or("").to_string();
        if file_path.is_empty() { return "Error: 'file_path' is required".to_string(); }

        let path = Path::new(&file_path);

        // Safety: protected files
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if PROTECTED_FILES.contains(&name) {
                return format!("Cannot write to protected file: {}", name);
            }
        }

        // Safety: path traversal
        if file_path.contains("..") {
            return "Path traversal not allowed".to_string();
        }

        // Size limit
        if content.len() > MAX_WRITE_SIZE {
            return format!(
                "Content too large ({} bytes, limit is {})",
                content.len(),
                MAX_WRITE_SIZE
            );
        }

        // Create parent directory
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return format!("Cannot create parent directory: {}", e);
                }
            }
        }

        // Write
        match std::fs::write(path, &content) {
            Ok(()) => {
                if content.is_empty() {
                    format!("Created empty file: {}", file_path)
                } else {
                    format!("Wrote {} bytes to {}", content.len(), file_path)
                }
            }
            Err(e) => format!("Error writing file: {}", e),
        }
    })
}
