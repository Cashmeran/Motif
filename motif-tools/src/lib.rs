pub mod bash;
pub mod edit;
pub mod read;
pub(crate) mod read_state;
pub mod search;
pub mod web_fetch;
pub mod write;

use std::path::Path;

pub(crate) const SKIP_DIRS: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "target",
    "node_modules",
    "build",
    "dist",
    "__pycache__",
    ".cache",
    ".next",
    ".nuxt",
    "vendor",
];

pub(crate) const BINARY_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "dylib", "png", "jpg", "jpeg", "gif", "ico", "pdf", "zip", "tar", "gz",
    "bz2", "class", "pyc", "wasm", "mp3", "mp4", "avi", "mov",
];

pub(crate) const PROTECTED_FILES: &[&str] =
    &[".env", ".env.local", ".gitconfig", ".bashrc", ".zshrc"];

pub(crate) fn is_binary_extension(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| BINARY_EXTENSIONS.contains(&e))
        .unwrap_or(false)
}

pub(crate) fn is_skip_dir(name: &str) -> bool {
    SKIP_DIRS.contains(&name)
}

pub(crate) fn paginate<T>(items: Vec<T>, offset: usize, head_limit: usize) -> (usize, Vec<T>) {
    let total = items.len();
    let limit = if head_limit == 0 { total } else { head_limit };
    let start = offset.min(total);
    let end = (start + limit).min(total);
    (
        total,
        items.into_iter().skip(start).take(end - start).collect(),
    )
}
