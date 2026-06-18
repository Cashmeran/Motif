//! Shared read state for read-before-edit enforcement.
//! Tracked by read tool, checked by edit/write tools.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

static STATE: LazyLock<Mutex<HashMap<String, ReadRecord>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone)]
struct ReadRecord {
    mtime: SystemTime,
}

fn path_key(p: &str) -> String { Path::new(p).to_string_lossy().replace('\\', "/") }

/// Record that a file was read. Called by the read tool.
pub(crate) fn record_read(path: &str, _content: &str, mtime: SystemTime) {
    let key = path_key(path);
    STATE.lock().unwrap().insert(key, ReadRecord { mtime });
}

/// Check if a file was read and hasn't been modified since.
/// Returns `Ok(())` if safe to edit, `Err(msg)` otherwise.
pub(crate) fn check_read(path: &str) -> Result<(), String> {
    let key = path_key(path);
    let state = STATE.lock().unwrap();
    let record = match state.get(&key) {
        Some(r) => r,
        None => return Err(format!("{} has not been read yet. Read the file before editing it.", path)),
    };
    let current = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return Err(format!("Cannot access {}: {}", path, e)),
    };
    match current.modified() {
        Ok(mtime) if mtime != record.mtime => {
            Err(format!("{} was modified since last read. Re-read before editing.", path))
        }
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Cannot check mtime for {}: {}", path, e)),
    }
}
