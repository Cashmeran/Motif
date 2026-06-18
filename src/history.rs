use crate::types::TimedMessage;

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
            .filter(|m| !matches!(m.message, crate::types::Message::System(_)))
            .count();
        let can_evict = non_sys_count.min(excess);
        let mut evicted = 0;
        self.messages.retain(|m| {
            if evicted >= can_evict {
                return true;
            }
            if matches!(m.message, crate::types::Message::System(_)) && non_sys_count > 0 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Message;

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
}
