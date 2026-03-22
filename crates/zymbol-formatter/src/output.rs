//! Output builder for Zymbol-Lang formatter
//!
//! Manages formatted code construction with proper indentation and line handling.

#![allow(dead_code)] // Helper methods for future use

use crate::config::FormatterConfig;

/// Builder for constructing formatted output
#[derive(Debug)]
pub struct OutputBuilder {
    /// The formatted output being built
    buffer: String,

    /// Current indentation level
    indent_level: usize,

    /// Current column position (0-indexed)
    current_column: usize,

    /// Configuration for formatting
    config: FormatterConfig,

    /// Whether we're at the start of a new line
    at_line_start: bool,
}

impl OutputBuilder {
    /// Create a new output builder with the given configuration
    pub fn new(config: FormatterConfig) -> Self {
        Self {
            buffer: String::new(),
            indent_level: 0,
            current_column: 0,
            config,
            at_line_start: true,
        }
    }

    /// Get the current configuration
    pub fn config(&self) -> &FormatterConfig {
        &self.config
    }

    /// Increase indentation level
    pub fn indent(&mut self) {
        self.indent_level += 1;
    }

    /// Decrease indentation level
    pub fn dedent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }

    /// Get current indentation level
    pub fn indent_level(&self) -> usize {
        self.indent_level
    }

    /// Write a string to the output
    pub fn write(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }

        // Handle indentation if at line start
        if self.at_line_start && !s.starts_with('\n') {
            let indent = self.config.indent_string(self.indent_level);
            self.buffer.push_str(&indent);
            self.current_column = indent.len();
            self.at_line_start = false;
        }

        // Track column position
        for ch in s.chars() {
            if ch == '\n' {
                self.current_column = 0;
                self.at_line_start = true;
            } else {
                self.current_column += 1;
                self.at_line_start = false;
            }
        }

        self.buffer.push_str(s);
    }

    /// Write a newline
    pub fn newline(&mut self) {
        self.buffer.push('\n');
        self.current_column = 0;
        self.at_line_start = true;
    }

    /// Remove trailing newline if at line start (for joining with previous line)
    pub fn backspace_newline(&mut self) {
        if self.at_line_start && self.buffer.ends_with('\n') {
            self.buffer.pop();
            self.at_line_start = false;
            // Recalculate current_column from last line
            if let Some(last_newline) = self.buffer.rfind('\n') {
                self.current_column = self.buffer.len() - last_newline - 1;
            } else {
                self.current_column = self.buffer.len();
            }
        }
    }

    /// Write a space
    pub fn space(&mut self) {
        self.write(" ");
    }

    /// Write a string followed by a space
    pub fn write_spaced(&mut self, s: &str) {
        self.write(s);
        self.space();
    }

    /// Write a space followed by a string
    pub fn space_write(&mut self, s: &str) {
        self.space();
        self.write(s);
    }

    /// Write with spaces on both sides
    pub fn write_padded(&mut self, s: &str) {
        self.space();
        self.write(s);
        self.space();
    }

    /// Write an opening brace, respecting brace_same_line config
    pub fn open_brace(&mut self) {
        if self.config.brace_same_line {
            self.write(" {");
        } else {
            self.newline();
            self.write("{");
        }
    }

    /// Write a closing brace on its own line
    pub fn close_brace(&mut self) {
        self.write("}");
    }

    /// Check if current line would exceed max length if we added more content
    pub fn would_exceed_line_length(&self, additional: usize) -> bool {
        self.current_column + additional > self.config.max_line_length
    }

    /// Get current column position
    pub fn current_column(&self) -> usize {
        self.current_column
    }

    /// Check if we're at the start of a line
    pub fn is_at_line_start(&self) -> bool {
        self.at_line_start
    }

    /// Consume the builder and return the formatted output
    pub fn finish(mut self) -> String {
        // Ensure output ends with exactly one newline
        while self.buffer.ends_with("\n\n") {
            self.buffer.pop();
        }
        if !self.buffer.is_empty() && !self.buffer.ends_with('\n') {
            self.buffer.push('\n');
        }
        self.buffer
    }

    /// Get the current output (for inspection without consuming)
    pub fn as_str(&self) -> &str {
        &self.buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_write() {
        let config = FormatterConfig::new();
        let mut builder = OutputBuilder::new(config);
        builder.write("hello");
        builder.write(" world");
        assert_eq!(builder.finish(), "hello world\n");
    }

    #[test]
    fn test_indentation() {
        let config = FormatterConfig::new().with_indent_size(4);
        let mut builder = OutputBuilder::new(config);
        builder.write("line1");
        builder.newline();
        builder.indent();
        builder.write("line2");
        builder.newline();
        builder.dedent();
        builder.write("line3");
        assert_eq!(builder.finish(), "line1\n    line2\nline3\n");
    }

    #[test]
    fn test_nested_indentation() {
        let config = FormatterConfig::new().with_indent_size(2);
        let mut builder = OutputBuilder::new(config);
        builder.write("a");
        builder.newline();
        builder.indent();
        builder.write("b");
        builder.newline();
        builder.indent();
        builder.write("c");
        builder.newline();
        builder.dedent();
        builder.dedent();
        builder.write("d");
        assert_eq!(builder.finish(), "a\n  b\n    c\nd\n");
    }

    #[test]
    fn test_open_brace_same_line() {
        let config = FormatterConfig::new();
        let mut builder = OutputBuilder::new(config);
        builder.write("?");
        builder.open_brace();
        assert_eq!(builder.as_str(), "? {");
    }

    #[test]
    fn test_open_brace_new_line() {
        let config = FormatterConfig::new().with_brace_new_line();
        let mut builder = OutputBuilder::new(config);
        builder.write("?");
        builder.open_brace();
        assert_eq!(builder.as_str(), "?\n{");
    }

    #[test]
    fn test_spaced_methods() {
        let config = FormatterConfig::new();
        let mut builder = OutputBuilder::new(config);
        builder.write("a");
        builder.write_padded("+");
        builder.write("b");
        assert_eq!(builder.finish(), "a + b\n");
    }
}
