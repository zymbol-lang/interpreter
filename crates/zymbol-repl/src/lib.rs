//! Interactive REPL for Zymbol-Lang
//!
//! Provides an interactive Read-Eval-Print Loop with:
//! - Command history (up/down arrows)
//! - Text selection (Shift+arrows)
//! - Clipboard support (Ctrl+C/X/V)
//! - Variable inspection (name?)
//! - Built-in commands (HELP, EXIT, VARS, CLEAR, HISTORY)

mod colors;
mod line_editor;
mod repl;

pub use repl::Repl;
