//! Motif TUI — dual-panel terminal chat.
//!
//! Layers (bottom → top):
//!   terminal.rs — PanicRestoreHook + mode actions (from Aegis)
//!   app.rs      — Pure state, unit-testable
//!   render.rs   — ratatui drawing, snapshot-testable
//!   input.rs    — tui-textarea wrapper
//!   event_loop.rs — tokio::select! main loop
//!   main.rs     — Bootstrap + wiring (this file)

mod app;
mod config;
mod event_loop;
mod input;
mod render;
mod terminal;

use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

#[tokio::main]
async fn main() {
    // ── Load config + build agent ──
    let _tui_cfg = config::load_or_default();
    let agent_cfg = motif_cli::config::load_or_create();
    let mut agent = motif_cli::config::make_agent(&agent_cfg);

    // ── TUI state ──
    let mut app = app::App::new();
    app.status_text = format!("Model: {} | Ready", agent.get_model());
    let mut input_widget = input::InputWidget::new();

    // ── Enter TUI mode ──
    let (_, hook) = terminal::enter_tui()
        .expect("Failed to enter TUI mode");
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut tui_terminal = Terminal::new(backend)
        .expect("Failed to create terminal");

    // ── Main event loop ──
    event_loop::run(&mut tui_terminal, &mut app, &mut input_widget, &mut agent).await;

    // ── Exit TUI mode + restore terminal ──
    // In raw mode, any stdout() handle works for terminal operations
    terminal::exit_tui(&mut std::io::stdout(), &hook)
        .expect("Failed to restore terminal");
}
