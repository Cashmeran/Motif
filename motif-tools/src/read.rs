//! File reading with offset/limit pagination and safety checks.

use crate::read_state;
use crate::PROTECTED_FILES;
use motif::RegisteredTool;
use motif::ToolDef;
use std::fmt::Write as _;
use std::path::Path;

const MAX_FILE_SIZE: u64 = 256 * 1024; // 256KB
const MAX_LINES: usize = 2000;
const BLOCKED_DEVICE_PATHS: &[&str] = &[
    "/dev/zero",
    "/dev/random",
    "/dev/urandom",
    "/dev/full",
    "/dev/tty",
    "/dev/console",
];

pub fn register() -> RegisteredTool {
    ToolDef::new("read", "Read a file with optional line-based pagination")
        .param::<String>("file_path", "Path to the file to read")
        .param::<u64>(
            "offset",
            "Start reading from this line (0-indexed, default 0)",
        )
        .param::<u64>("limit", "Max lines to read (default 2000, 0 = unlimited)")
        .build(read_impl)
}

fn read_impl(args: String) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> {
    Box::pin(async move {
        let v: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
        let file_path = v["file_path"].as_str().unwrap_or("").to_string();
        if file_path.is_empty() {
            return "Error: 'file_path' is required".to_string();
        }

        let path = Path::new(&file_path);

        // Safety: protected files
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if PROTECTED_FILES.contains(&name) {
                return format!("Cannot read protected file: {}", name);
            }
        }

        // Safety: device files
        let path_str = path.to_string_lossy();
        if BLOCKED_DEVICE_PATHS.iter().any(|d| path_str.starts_with(d))
            || path_str.contains("/proc/") && path_str.contains("/fd/")
        {
            return format!("Cannot read device file: {}", path_str);
        }

        // Safety: path traversal
        if file_path.contains("..") {
            return "Path traversal not allowed".to_string();
        }

        // Size check
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) => return format!("Cannot access file: {}", e),
        };
        if meta.len() > MAX_FILE_SIZE {
            return format!(
                "File too large ({} bytes, limit is {})",
                meta.len(),
                MAX_FILE_SIZE
            );
        }

        // Read
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => return format!("Error reading file: {}", e),
        };

        // Record read for read-before-edit enforcement
        if let Ok(mtime) = meta.modified() {
            read_state::record_read(&file_path, &content, mtime);
        }

        let offset = v["offset"].as_u64().unwrap_or(0) as usize;
        let limit = v["limit"].as_u64().unwrap_or(MAX_LINES as u64) as usize;
        let effective_limit = if limit == 0 || limit > MAX_LINES {
            MAX_LINES
        } else {
            limit
        };

        let lines: Vec<&str> = content.lines().collect();
        let total = lines.len();
        let start = offset.min(total);
        let end = (start + effective_limit).min(total);

        let mut out = String::new();
        for line in lines[start..end].iter() {
            let _ = writeln!(out, "{}", line);
        }

        if start > 0 || end < total {
            let _ = write!(out, "\n(Showing lines {}-{} of {})", start + 1, end, total);
        }
        if out.is_empty() {
            out = "(File is empty)".to_string();
        }
        out
    })
}
