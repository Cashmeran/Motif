//! File editing via exact string replacement. Atomic by design: the edit
//! is only applied when `old_string` appears exactly once in the file.

use motif::RegisteredTool;
use motif::ToolDef;
use crate::read_state;

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

        // Read-before-edit enforcement
        if let Err(e) = read_state::check_read(&file_path) {
            return e;
        }

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

        // Quote normalization: LLM may output straight quotes while file has curly quotes (or vice versa)
        let old_matched = match normalize_quotes(&old, &content) {
            Some(actual) => actual,
            None => {
                // Concurrent modification detection: content hash changed since last read?
                // Write the hash to a static tracker on each read, compare here.
                return format!("old_string not found in {}", file_path);
            }
        };
        let actual_old = if old_matched == old { old.clone() } else { old_matched };

        // Replace using the actual (possibly normalized) old string
        let result = if replace_all {
            if !content.contains(&actual_old) {
                format!("old_string not found in {}", file_path)
            } else {
                let replaced = content.replace(&actual_old, &new);
                match std::fs::write(&file_path, &replaced) {
                    Ok(()) => format!("Replaced {} occurrences in {}", content.matches(&actual_old).count(), file_path),
                    Err(e) => format!("Error writing file: {}", e),
                }
            }
        } else {
            let count = content.match_indices(&actual_old).count();
            if count == 0 {
                format!("old_string not found in {}", file_path)
            } else if count > 1 {
                format!(
                    "old_string appears {} times in {} — it must be unique for a safe edit. Use a longer string with more surrounding context, or use replace_all: true.",
                    count, file_path
                )
            } else {
                let replaced = content.replacen(&actual_old, &new, 1);
                match std::fs::write(&file_path, &replaced) {
                    Ok(()) => format!("Edited {}", file_path),
                    Err(e) => format!("Error writing file: {}", e),
                }
            }
        };
        result
    })
}

/// Try common quote variants when the exact old_string is not found.
/// LLMs often output straight quotes (""") while source files may use
/// curly quotes ("\u{201c}" / "\u{201d}"), and vice versa.
fn normalize_quotes(needle: &str, haystack: &str) -> Option<String> {
    if haystack.contains(needle) { return Some(needle.to_string()); }

    // Straight → curly double
    if needle.contains('"') {
        let curly = needle.replace('"', "\u{201c}");
        if haystack.contains(&curly) { return Some(curly); }
        let curly2 = needle.replace('"', "\u{201d}");
        if haystack.contains(&curly2) { return Some(curly2); }
    }

    // Curly double → straight
    if needle.contains('\u{201c}') || needle.contains('\u{201d}') {
        let straight = needle.replace('\u{201c}', "\"").replace('\u{201d}', "\"");
        if haystack.contains(&straight) { return Some(straight); }
    }

    // Straight → curly single
    if needle.contains('\'') {
        let curly = needle.replace('\'', "\u{2018}");
        if haystack.contains(&curly) { return Some(curly); }
    }

    // Curly single → straight
    if needle.contains('\u{2018}') {
        let straight = needle.replace('\u{2018}', "'");
        if haystack.contains(&straight) { return Some(straight); }
    }

    None
}
