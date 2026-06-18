//! Built-in tools. Each tool exports a `register()` function returning a
//! [`RegisteredTool`]. Import what you need:
//! ```rust,ignore
//! use motif::tools::search;
//! agent.tool(search::register());
//! ```

pub mod bash;
pub mod read;
pub mod search;
pub mod write;

use std::path::Path;

// ── Shared constants ──

/// Directories excluded from recursive search.
pub(crate) const SKIP_DIRS: &[&str] = &[
    ".git", ".svn", ".hg", ".bzr",
    "target", "node_modules", "build", "dist",
    "__pycache__", ".cache", ".next", ".nuxt",
    "vendor", "bower_components",
];

/// Extensions treated as binary (skipped by content search).
pub(crate) const BINARY_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "dylib", "bin",
    "png", "jpg", "jpeg", "gif", "ico", "bmp", "webp", "svg",
    "pdf", "zip", "tar", "gz", "bz2", "xz", "7z", "rar",
    "class", "pyc", "pyo", "wasm", "woff", "woff2", "ttf", "eot",
    "mp3", "mp4", "avi", "mov", "mkv",
];

/// Files that should never be modified.
pub(crate) const PROTECTED_FILES: &[&str] = &[
    ".env", ".env.local", ".gitconfig", ".bashrc", ".zshrc",
    ".mcp.json", ".claude.json",
    "id_rsa", "id_ed25519", "id_ecdsa",
];

// ── Shared helpers ──

/// Check if a file extension indicates binary content.
pub(crate) fn is_binary_extension(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| BINARY_EXTENSIONS.contains(&e))
        .unwrap_or(false)
}

/// Check if a path component is a skipped directory.
pub(crate) fn is_skip_dir(name: &str) -> bool {
    SKIP_DIRS.contains(&name)
}

/// Paginate results. Returns the total before pagination and the truncated list.
pub(crate) fn paginate<T>(
    items: Vec<T>,
    offset: usize,
    head_limit: usize,
) -> (usize, Vec<T>) {
    let total = items.len();
    let effective_limit = if head_limit == 0 { total } else { head_limit };
    let start = offset.min(total);
    let end = (start + effective_limit).min(total);
    let truncated = items.into_iter().skip(start).take(end - start).collect();
    (total, truncated)
}
