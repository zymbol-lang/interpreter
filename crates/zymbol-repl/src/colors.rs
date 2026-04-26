//! Color utilities for the REPL prompt

use crossterm::style::{Color, Stylize};

/// Returns the colored prompt string "zymbol> "
pub fn prompt() -> String {
    "zymbol> ".with(Color::Cyan).to_string()
}

/// Returns the visible length of the prompt (without ANSI escape codes)
/// Used for cursor positioning
pub const fn prompt_visible_length() -> usize {
    8 // "zymbol> " is 8 characters
}

/// Format an error message in red
pub fn error(message: &str) -> String {
    message.with(Color::Red).to_string()
}

/// Format a success message in green
#[allow(dead_code)]
pub fn success(message: &str) -> String {
    message.with(Color::Green).to_string()
}

/// Format a type name in yellow
pub fn type_name(name: &str) -> String {
    name.with(Color::Yellow).to_string()
}

/// Format a value in white/default
pub fn value(val: &str) -> String {
    val.with(Color::White).to_string()
}

/// Format a help command in cyan
pub fn command(cmd: &str) -> String {
    cmd.with(Color::Cyan).bold().to_string()
}

/// The `=>` arrow shown before an evaluated result
pub fn result_arrow() -> String {
    "=>".with(Color::Green).bold().to_string()
}

/// Dim text for secondary info (e.g., `::` and the type label)
pub fn dim(s: &str) -> String {
    s.with(Color::DarkGrey).to_string()
}
