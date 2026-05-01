//! Formatter configuration for Zymbol-Lang
//!
//! Defines formatting options like indentation, line length, and spacing preferences.

/// Configuration options for the Zymbol code formatter
#[derive(Debug, Clone)]
pub struct FormatterConfig {
    /// Number of spaces for each indentation level (default: 4)
    pub indent_size: usize,

    /// Maximum line length before wrapping (default: 100)
    pub max_line_length: usize,

    /// Whether to use spaces (true) or tabs (false) for indentation (default: true)
    pub use_spaces: bool,

    /// Maximum length for an inline array before wrapping (default: 60)
    pub max_inline_array_length: usize,

    /// Whether to put opening brace on same line (default: true)
    pub brace_same_line: bool,

    /// Whether to format single-statement blocks on one line (default: true)
    pub inline_single_statement: bool,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            indent_size: 4,
            max_line_length: 100,
            use_spaces: true,
            max_inline_array_length: 60,
            brace_same_line: true,
            inline_single_statement: true,
        }
    }
}

impl FormatterConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method to set indent size
    pub fn with_indent_size(mut self, size: usize) -> Self {
        self.indent_size = size;
        self
    }

    /// Builder method to set max line length
    pub fn with_max_line_length(mut self, length: usize) -> Self {
        self.max_line_length = length;
        self
    }

    /// Builder method to use tabs instead of spaces
    pub fn with_tabs(mut self) -> Self {
        self.use_spaces = false;
        self
    }

    /// Builder method to put braces on new line
    pub fn with_brace_new_line(mut self) -> Self {
        self.brace_same_line = false;
        self
    }

    /// Builder method to disable inline single statements
    pub fn without_inline_single_statement(mut self) -> Self {
        self.inline_single_statement = false;
        self
    }

    /// Get the indentation string for a given level
    pub fn indent_string(&self, level: usize) -> String {
        if self.use_spaces {
            " ".repeat(self.indent_size * level)
        } else {
            "\t".repeat(level)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FormatterConfig::default();
        assert_eq!(config.indent_size, 4);
        assert_eq!(config.max_line_length, 100);
        assert!(config.use_spaces);
    }

    #[test]
    fn test_builder_methods() {
        let config = FormatterConfig::new()
            .with_indent_size(2)
            .with_max_line_length(80)
            .with_tabs();

        assert_eq!(config.indent_size, 2);
        assert_eq!(config.max_line_length, 80);
        assert!(!config.use_spaces);
    }

    #[test]
    fn test_indent_string_spaces() {
        let config = FormatterConfig::new().with_indent_size(4);
        assert_eq!(config.indent_string(0), "");
        assert_eq!(config.indent_string(1), "    ");
        assert_eq!(config.indent_string(2), "        ");
    }

    #[test]
    fn test_indent_string_tabs() {
        let config = FormatterConfig::new().with_tabs();
        assert_eq!(config.indent_string(0), "");
        assert_eq!(config.indent_string(1), "\t");
        assert_eq!(config.indent_string(2), "\t\t");
    }
}
