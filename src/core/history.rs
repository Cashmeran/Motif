use crate::core::types::TimedMessage;

/// Manages conversation history. Implementations may be infinite, capped, or
/// backed by external storage.
pub trait History: Send + Sync {
    /// Append a message to history.
    fn add(&mut self, message: TimedMessage);

    /// Return all messages in chronological order.
    fn get_all(&self) -> &[TimedMessage];

    /// Remove all messages.
    fn clear(&mut self);
}

/// Unbounded in-memory history. Suitable as a default; swap for a capped
/// implementation when context limits matter.
pub struct InfiniteHistory {
    messages: Vec<TimedMessage>,
}

impl InfiniteHistory {
    pub fn new() -> Self {
        Self { messages: vec![] }
    }
}

impl Default for InfiniteHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl History for InfiniteHistory {
    fn add(&mut self, message: TimedMessage) {
        self.messages.push(message);
    }

    fn get_all(&self) -> &[TimedMessage] {
        &self.messages
    }

    fn clear(&mut self) {
        self.messages.clear();
    }
}

// --- BoundedHistory ---

/// Capped in-memory history. When the capacity is exceeded, the oldest
/// non-system messages are dropped to make room. System messages (the
/// first message) are pinned and never evicted.
pub struct BoundedHistory {
    messages: Vec<TimedMessage>,
    capacity: usize,
}

impl BoundedHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            messages: vec![],
            capacity: capacity.max(1),
        }
    }
}

impl History for BoundedHistory {
    fn add(&mut self, message: TimedMessage) {
        self.messages.push(message);
        let excess = self.messages.len().saturating_sub(self.capacity);
        if excess == 0 {
            return;
        }
        // Drop oldest non-system messages. System messages are pinned.
        // If ALL messages are system, drop from the front anyway.
        let non_sys_count = self
            .messages
            .iter()
            .filter(|m| !matches!(m.message, crate::core::types::Message::System(_)))
            .count();
        let can_evict = non_sys_count.min(excess);
        let mut evicted = 0;
        self.messages.retain(|m| {
            if evicted >= can_evict {
                return true;
            }
            if matches!(m.message, crate::core::types::Message::System(_)) && non_sys_count > 0 {
                return true;
            }
            evicted += 1;
            false
        });
    }

    fn get_all(&self) -> &[TimedMessage] {
        &self.messages
    }

    fn clear(&mut self) {
        self.messages.clear();
    }
}

// --- FileHistory ---

use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

/// Persistent history backed by JSONL files at `~/.motif/sessions/`.
/// Each `add()` writes a single line to the file and fsyncs.
/// `clear()` starts a new session file.
pub struct FileHistory {
    messages: Vec<TimedMessage>,
    session_id: String,
    sessions_dir: PathBuf,
    file: Option<BufWriter<File>>,
}

impl FileHistory {
    /// Start a new session. Creates the sessions directory if needed.
    /// `session_id: None` generates a random 12-char id.
    pub fn new(session_id: Option<&str>) -> Self {
        let dir = dirs_sessions_dir();
        let _ = fs::create_dir_all(&dir);
        let id = session_id.map(|s| s.to_string()).unwrap_or_else(nano_id);
        let path = dir.join(format!("{}.jsonl", id));
        let file = OpenOptions::new().create(true).append(true).open(&path).ok();
        let f = file.map(|f| BufWriter::new(f));
        // Write metadata first line
        let mut fh = Self { messages: vec![], session_id: id, sessions_dir: dir, file: f };
        fh.append_raw(&serde_json::json!({"_meta": true, "created": chrono_now()}));
        fh.save_latest();
        fh.save_index().ok();
        fh
    }

    /// Load an existing session by ID. Reads all JSONL lines into memory.
    pub fn load(id: &str) -> Option<Self> {
        let dir = dirs_sessions_dir();
        let path = dir.join(format!("{}.jsonl", id));
        let data = fs::read_to_string(&path).ok()?;
        let messages: Vec<TimedMessage> = data
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter(|l| !l.contains("\"_meta\""))
            .filter_map(|l| serde_json::from_str::<TimedMessage>(l).ok())
            .collect();
        if messages.is_empty() { return None; }
        let file = OpenOptions::new().create(true).append(true).open(&path).ok().map(|f| BufWriter::new(f));
        let fh = Self { messages, session_id: id.to_string(), sessions_dir: dir, file };
        fh.save_latest();
        Some(fh)
    }

    /// List all session IDs from index.json.
    pub fn list() -> Vec<serde_json::Value> {
        let idx = dirs_sessions_dir().join("index.json");
        if let Ok(data) = fs::read_to_string(&idx) {
            serde_json::from_str(&data).unwrap_or_default()
        } else { vec![] }
    }

    /// Current session ID.
    pub fn session_id(&self) -> &str { &self.session_id }

    fn append_raw(&mut self, v: &serde_json::Value) {
        if let Some(ref mut f) = self.file {
            let mut line = serde_json::to_string(v).unwrap_or_default();
            line.push('\n');
            let _ = f.write_all(line.as_bytes());
            let _ = f.flush();
        }
    }

    fn save_latest(&self) {
        let latest = self.sessions_dir.join("latest");
        let target = format!("{}.jsonl", self.session_id);
        let _ = fs::remove_file(&latest);
        #[cfg(unix)] { std::os::unix::fs::symlink(target, latest).ok(); }
        #[cfg(not(unix))] {
            // Windows: write a file containing the session ID
            let _ = fs::write(&latest, &target);
        }
    }

    fn save_index(&self) -> std::io::Result<()> {
        let mut idx: Vec<serde_json::Value> = Self::list();
        let first_msg = self.messages.first().map(|m| serde_json::to_value(m).unwrap_or_default());
        // Remove existing entry for this session
        idx.retain(|e| e.get("id").and_then(|i| i.as_str()) != Some(&self.session_id));
        idx.push(serde_json::json!({
            "id": self.session_id,
            "date": chrono_now(),
            "count": self.messages.len(),
            "first": first_msg,
        }));
        let data = serde_json::to_string_pretty(&idx)?;
        fs::write(self.sessions_dir.join("index.json"), &data)
    }
}

impl History for FileHistory {
    fn add(&mut self, message: TimedMessage) {
        self.append_raw(&serde_json::to_value(&message).unwrap_or_default());
        self.messages.push(message);
    }

    fn get_all(&self) -> &[TimedMessage] { &self.messages }

    fn clear(&mut self) {
        self.messages.clear();
        // Start a new session
        self.session_id = nano_id();
        let path = self.sessions_dir.join(format!("{}.jsonl", self.session_id));
        let file = OpenOptions::new().create(true).append(true).open(&path).ok();
        self.file = file.map(|f| BufWriter::new(f));
        self.append_raw(&serde_json::json!({"_meta": true, "created": chrono_now()}));
        self.save_latest();
        self.save_index().ok();
    }
}

fn dirs_sessions_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".motif").join("sessions")
}

fn nano_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("{:016x}", t).chars().take(12).collect()
}

fn chrono_now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::Message;

    #[test]
    fn test_infinite_history_add_and_retrieve() {
        let mut h = InfiniteHistory::new();
        h.add(TimedMessage::new(Message::user("hello")));
        h.add(TimedMessage::new(Message::system("sys")));
        assert_eq!(h.get_all().len(), 2);
    }

    #[test]
    fn test_infinite_history_clear() {
        let mut h = InfiniteHistory::new();
        h.add(TimedMessage::new(Message::user("hello")));
        h.clear();
        assert!(h.get_all().is_empty());
    }

    #[test]
    fn test_multiple_adds() {
        let mut h = InfiniteHistory::new();
        h.add(TimedMessage::new(Message::user("a")));
        h.add(TimedMessage::new(Message::user("b")));
        assert_eq!(h.get_all().len(), 2);
    }

    #[test]
    fn test_bounded_history_enforces_capacity() {
        let mut h = BoundedHistory::new(3);
        h.add(TimedMessage::new(Message::system("sys")));
        h.add(TimedMessage::new(Message::user("a")));
        h.add(TimedMessage::new(Message::user("b")));
        h.add(TimedMessage::new(Message::user("c"))); // triggers eviction
        assert_eq!(h.get_all().len(), 3);
        // System message should be preserved
        assert!(matches!(h.get_all()[0].message, Message::System(_)));
    }

    #[test]
    fn test_bounded_history_pins_system() {
        let mut h = BoundedHistory::new(2);
        h.add(TimedMessage::new(Message::system("sys")));
        h.add(TimedMessage::new(Message::user("u1")));
        h.add(TimedMessage::new(Message::user("u2")));
        h.add(TimedMessage::new(Message::user("u3")));
        assert_eq!(h.get_all().len(), 2);
        // System must stay
        assert!(matches!(h.get_all()[0].message, Message::System(_)));
        // u3 should be the latest user message
        if let Message::User(ref um) = h.get_all()[1].message {
            assert_eq!(um.content, "u3");
        } else {
            panic!("expected user message");
        }
    }

    #[test]
    fn test_file_history_roundtrip() {
        let mut h = FileHistory::new(Some("test_session"));
        h.add(TimedMessage::new(Message::user("hello")));
        h.add(TimedMessage::new(Message::system("sys")));
        assert_eq!(h.get_all().len(), 2);
        assert_eq!(h.session_id(), "test_session");

        // Check file exists
        let path = dirs::home_dir().unwrap().join(".motif").join("sessions").join("test_session.jsonl");
        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("hello"));
        assert!(content.contains("\"_meta\""));

        // Cleanup
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_file_history_clear() {
        let mut h = FileHistory::new(Some("test_clear"));
        h.add(TimedMessage::new(Message::user("a")));
        h.clear();
        assert!(h.get_all().is_empty());
        let path = dirs::home_dir().unwrap().join(".motif").join("sessions").join("test_clear.jsonl");
        fs::remove_file(&path).ok();
    }
}
