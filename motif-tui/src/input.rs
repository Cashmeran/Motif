//! Text input area — wrapper around tui-textarea.
//! From: Aegis `app/input.rs` pattern (InputState + submit/cancel).

use tui_textarea::TextArea;

/// Input widget wrapping tui-textarea.
/// Manages text entry and submit/cancel lifecycle.
pub struct InputWidget {
    pub textarea: TextArea<'static>,
    pub cursor_visible: bool,
}

impl InputWidget {
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_placeholder_text("Type a message, Enter to send...");
        Self {
            textarea,
            cursor_visible: true,
        }
    }

    /// Insert a character at cursor.
    pub fn insert_char(&mut self, ch: char) {
        self.textarea.insert_char(ch);
    }

    /// Delete character before cursor.
    pub fn delete_back(&mut self) {
        self.textarea.delete_char();
    }

    /// Get the current text content.
    pub fn text(&self) -> String {
        self.textarea.lines().join("\n")
    }

    /// Clear the input area.
    pub fn clear(&mut self) {
        self.textarea = TextArea::default();
        self.textarea.set_placeholder_text("Type a message, Enter to send...");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_input_is_empty() {
        let w = InputWidget::new();
        assert!(w.text().is_empty());
    }

    #[test]
    fn test_insert_and_delete() {
        let mut w = InputWidget::new();
        w.insert_char('a');
        w.insert_char('b');
        assert_eq!(w.text(), "ab");
        w.delete_back();
        assert_eq!(w.text(), "a");
    }

    #[test]
    fn test_clear() {
        let mut w = InputWidget::new();
        w.insert_char('x');
        w.clear();
        assert!(w.text().is_empty());
    }
}
