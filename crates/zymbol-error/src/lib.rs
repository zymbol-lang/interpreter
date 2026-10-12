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

impl Severity {
    /// The word without colour.
    pub fn plain(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
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
        eprint!("{}", self.render(source_map, true));
    }

    /// The block `emit` prints, as text: the headline, where, the source line
    /// with its carets, the notes, the help, and a blank line.
    ///
    /// Without colour it is what a subscript's failure carries (GLB-017 I,
    /// decided 2026-09-26): the failure written as `zymbol run` writes it,
    /// and not the terminal's escape codes inside `_err`.
    pub fn render(&self, source_map: &SourceMap, color: bool) -> String {
        use std::fmt::Write as _;
        let paint = |text: &str, f: fn(&str) -> String| if color { f(text) } else { text.to_string() };
        let blue_bold = |t: &str| t.blue().bold().to_string();
        let blue = |t: &str| t.blue().to_string();
        let red_bold = |t: &str| t.red().bold().to_string();
        let green_bold = |t: &str| t.green().bold().to_string();
        let severity = if color { self.severity.to_string() } else { self.severity.plain().to_string() };

        let mut out = String::new();
        let _ = writeln!(out, "{}: {}", severity, self.message);

        if let Some(span) = &self.span {
            if let Some(file) = source_map.get(span.file_id) {
                let line_num = span.start.line;
                let _ = writeln!(
                    out,
                    "  {} {}:{}:{}",
                    paint("-->", blue_bold),
                    file.name,
                    line_num,
                    span.start.column
                );
                if let Some(line) = file.line(line_num) {
                    let line_str = format!("{:4}", line_num);
                    let _ = writeln!(out, "{} {}", paint(&line_str, blue_bold), paint("|", blue));
                    let _ = writeln!(out, "{} {} {}", paint(&line_str, blue_bold), paint("|", blue), line);
                    // Column is 1-indexed, so subtract 1 for correct positioning
                    let indent = " ".repeat((span.start.column - 1) as usize);
                    let caret_len = span.end.column.saturating_sub(span.start.column).max(1);
                    let carets = "^".repeat(caret_len as usize);
                    let _ = writeln!(
                        out,
                        "{} {}",
                        paint("     |", blue),
                        paint(&format!("{}{}", indent, carets), red_bold)
                    );
                }
            }
        }

        for note in &self.notes {
            let _ = writeln!(out, "  {} {}", paint("=", blue_bold), note);
        }

        // `= help:`, not a bare `help:`. rustc reserves the bare form for a
        // SPANNED suggestion that brings its own snippet, and uses `= help:`
        // for a trailing line of guidance — which is the only kind Zymbol has.
        // Every other place that prints a diagnostic block already spelled it
        // that way (twelve of them in zymbol-cli); this printer was the one
        // that did not, so the same warning read differently depending on
        // which path emitted it.
        //
        // It is also load-bearing for the corpus: `zyq`'s `strip_warnings`
        // drops a diagnostic block by its `  =` lines, so a help line without
        // the `=` leaks into the compared output of 75 goldens.
        if let Some(help) = &self.help {
            let _ = writeln!(out, "  {} {}", paint("= help:", green_bold), help);
        }

        out.push('\n');
        out
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

    pub fn iter(&self) -> std::slice::Iter<'_, Diagnostic> {
        self.diagnostics.iter()
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
