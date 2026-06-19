use crate::types::TimedMessage;

pub trait History: Send + Sync {
    fn add(&mut self, message: TimedMessage);
    fn get_all(&self) -> &[TimedMessage];
    fn clear(&mut self);
}

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
    fn add(&mut self, m: TimedMessage) {
        self.messages.push(m);
    }
    fn get_all(&self) -> &[TimedMessage] {
        &self.messages
    }
    fn clear(&mut self) {
        self.messages.clear();
    }
}

pub struct BoundedHistory {
    messages: Vec<TimedMessage>,
    capacity: usize,
}
impl BoundedHistory {
    pub fn new(n: usize) -> Self {
        Self {
            messages: vec![],
            capacity: n.max(1),
        }
    }
}
impl History for BoundedHistory {
    fn add(&mut self, m: TimedMessage) {
        self.messages.push(m);
        let excess = self.messages.len().saturating_sub(self.capacity);
        if excess == 0 {
            return;
        }
        let ns = self
            .messages
            .iter()
            .filter(|m| !matches!(m.message, crate::types::Message::System(_)))
            .count();
        let can = ns.min(excess);
        let mut ev = 0;
        self.messages.retain(|m| {
            if ev >= can {
                return true;
            }
            if matches!(m.message, crate::types::Message::System(_)) && ns > 0 {
                return true;
            }
            ev += 1;
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
