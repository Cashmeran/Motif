//! App state — decoupled from terminal I/O, fully unit-testable.
//!
//! App holds all mutable state: messages, input, scroll position, status.
//! No dependency on crossterm or ratatui — pure logic.

/// A single message in the chat history.
#[derive(Clone, Debug, PartialEq)]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// Input mode determines what the input area is doing.
#[derive(Clone, Debug, PartialEq)]
pub enum InputMode {
    /// Normal text input — user is typing a message
    Normal,
    /// Agent is processing — input is read-only
    Processing,
}

/// Scroll direction for the message view.
#[derive(Clone, Debug)]
pub struct ScrollState {
    /// Total line count in the message area
    pub total_lines: usize,
    /// Current scroll offset (0 = bottom/newest)
    pub offset: usize,
    /// Viewport height in lines
    pub viewport_height: usize,
}

impl ScrollState {
    pub fn new() -> Self {
        Self {
            total_lines: 0,
            offset: 0,
            viewport_height: 1,
        }
    }

    /// Scroll up by `n` lines, clamped.
    pub fn scroll_up(&mut self, n: usize) {
        let max_offset = self.total_lines.saturating_sub(self.viewport_height);
        self.offset = (self.offset + n).min(max_offset);
    }

    /// Scroll down by `n` lines, clamped to zero.
    pub fn scroll_down(&mut self, n: usize) {
        self.offset = self.offset.saturating_sub(n);
    }

    /// Scroll to bottom (newest messages).
    pub fn scroll_to_bottom(&mut self) {
        self.offset = 0;
    }

    /// Update viewport size after resize.
    pub fn set_viewport(&mut self, height: usize) {
        self.viewport_height = height.max(1);
    }
}

/// Central app state. Thread-safe only via App:: methods.
pub struct App {
    pub messages: Vec<Message>,
    pub input: String,
    pub input_mode: InputMode,
    pub scroll: ScrollState,
    pub status_text: String,
    /// Accumulating streaming content (appended by stream deltas)
    pub streaming: String,
    /// Whether the agent is currently streaming a response
    pub is_streaming: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            input_mode: InputMode::Normal,
            scroll: ScrollState::new(),
            status_text: String::from("Ready"),
            streaming: String::new(),
            is_streaming: false,
        }
    }

    /// Add a user message and clear the input buffer.
    pub fn submit_input(&mut self) -> Option<String> {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return None;
        }
        self.messages.push(Message {
            role: Role::User,
            content: text.clone(),
        });
        self.input.clear();
        self.input_mode = InputMode::Processing;
        self.status_text = String::from("Thinking...");
        self.streaming.clear();
        self.is_streaming = true;
        self.scroll.scroll_to_bottom();
        Some(text)
    }

    /// Append a streaming content delta.
    pub fn append_stream(&mut self, delta: &str) {
        self.streaming.push_str(delta);
    }

    /// Finalize streaming: commit to messages.
    pub fn finish_stream(&mut self) {
        if !self.streaming.is_empty() {
            self.messages.push(Message {
                role: Role::Assistant,
                content: self.streaming.clone(),
            });
            self.streaming.clear();
        }
        self.is_streaming = false;
        self.input_mode = InputMode::Normal;
        self.status_text = String::from("Ready");
        self.scroll.scroll_to_bottom();
    }

    /// Add a system message (tool result, error, etc.).
    pub fn add_system_msg(&mut self, text: &str) {
        self.messages.push(Message {
            role: Role::System,
            content: text.to_string(),
        });
    }

    /// Total visible lines in the message area (for scroll calculation).
    pub fn total_message_lines(&self) -> usize {
        self.messages
            .iter()
            .map(|m| m.content.lines().count())
            .sum::<usize>()
            + if self.is_streaming {
                self.streaming.lines().count()
            } else {
                0
            }
    }

    /// Clear all messages and reset state.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.input.clear();
        self.streaming.clear();
        self.input_mode = InputMode::Normal;
        self.status_text = String::from("Ready");
        self.scroll = ScrollState::new();
        self.is_streaming = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_submit_empty_input_returns_none() {
        let mut app = App::new();
        assert_eq!(app.submit_input(), None);
        assert_eq!(app.messages.len(), 0);
    }

    #[test]
    fn test_submit_input_adds_message_and_clears() {
        let mut app = App::new();
        app.input = String::from("hello");
        let result = app.submit_input();
        assert_eq!(result, Some("hello".into()));
        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].role, Role::User);
        assert!(app.input.is_empty());
        assert_eq!(app.input_mode, InputMode::Processing);
    }

    #[test]
    fn test_stream_append_and_finish() {
        let mut app = App::new();
        app.input = String::from("test");
        app.submit_input();

        app.append_stream("Hello");
        app.append_stream(" World");
        assert_eq!(app.streaming, "Hello World");

        app.finish_stream();
        assert!(app.streaming.is_empty());
        assert_eq!(app.messages.len(), 2); // user + assistant
        assert_eq!(app.messages[1].role, Role::Assistant);
        assert_eq!(app.messages[1].content, "Hello World");
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_scroll_up_down() {
        let mut s = ScrollState::new();
        s.total_lines = 50;
        s.viewport_height = 10;
        s.scroll_up(5);
        assert_eq!(s.offset, 5);
        s.scroll_down(3);
        assert_eq!(s.offset, 2);
        s.scroll_down(10); // clamped
        assert_eq!(s.offset, 0);
    }

    #[test]
    fn test_clear_resets_all() {
        let mut app = App::new();
        app.input = String::from("test");
        app.submit_input();
        app.finish_stream();
        app.clear();
        assert!(app.messages.is_empty());
        assert!(app.input.is_empty());
        assert_eq!(app.input_mode, InputMode::Normal);
    }
}
