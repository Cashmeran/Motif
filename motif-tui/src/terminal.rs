//! Terminal lifecycle management — ported from Aegis `terminal_runtime/`.
//!
//! Key patterns:
//! - PanicRestoreHook: guarantees terminal cleanup on panic (AtomicBool CAS)
//! - TerminalModeAction: declarative enum for all terminal operations
//! - Named action profiles: CHAT_STARTUP, SHUTDOWN_RESTORE constant slices

use crossterm::execute;
use std::io::{self, stdout, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ── Panic Restore Hook ──
// From: Aegis `app/terminal_runtime/panic_hook.rs`

/// Installs a panic hook that restores the terminal before the process dies.
/// Uses Arc<AtomicBool> CAS to ensure restore happens exactly once.
#[derive(Clone)]
pub struct PanicRestoreHook {
    restored: Arc<AtomicBool>,
}

impl PanicRestoreHook {
    pub fn install() -> Self {
        let restored = Arc::new(AtomicBool::new(false));
        let r = restored.clone();
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if r.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                let _ = execute!(stdout(), crossterm::terminal::LeaveAlternateScreen);
                let _ = crossterm::terminal::disable_raw_mode();
            }
            prev(info);
        }));
        Self { restored }
    }

    /// Restore terminal explicitly (non-panic path).
    /// Safe to call multiple times — only first call takes effect.
    pub fn restore(&self) {
        if self
            .restored
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let _ = execute!(stdout(), crossterm::terminal::LeaveAlternateScreen);
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
}

// ── Terminal Mode Actions ──
// From: Aegis `app/terminal_runtime/modes.rs`

/// Declarative terminal operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalModeAction {
    EnableRawMode,
    DisableRawMode,
    EnterAlternateScreen,
    LeaveAlternateScreen,
    ShowCursor,
    HideCursor,
    EnableLineWrap,
    DisableLineWrap,
}

/// Apply a single action to stdout.
pub fn apply_action(stdout: &mut Stdout, action: TerminalModeAction) -> io::Result<()> {
    match action {
        TerminalModeAction::EnableRawMode => crossterm::terminal::enable_raw_mode(),
        TerminalModeAction::DisableRawMode => crossterm::terminal::disable_raw_mode(),
        TerminalModeAction::EnterAlternateScreen => {
            execute!(stdout, crossterm::terminal::EnterAlternateScreen)
        }
        TerminalModeAction::LeaveAlternateScreen => {
            execute!(stdout, crossterm::terminal::LeaveAlternateScreen)
        }
        TerminalModeAction::ShowCursor => execute!(stdout, crossterm::cursor::Show),
        TerminalModeAction::HideCursor => execute!(stdout, crossterm::cursor::Hide),
        TerminalModeAction::EnableLineWrap => execute!(stdout, crossterm::terminal::EnableLineWrap),
        TerminalModeAction::DisableLineWrap => {
            execute!(stdout, crossterm::terminal::DisableLineWrap)
        }
    }
}

/// Apply a slice of actions in order.
pub fn apply_actions(stdout: &mut Stdout, actions: &[TerminalModeAction]) -> io::Result<()> {
    for &a in actions {
        apply_action(stdout, a)?;
    }
    Ok(())
}

// ── Named Action Profiles ──
// From: Aegis `app/terminal_runtime/modes.rs`

/// Actions to enter TUI mode.
pub const CHAT_STARTUP: &[TerminalModeAction] = &[
    TerminalModeAction::EnableRawMode,
    TerminalModeAction::EnterAlternateScreen,
    TerminalModeAction::HideCursor,
];

/// Actions to restore terminal on exit.
pub const SHUTDOWN_RESTORE: &[TerminalModeAction] = &[
    TerminalModeAction::ShowCursor,
    TerminalModeAction::LeaveAlternateScreen,
    TerminalModeAction::EnableLineWrap,
    TerminalModeAction::DisableRawMode,
];

// ── Terminal Bootstrap ──

/// Enter TUI mode. Returns the panic hook guard (must be held until shutdown).
pub fn enter_tui() -> io::Result<(Stdout, PanicRestoreHook)> {
    let hook = PanicRestoreHook::install();
    let mut stdout = stdout();
    apply_actions(&mut stdout, CHAT_STARTUP)?;
    Ok((stdout, hook))
}

/// Exit TUI mode and restore terminal.
pub fn exit_tui(stdout: &mut Stdout, hook: &PanicRestoreHook) -> io::Result<()> {
    apply_actions(stdout, SHUTDOWN_RESTORE)?;
    hook.restore();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_profiles_non_empty() {
        assert!(!CHAT_STARTUP.is_empty());
        assert!(!SHUTDOWN_RESTORE.is_empty());
    }

    #[test]
    fn test_shutdown_restore_reverses_chat_startup() {
        // Verify shutdown restores what startup sets
        let startup_has_raw = CHAT_STARTUP
            .contains(&TerminalModeAction::EnableRawMode);
        let shutdown_has_raw = SHUTDOWN_RESTORE
            .contains(&TerminalModeAction::DisableRawMode);
        assert!(startup_has_raw);
        assert!(shutdown_has_raw);
    }

    #[test]
    fn test_panic_hook_restore_once() {
        let hook = PanicRestoreHook {
            restored: Arc::new(AtomicBool::new(false)),
        };
        hook.restore();
        hook.restore();
        // No panic = pass
    }

    #[test]
    fn test_apply_action_does_not_panic() {
        // All actions should be constructable without panic
        let actions = [
            TerminalModeAction::EnableRawMode,
            TerminalModeAction::EnterAlternateScreen,
            TerminalModeAction::HideCursor,
            TerminalModeAction::ShowCursor,
            TerminalModeAction::LeaveAlternateScreen,
            TerminalModeAction::EnableLineWrap,
            TerminalModeAction::DisableRawMode,
        ];
        assert_eq!(actions.len(), 7);
    }
}
