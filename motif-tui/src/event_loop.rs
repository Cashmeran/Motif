//! TUI event loop — tokio::select! over crossterm events + render throttle.
//! From: Aegis `app/mod.rs` pattern.

use crate::app::{App, InputMode, Message, Role};
use crate::input::InputWidget;
use crate::render;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use futures_util::StreamExt;
use motif::Agent;
use ratatui::Terminal;
use std::io::Stdout;
use std::time::{Duration, Instant};

pub async fn run(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<Stdout>>,
    app: &mut App,
    input_widget: &mut InputWidget,
    agent: &mut Agent,
) {
    let tick = Duration::from_millis(16);
    let mut last_render = Instant::now();
    let mut events = crossterm::event::EventStream::new();

    loop {
        tokio::select! {
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(event)) => {
                        match event {
                            Event::Key(key) if key.kind == KeyEventKind::Press => {
                                if !handle_key(app, input_widget, key.code) {
                                    break;
                                }
                                if key.code == KeyCode::Enter && app.input_mode == InputMode::Processing {
                                    let prompt = app.submit_input();
                                    if let Some(p) = prompt {
                                        input_widget.clear();
                                        call_agent(agent, app, &p).await;
                                    }
                                }
                            }
                            Event::Resize(_, _) => {
                                last_render = Instant::now() - tick;
                            }
                            _ => {}
                        }
                    }
                    Some(Err(_)) | None => break,
                }
            }
            _ = tokio::time::sleep(tick) => {}
        }

        if last_render.elapsed() >= tick {
            app.input = input_widget.text();
            terminal
                .draw(|f| render::render(f, app))
                .expect("render failed");
            last_render = Instant::now();
        }
    }
}

fn handle_key(app: &mut App, input: &mut InputWidget, code: KeyCode) -> bool {
    match code {
        KeyCode::Esc => return false,
        KeyCode::Char(ch) if app.input_mode == InputMode::Normal => {
            input.insert_char(ch);
        }
        KeyCode::Backspace if app.input_mode == InputMode::Normal => {
            input.delete_back();
        }
        KeyCode::Enter if app.input_mode == InputMode::Normal => {
            app.input = input.text();
            app.input_mode = InputMode::Processing;
        }
        KeyCode::PageUp => app.scroll.scroll_up(5),
        KeyCode::PageDown => app.scroll.scroll_down(5),
        _ => {}
    }
    true
}

async fn call_agent(agent: &mut Agent, app: &mut App, prompt: &str) {
    match agent.chat(prompt).await {
        Ok(result) => {
            app.messages.push(Message {
                role: Role::Assistant,
                content: result,
            });
            app.input_mode = InputMode::Normal;
            app.status_text = format!(
                "Model: {} | Tokens: {} | Messages: {}",
                agent.get_model(),
                agent.total_tokens_used(),
                app.messages.len(),
            );
        }
        Err(e) => {
            app.messages.push(Message {
                role: Role::System,
                content: format!("Error: {}", e),
            });
            app.input_mode = InputMode::Normal;
            app.status_text = format!("Error: {}", e);
        }
    }
}
