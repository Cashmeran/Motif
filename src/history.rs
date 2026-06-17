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
}
