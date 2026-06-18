//! Motif CLI — terminal chat. Config at `~/.motif/config.json`.

mod cmd;
mod commands;
mod config;

use commands::{Outcome, Registry};
use rustyline::{error::ReadlineError, DefaultEditor};

#[tokio::main]
async fn main() {
    let cfg = config::load_or_create();
    let mut agent = config::make_agent(&cfg);
    let reg = Registry::new();

    println!("Motif CLI · model: {} · /help", cfg.model);

    let mut editor = DefaultEditor::new().expect("Failed to init line editor");

    loop {
        match editor.readline("> ") {
            Ok(line) => {
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() { continue; }

                match reg.handle(&line, &mut agent, &cfg).await {
                    Outcome::Continue => {}
                    Outcome::Exit => break,
                    Outcome::PassToAgent(text) => {
                        let _ = editor.add_history_entry(&text);
                        let result = if cfg.streaming.unwrap_or(true) && agent.wants_streaming() {
                            agent.chat_stream(&text).await
                        } else {
                            agent.chat(&text).await
                        };
                        match result {
                            Ok(r) => println!("\n{}\n", r),
                            Err(e) => eprintln!("Error: {}", e),
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => { println!("^C"); continue; }
            Err(ReadlineError::Eof) => break,
            Err(e) => { eprintln!("Error: {}", e); break; }
        }
    }
}
