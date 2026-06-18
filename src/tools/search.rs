//! Combined search tool: regex content search (grep) + filename matching (glob).

use crate::core::tool::RegisteredTool;
use crate::core::tool::ToolDef;
use crate::tools::{is_binary_extension, is_skip_dir, paginate};
use regex::RegexBuilder;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

const DEFAULT_HEAD_LIMIT: usize = 250;

pub fn register() -> RegisteredTool {
    ToolDef::new("search", "Search file content (regex) or filenames (glob)")
        .param::<String>("query", "Search pattern. Regex for content modes, glob for filename mode")
        .param::<String>(
            "mode",
            r#"Search mode:
- "content": show matching lines (supports context, pagination)
- "files_with_matches": only file paths (sorted by modification time)
- "count": match counts per file
- "filename": glob pattern match against file names (default)"#,
        )
        .param::<String>(
            "path",
            "Directory or file to search in. Defaults to current working directory",
        )
        .param::<String>("glob", "Optional file pattern filter (e.g. \"*.rs\")")
        .param::<bool>("ignore_case", "Case-insensitive search (default: true)")
        .param::<bool>("multiline", "Enable multiline regex matching")
        .param::<u64>("head_limit", "Max results (default: 250, 0 = unlimited)")
        .param::<u64>("offset", "Skip first N results (for pagination)")
        .param::<u64>("before_context", "Lines to show before each match")
        .param::<u64>("after_context", "Lines to show after each match")
        .param::<bool>("line_numbers", "Show line numbers (default: true)")
        .build(search_impl)
}

fn search_impl(args: String) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> {
    Box::pin(async move {
        let v: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
        let query = v["query"].as_str().unwrap_or("").to_string();
        if query.is_empty() {
            return "Error: 'query' is required".to_string();
        }

        let mode = v["mode"].as_str().unwrap_or("filename").to_string();
        let path = v["path"].as_str().map(|s| s.to_string()).unwrap_or_else(|| ".".into());
        let glob = v["glob"].as_str().map(|s| s.to_string());
        let ignore_case = v["ignore_case"].as_bool().unwrap_or(true);
        let multiline = v["multiline"].as_bool().unwrap_or(false);
        let head_limit = v["head_limit"].as_u64().unwrap_or(DEFAULT_HEAD_LIMIT as u64) as usize;
        let offset = v["offset"].as_u64().unwrap_or(0) as usize;
        let before = v["before_context"].as_u64().unwrap_or(0) as usize;
        let after = v["after_context"].as_u64().unwrap_or(0) as usize;
        let line_numbers = v["line_numbers"].as_bool().unwrap_or(true);

        let root = Path::new(&path);
        if !root.exists() {
            return format!("Path not found: {}", path);
        }

        match mode.as_str() {
            "content" | "count" | "files_with_matches" => {
                search_content(root, &query, &mode, glob, ignore_case, multiline, head_limit, offset, before, after, line_numbers).await
            }
            "filename" | _ => {
                search_filenames(root, &query, glob, head_limit, offset).await
            }
        }
    })
}

async fn search_content(
    root: &Path,
    pattern: &str,
    mode: &str,
    glob_filter: Option<String>,
    ignore_case: bool,
    multiline: bool,
    head_limit: usize,
    offset: usize,
    before: usize,
    after: usize,
    line_numbers: bool,
) -> String {
    let re = match RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .dot_matches_new_line(multiline)
        .multi_line(multiline)
        .build()
    {
        Ok(r) => r,
        Err(e) => return format!("Invalid regex: {}", e),
    };

    let mut entries: Vec<(String, SystemTime)> = Vec::new();
    let is_count = mode == "count";
    let is_files_only = mode == "files_with_matches";

    // Walk directory tree
    let mut dirs = vec![root.to_path_buf()];
    let mut depth = 0;
    while !dirs.is_empty() && depth < 30 {
        let mut next = Vec::new();
        for dir in &dirs {
            let entries_iter = match fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries_iter.flatten() {
                let ft = match entry.file_type() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') || is_skip_dir(&name_str) {
                    continue;
                }
                let p = entry.path();
                if ft.is_dir() {
                    if depth < 29 {
                        next.push(p);
                    }
                } else if ft.is_file()
                    && !is_binary_extension(&p)
                    && glob_matches(&p, glob_filter.as_deref())
                {
                    if let Ok(content) = fs::read_to_string(&p) {
                        if is_files_only {
                            let count = content.lines().filter(|l| re.is_match(l)).count();
                            if count > 0 {
                                if let Ok(meta) = fs::metadata(&p) {
                                    if let Ok(mtime) = meta.modified() {
                                        if let Ok(rel) = p.strip_prefix(root) {
                                            entries.push((rel.display().to_string(), mtime));
                                        }
                                    }
                                }
                            }
                            continue;
                        }
                        if is_count {
                            let count = content.lines().filter(|l| re.is_match(l)).count();
                            if count > 0 {
                                if let Ok(rel) = p.strip_prefix(root) {
                                    entries.push((format!("{}: {}", rel.display(), count), SystemTime::UNIX_EPOCH));
                                }
                            }
                            continue;
                        }
                        // Content mode
                        let lines: Vec<&str> = content.lines().collect();
                        let mut file_entries: Vec<(String, SystemTime)> = Vec::new();
                        let mut i = 0;
                        while i < lines.len() {
                            if re.is_match(lines[i]) {
                                let mut block = String::new();
                                let ctx_start = if before > 0 { i.saturating_sub(before) } else { i };
                                let ctx_end = (i + after + 1).min(lines.len());
                                for j in ctx_start..ctx_end {
                                    let marker = if j == i { ">" } else { " " };
                                    if line_numbers {
                                        let _ = writeln!(block, "{}{:>6} {}", marker, j + 1, lines[j]);
                                    } else {
                                        let _ = writeln!(block, "{}{}", marker, lines[j]);
                                    }
                                }
                                if ctx_end < lines.len() || ctx_start > 0 {
                                    block.push_str("--\n");
                                }
                                file_entries.push((block, SystemTime::UNIX_EPOCH));
                                i = ctx_end;
                            } else {
                                i += 1;
                            }
                        }
                        if !file_entries.is_empty() {
                            if let Ok(rel) = p.strip_prefix(root) {
                                let header = format!("## {}\n", rel.display());
                                let body: String = file_entries.into_iter().map(|(s, _)| s).collect();
                                entries.push((format!("{}{}", header, body), SystemTime::UNIX_EPOCH));
                            }
                        }
                    }
                }
            }
        }
        dirs = next;
        depth += 1;
    }

    // Sort by mtime descending for files_with_matches, alphabetical otherwise
    if is_files_only {
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    }

    let (total, results) = paginate(entries.into_iter().map(|(s, _)| s).collect(), offset, head_limit);
    let mut out = String::new();
    for r in &results {
        out.push_str(r);
        if !r.ends_with('\n') { out.push('\n'); }
    }
    if head_limit > 0 && total > offset + head_limit {
        let _ = write!(out, "\n(Truncated: showing {}-{} of {} results. Use offset/head_limit for more.)", offset + 1, offset + results.len(), total);
    }
    if out.is_empty() {
        out = format!("No matches found for '{}'", pattern);
    }
    out
}

async fn search_filenames(
    root: &Path,
    pattern: &str,
    glob_filter: Option<String>,
    head_limit: usize,
    offset: usize,
) -> String {
    let mut entries: Vec<(String, SystemTime)> = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    let mut depth = 0;
    while !dirs.is_empty() && depth < 30 {
        let mut next = Vec::new();
        for dir in &dirs {
            let iter = match fs::read_dir(dir) { Ok(e) => e, Err(_) => continue };
            for entry in iter.flatten() {
                let ft = match entry.file_type() { Ok(t) => t, Err(_) => continue };
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') || is_skip_dir(&name_str) { continue; }
                let p = entry.path();
                if ft.is_dir() { if depth < 29 { next.push(p); } continue; }
                let rel = match p.strip_prefix(root) { Ok(r) => r.display().to_string(), Err(_) => continue };
                if !glob_matches(&p, glob_filter.as_deref()) { continue; }
                if !simple_glob_match(pattern, &rel) { continue; }
                let mtime = fs::metadata(&p).ok().and_then(|m| m.modified().ok()).unwrap_or(SystemTime::UNIX_EPOCH);
                entries.push((rel, mtime));
            }
        }
        dirs = next; depth += 1;
    }

    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let (total, results) = paginate(entries.into_iter().map(|(s, _)| s).collect(), offset, head_limit);

    let mut out = String::new();
    if results.is_empty() {
        let _ = write!(out, "No files matching '{}' found", pattern);
    } else {
        let _ = writeln!(out, "{} file(s) matching '{}':", total, pattern);
        for r in &results { let _ = writeln!(out, "{}", r); }
        if head_limit > 0 && total > offset + head_limit {
            let _ = write!(out, "\n(Truncated. Use offset/head_limit for more.)");
        }
    }
    out
}

/// Simple glob match supporting *, ?, and {a,b,c} brace expansion.
fn simple_glob_match(pattern: &str, name: &str) -> bool {
    let name = name.to_lowercase();
    let pattern = pattern.to_lowercase();
    // Handle brace expansion: expand each alternative and try
    if let Some((pre, rest)) = pattern.split_once('{') {
        if let Some((braces, post)) = rest.split_once('}') {
            for alt in braces.split(',') {
                let expanded = format!("{}{}{}", pre, alt.trim(), post);
                if simple_glob_match(&expanded, &name) { return true; }
            }
            return false;
        }
    }
    // Simple glob: replace * with regex .*, ? with .
    let mut regex = String::from("^");
    for ch in pattern.chars() {
        match ch {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            c if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/' => regex.push(c),
            _ => regex.push_str(&regex::escape(&ch.to_string())),
        }
    }
    regex.push('$');
    regex::Regex::new(&regex).map(|r| r.is_match(&name)).unwrap_or(false)
}

/// Check if a file path matches an optional glob filter.
fn glob_matches(p: &Path, glob: Option<&str>) -> bool {
    match glob {
        Some(g) => simple_glob_match(g, &p.display().to_string()),
        None => true,
    }
}
