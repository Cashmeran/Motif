//! Integration tests for motif-tui using ratatui TestBackend.
//! No real terminal required — tests run in CI.

use motif_tui::app::{App, InputMode, Role, ScrollState};
use motif_tui::render;

#[test]
fn test_app_lifecycle() {
    let mut app = App::new();
    assert!(app.messages.is_empty());
    assert_eq!(app.input_mode, InputMode::Normal);

    // Submit input
    app.input = String::from("hello agent");
    let prompt = app.submit_input();
    assert_eq!(prompt, Some("hello agent".into()));
    assert_eq!(app.input_mode, InputMode::Processing);

    // Stream
    app.append_stream("Hello");
    app.append_stream(" there");
    assert_eq!(app.streaming, "Hello there");

    // Finish
    app.finish_stream();
    assert_eq!(app.messages.len(), 2); // user + assistant
    assert_eq!(app.messages[1].role, Role::Assistant);
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn test_scroll_clamping() {
    let mut s = ScrollState::new();
    s.total_lines = 100;
    s.viewport_height = 20;

    // Scroll up max
    s.scroll_up(1000);
    assert_eq!(s.offset, 80); // 100 - 20 = 80 max

    // Scroll to bottom
    s.scroll_to_bottom();
    assert_eq!(s.offset, 0);

    // Can't scroll below 0
    s.scroll_down(100);
    assert_eq!(s.offset, 0);
}

#[test]
fn test_render_snapshot_empty() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let app = App::new();
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render::render(f, &app)).unwrap();

    let buffer = terminal.backend().buffer();
    let area = buffer.area();
    assert!(area.width == 80);
    assert!(area.height == 24);
}

#[test]
fn test_render_snapshot_with_messages() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = App::new();
    app.messages.push(motif_tui::app::Message {
        role: Role::User,
        content: "test message".into(),
    });
    app.messages.push(motif_tui::app::Message {
        role: Role::Assistant,
        content: "response".into(),
    });

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render::render(f, &app)).unwrap();

    let buffer = terminal.backend().buffer();
    let text: String = (0..buffer.area().height)
        .flat_map(|y| {
            (0..buffer.area().width)
                .filter_map(move |x| buffer.cell((x, y)).map(|c| c.symbol().chars().next().unwrap_or(' ')))
                .chain(std::iter::once('\n'))
        })
        .collect();

    assert!(text.contains("test message"), "Render should contain user message");
    assert!(text.contains("response"), "Render should contain assistant response");
}
