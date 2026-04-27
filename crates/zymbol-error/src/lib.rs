//! Error reporting system for Zymbol-Lang
//!
//! Provides rich diagnostic messages with source context and colorized output.

use owo_colors::OwoColorize;
use std::fmt;
use zymbol_span::{SourceMap, Span};

/// Severity of a diagnostic message
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "{}", "error".red().bold()),
            Severity::Warning => write!(f, "{}", "warning".yellow().bold()),
            Severity::Note => write!(f, "{}", "note".blue().bold()),
        }
    }
}

/// A diagnostic message (error, warning, or note)
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
    pub notes: Vec<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            span: None,
            notes: Vec::new(),
            help: None,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            span: None,
            notes: Vec::new(),
            help: None,
        }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Print the diagnostic to stderr with colors and source context
    pub fn emit(&self, source_map: &SourceMap) {
        eprintln!("{}: {}", self.severity, self.message);

        if let Some(span) = &self.span {
            if let Some(file) = source_map.get(span.file_id) {
                let line_num = span.start.line;

                // Print file location
                eprintln!(
                    "  {} {}:{}:{}",
                    "-->".blue().bold(),
                    file.name,
                    line_num,
                    span.start.column
                );

                // Print source line if available
                if let Some(line) = file.line(line_num) {
                    let line_str = format!("{:4}", line_num);
                    eprintln!("{} {}", line_str.blue().bold(), "|".blue());
                    eprintln!("{} {} {}", line_str.blue().bold(), "|".blue(), line);

                    // Print caret indicator
                    // Column is 1-indexed, so subtract 1 for correct positioning
                    let indent = " ".repeat((span.start.column - 1) as usize);
                    let caret_len = span.end.column.saturating_sub(span.start.column).max(1);
                    let carets = "^".repeat(caret_len as usize);
                    eprintln!(
                        "{} {}",
                        "     |".blue(),
                        format!("{}{}", indent, carets).red().bold()
                    );
                }
            }
        }

        // Print notes
        for note in &self.notes {
            eprintln!("  {} {}", "=".blue().bold(), note);
        }

        // Print help
        if let Some(help) = &self.help {
            eprintln!("  {} {}", "help:".green().bold(), help);
        }

        eprintln!();
    }
}

/// Accumulates multiple diagnostics
#[derive(Debug, Default)]
pub struct DiagnosticBag {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticBag {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    pub fn add(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.add(Diagnostic::error(message));
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    pub fn emit_all(&self, source_map: &SourceMap) {
        for diagnostic in &self.diagnostics {
            diagnostic.emit(source_map);
        }
    }

    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_creation() {
        let diag = Diagnostic::error("test error")
            .with_note("this is a note")
            .with_help("try this instead");

        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "test error");
        assert_eq!(diag.notes.len(), 1);
        assert!(diag.help.is_some());
    }

    #[test]
    fn test_diagnostic_bag() {
        let mut bag = DiagnosticBag::new();
        assert!(!bag.has_errors());

        bag.error("first error");
        assert!(bag.has_errors());
        assert_eq!(bag.len(), 1);

        bag.error("second error");
        assert_eq!(bag.len(), 2);
    }
}
