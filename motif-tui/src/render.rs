//! Rendering — ratatui draw logic, testable with TestBackend.
//!
//! Layout: top area (chat messages) | bottom area (input box) | status bar

use crate::app::{App, InputMode, Message, Role};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

/// Render the full TUI frame.
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = layout(frame.area());

    render_chat(frame, chunks[0], app);
    render_input(frame, chunks[1], app);
    render_status(frame, chunks[2], app);
}

fn layout(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),  // chat area
            Constraint::Length(3), // input area
            Constraint::Length(1), // status bar
        ])
        .split(area)
        .to_vec()
}

fn render_chat(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = Vec::new();

    for msg in &app.messages {
        let (prefix, style) = match msg.role {
            Role::User => ("> ", Style::default().fg(Color::Cyan)),
            Role::Assistant => ("", Style::default()),
            Role::System => ("# ", Style::default().fg(Color::Yellow)),
        };
        for line in msg.content.lines() {
            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::raw(line.to_string()),
            ]));
        }
    }

    // Show streaming content if active
    if app.is_streaming && !app.streaming.is_empty() {
        for line in app.streaming.lines() {
            lines.push(Line::from(Span::raw(line.to_string())));
        }
    }

    let text = Text::from(lines);
    let block = Block::default()
        .title(" Chat ")
        .borders(Borders::ALL);
    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll.offset as u16, 0));

    frame.render_widget(paragraph, area);
}

fn render_input(frame: &mut Frame, area: Rect, app: &App) {
    let (border_style, title) = match app.input_mode {
        InputMode::Normal => (Style::default(), " Input (Enter to send) "),
        InputMode::Processing => (Style::default().fg(Color::Yellow), " Processing... "),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let text = if app.input_mode == InputMode::Processing {
        "..."
    } else {
        &app.input
    };

    let paragraph = Paragraph::new(text.to_string())
        .block(block)
        .style(Style::default());

    frame.render_widget(paragraph, area);
}

fn render_status(frame: &mut Frame, area: Rect, app: &App) {
    let text = Span::styled(
        &app.status_text,
        Style::default().fg(Color::DarkGray),
    );
    let line = Line::from(text);
    frame.render_widget(Paragraph::new(line), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_layout_produces_three_areas() {
        let area = Rect::new(0, 0, 80, 24);
        let chunks = layout(area);
        assert_eq!(chunks.len(), 3);
        // chat area should be largest
        assert!(chunks[0].height > 0);
        // input area fixed at 3 lines
        assert_eq!(chunks[1].height, 3);
        // status bar fixed at 1 line
        assert_eq!(chunks[2].height, 1);
    }

    #[test]
    fn test_render_empty_app() {
        let app = App::new();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, &app))
            .unwrap();
        // Verify buffer contains the block titles
        let buffer = terminal.backend().buffer();
        let text = buffer_to_string(buffer);
        assert!(text.contains("Chat"), "Should have Chat block");
        assert!(text.contains("Input"), "Should have Input block");
    }

    #[test]
    fn test_render_with_message() {
        let mut app = App::new();
        app.messages.push(Message {
            role: Role::User,
            content: "hello world".into(),
        });
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, &app))
            .unwrap();
        let text = buffer_to_string(terminal.backend().buffer());
        assert!(text.contains("hello world"), "Should show message: {}", text);
    }

    #[test]
    fn test_render_streaming() {
        let mut app = App::new();
        app.is_streaming = true;
        app.streaming = String::from("streaming content...");
        app.input_mode = InputMode::Processing;
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, &app))
            .unwrap();
        let text = buffer_to_string(terminal.backend().buffer());
        assert!(
            text.contains("streaming content"),
            "Should show streaming: {}",
            text
        );
        assert!(
            text.contains("Processing"),
            "Should show Processing: {}",
            text
        );
    }

    fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
        let mut s = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                if let Some(cell) = buffer.cell((x, y)) {
                    s.push(cell.symbol().chars().next().unwrap_or(' '));
                }
            }
            s.push('\n');
        }
        s
    }
}
