//! File-based session persistence. `FileHistory` implements `motif::History`,
//! appending each message as a JSONL line to disk.

use motif::History;
use motif::TimedMessage;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

pub struct FileHistory {
    messages: Vec<TimedMessage>,
    session_id: String,
    sessions_dir: PathBuf,
    file: Option<BufWriter<File>>,
}

impl FileHistory {
    pub fn new(session_id: Option<&str>) -> Self {
        let dir = sessions_dir();
        let _ = fs::create_dir_all(&dir);
        let id = session_id.map(|s| s.to_string()).unwrap_or_else(nano_id);
        let path = dir.join(format!("{}.jsonl", id));
        let file = OpenOptions::new().create(true).append(true).open(&path).ok().map(BufWriter::new);
        let mut fh = Self { messages: vec![], session_id: id, sessions_dir: dir, file };
        fh.append_raw(&serde_json::json!({"_meta":true,"created":now_str()}));
        fh.save_latest();
        fh.save_index().ok();
        fh
    }

    pub fn load(id: &str) -> Option<Self> {
        let dir = sessions_dir();
        let path = dir.join(format!("{}.jsonl", id));
        let data = fs::read_to_string(&path).ok()?;
        let messages: Vec<TimedMessage> = data.lines()
            .filter(|l| !l.trim().is_empty() && !l.contains("\"_meta\""))
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        if messages.is_empty() { return None; }
        let file = OpenOptions::new().create(true).append(true).open(&path).ok().map(BufWriter::new);
        let fh = Self { messages, session_id: id.to_string(), sessions_dir: dir, file };
        fh.save_latest();
        Some(fh)
    }

    pub fn list() -> Vec<serde_json::Value> {
        let idx = sessions_dir().join("index.json");
        fs::read_to_string(&idx).ok()
            .and_then(|d| serde_json::from_str(&d).ok())
            .unwrap_or_default()
    }

    pub fn session_id(&self) -> &str { &self.session_id }

    /// Delete a session by ID. Returns true if deleted, false if not found.
    pub fn delete(id: &str) -> bool {
        let dir = sessions_dir();
        let path = dir.join(format!("{}.jsonl", id));
        let deleted = fs::remove_file(&path).is_ok();
        if deleted {
            // Remove from index
            let mut idx: Vec<serde_json::Value> = Self::list();
            idx.retain(|e| e.get("id").and_then(|i| i.as_str()) != Some(id));
            let _ = fs::write(dir.join("index.json"), serde_json::to_string_pretty(&idx).unwrap_or_default());
            // Clear latest if it pointed to this session
            let latest = dir.join("latest");
            if fs::read_to_string(&latest).ok().as_deref() == Some(&format!("{}.jsonl", id)) {
                let _ = fs::remove_file(&latest);
            }
        }
        deleted
    }

    /// Export session contents as a formatted string.
    pub fn export(id: &str) -> Option<String> {
        let dir = sessions_dir();
        let path = dir.join(format!("{}.jsonl", id));
        let data = fs::read_to_string(&path).ok()?;
        let messages: Vec<serde_json::Value> = data.lines()
            .filter(|l| !l.trim().is_empty() && !l.contains("\"_meta\""))
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        if messages.is_empty() { return None; }
        serde_json::to_string_pretty(&messages).ok()
    }

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
        #[cfg(not(unix))] { let _ = fs::write(&latest, &target); }
    }

    fn save_index(&self) -> std::io::Result<()> {
        let mut idx: Vec<serde_json::Value> = Self::list();
        let first = self.messages.first().and_then(|m| serde_json::to_value(m).ok());
        idx.retain(|e| e.get("id").and_then(|i| i.as_str()) != Some(&self.session_id));
        idx.push(serde_json::json!({"id":self.session_id,"date":now_str(),"count":self.messages.len(),"first":first}));
        fs::write(self.sessions_dir.join("index.json"), serde_json::to_string_pretty(&idx)?)
    }
}

impl History for FileHistory {
    fn add(&mut self, m: TimedMessage) {
        self.append_raw(&serde_json::to_value(&m).unwrap_or_default());
        self.messages.push(m);
    }
    fn get_all(&self) -> &[TimedMessage] { &self.messages }
    fn clear(&mut self) {
        self.messages.clear();
        self.session_id = nano_id();
        let path = self.sessions_dir.join(format!("{}.jsonl", self.session_id));
        self.file = OpenOptions::new().create(true).append(true).open(&path).ok().map(BufWriter::new);
        self.append_raw(&serde_json::json!({"_meta":true,"created":now_str()}));
        self.save_latest();
        self.save_index().ok();
    }
}

fn sessions_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".motif").join("sessions")
}
fn nano_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("{:016x}", t).chars().take(12).collect()
}
fn now_str() -> String { chrono::Local::now().format("%Y-%m-%d %H:%M").to_string() }

#[cfg(test)]
mod tests {
    use super::*;
    use motif::Message;

    #[test]
    fn test_roundtrip() {
        let mut h = FileHistory::new(Some("test_sess"));
        h.add(TimedMessage::new(Message::user("hello")));
        assert_eq!(h.get_all().len(), 1);
        let path = sessions_dir().join("test_sess.jsonl");
        assert!(path.exists());
        fs::remove_file(&path).ok();
    }
}
