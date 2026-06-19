//! Motif CLI — terminal chat with oneshot and REPL modes.
//! Config at `~/.motif/config.json`. Sessions at `~/.motif/sessions/`.

mod cmd;
mod commands;
mod config;
mod hooks;

use commands::{Outcome, Registry};
use rustyline::{error::ReadlineError, DefaultEditor};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    // ── CLI argument parsing ──
    if args.len() > 1 {
        match args[1].as_str() {
            "-h" | "--help" => {
                println!("Motif CLI — Rust agent REPL");
                println!();
                println!("USAGE:");
                println!("  motif                Start interactive REPL");
                println!("  motif -p <prompt>    Single-shot: run one prompt and exit");
                println!("  motif -h, --help     Show this help");
                println!("  motif -V, --version  Show version");
                println!();
                println!("CONFIG:  ~/.motif/config.json");
                println!("SESSION: ~/.motif/sessions/");
                return;
            }
            "-V" | "--version" => {
                println!("motif {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "-p" | "--prompt" => {
                let prompt = args[2..].join(" ");
                if prompt.is_empty() {
                    eprintln!("Error: -p requires a prompt argument");
                    std::process::exit(1);
                }
                let cfg = config::load_or_create();
                let mut agent = config::make_agent(&cfg);
                match agent.chat(&prompt).await {
                    Ok(r) => println!("{}", r),
                    Err(e) => eprintln!("Error: {}", e),
                }
                return;
            }
            _ => {}
        }
    }

    // ── REPL mode ──
    let cfg = config::load_or_create();
    let mut agent = config::make_agent(&cfg);
    let reg = Registry::new();

    println!("Motif CLI · model: {} · /help", cfg.model);

    let mut editor = DefaultEditor::new().expect("Failed to init line editor");

    loop {
        match editor.readline("> ") {
            Ok(line) => {
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() {
                    continue;
                }

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
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }
}
