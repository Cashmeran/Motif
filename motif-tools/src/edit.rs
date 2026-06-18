//! File editing via exact string replacement. Atomic by design: the edit
//! is only applied when `old_string` appears exactly once in the file.

use motif::RegisteredTool;
use motif::ToolDef;

const MAX_FILE_SIZE: u64 = 1_048_576; // 1 MiB (Aegis default)
const MAX_OLD_STRING_LEN: usize = 10_000;

pub fn register() -> RegisteredTool {
    ToolDef::new(
        "edit",
        "Replace a specific string in a file. The old_string must appear exactly once — otherwise the edit is rejected to prevent unintended changes.",
    )
    .param::<String>("file_path", "Path to the file to edit")
    .param::<String>("old_string", "Exact string to replace. Must be unique in the file")
    .param::<String>("new_string", "Replacement string")
    .param::<bool>("replace_all", "Replace all occurrences (default: false — requires unique match)")
    .build(edit_impl)
}

fn edit_impl(args: String) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> {
    Box::pin(async move {
        let v: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
        let file_path = v["file_path"].as_str().unwrap_or("").to_string();
        let old = v["old_string"].as_str().unwrap_or("").to_string();
        let new = v["new_string"].as_str().unwrap_or("").to_string();
        let replace_all = v["replace_all"].as_bool().unwrap_or(false);

        if file_path.is_empty() { return "Error: 'file_path' is required".to_string(); }
        if old.is_empty() && new.is_empty() { return "Error: old_string and new_string cannot both be empty".to_string(); }

        // Safety: path traversal
        if file_path.contains("..") { return "Path traversal not allowed".to_string(); }

        // Safety: file size
        let meta = match std::fs::metadata(&file_path) {
            Ok(m) => m,
            Err(e) => return format!("Cannot access file: {}", e),
        };
        if meta.len() > MAX_FILE_SIZE {
            return format!("File too large ({} bytes, limit is {})", meta.len(), MAX_FILE_SIZE);
        }

        // Safety: old_string length
        if old.len() > MAX_OLD_STRING_LEN {
            return format!("old_string too long ({} chars, limit is {})", old.len(), MAX_OLD_STRING_LEN);
        }

        // Read
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(e) => return format!("Error reading file: {}", e),
        };

        // Empty old_string: full overwrite
        if old.is_empty() {
            match std::fs::write(&file_path, &new) {
                Ok(()) => return format!("Wrote {} bytes to {}", new.len(), file_path),
                Err(e) => return format!("Error writing file: {}", e),
            }
        }

        // No-op
        if old == new { return "old_string and new_string are identical — nothing to do".to_string(); }

        // Replace
        let result = if replace_all {
            if !content.contains(&old) {
                format!("old_string not found in {}", file_path)
            } else {
                let replaced = content.replace(&old, &new);
                match std::fs::write(&file_path, &replaced) {
                    Ok(()) => format!("Replaced {} occurrences in {}", content.matches(&old).count(), file_path),
                    Err(e) => format!("Error writing file: {}", e),
                }
            }
        } else {
            let count = content.match_indices(&old).count();
            if count == 0 {
                format!("old_string not found in {}", file_path)
            } else if count > 1 {
                format!(
                    "old_string appears {} times in {} — it must be unique for a safe edit. Use a longer string with more surrounding context, or use replace_all: true.",
                    count, file_path
                )
            } else {
                let replaced = content.replacen(&old, &new, 1);
                match std::fs::write(&file_path, &replaced) {
                    Ok(()) => format!("Edited {}", file_path),
                    Err(e) => format!("Error writing file: {}", e),
                }
            }
        };
        result
    })
}
