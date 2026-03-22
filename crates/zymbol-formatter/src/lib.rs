//! Zymbol-Lang Code Formatter
//!
//! A code formatter for Zymbol-Lang that produces consistent, readable output
//! following the language's symbolic conventions.
//!
//! # Example
//!
//! ```ignore
//! use zymbol_formatter::{format, format_with_config, FormatterConfig};
//!
//! // Format with defaults
//! let source = "x=5\n>> x";
//! let formatted = format(source)?;
//!
//! // Format with custom config
//! let config = FormatterConfig::new()
//!     .with_indent_size(2)
//!     .with_max_line_length(80);
//! let formatted = format_with_config(source, config)?;
//! ```
//!
//! # Formatting Rules
//!
//! ## Spacing Around Operators
//!
//! | Operator | Spacing | Example |
//! |----------|---------|---------|
//! | `=`, `:=` | spaces | `x = 5`, `PI := 3.14` |
//! | `+`, `-`, `*`, `/`, `%` | spaces | `a + b * c` |
//! | `..` | no spaces | `1..10` |
//! | `->` | spaces | `x -> x * 2` |
//! | `$#`, `$+`, etc. | no space before | `arr$#` |
//! | `::` | no spaces | `module::func()` |
//! | `.` | no spaces | `tuple.field` |
//!
//! ## Blocks
//!
//! Single-statement blocks can be formatted inline:
//! ```zymbol
//! ? x > 0 { >> "yes" }
//! ```
//!
//! Multi-statement blocks use multiple lines:
//! ```zymbol
//! ? x > 0 {
//!     >> "positive"
//!     x = x + 1
//! }
//! ```
//!
//! # Comment Preservation
//!
//! Comments are preserved during formatting. Line comments (//) are kept at the
//! end of the line they appear on. Block comments (/* */) are preserved inline.

mod config;
mod output;
mod visitor;

pub use config::FormatterConfig;

use std::collections::HashMap;
use thiserror::Error;
use zymbol_lexer::{Lexer, Token, TokenKind};
use zymbol_parser::Parser;
use zymbol_span::FileId;

use output::OutputBuilder;
use visitor::FormatVisitor;

/// Error type for formatting operations
#[derive(Error, Debug)]
pub enum FormatError {
    /// Lexer errors occurred during tokenization
    #[error("lexer errors: {0}")]
    LexerError(String),

    /// Parser errors occurred during parsing
    #[error("parser errors: {0}")]
    ParserError(String),
}

/// Comment extracted from source, with its position
#[derive(Debug, Clone)]
struct Comment {
    content: String,
    is_block: bool,
    #[allow(dead_code)]
    line: u32,
}

/// Extract comments from token stream, organized by line number
fn extract_comments(tokens: &[Token]) -> HashMap<u32, Vec<Comment>> {
    let mut comments: HashMap<u32, Vec<Comment>> = HashMap::new();

    for token in tokens {
        match &token.kind {
            TokenKind::LineComment(content) => {
                let comment = Comment {
                    content: content.clone(),
                    is_block: false,
                    line: token.span.start.line,
                };
                comments.entry(token.span.start.line).or_default().push(comment);
            }
            TokenKind::BlockComment(content) => {
                let comment = Comment {
                    content: content.clone(),
                    is_block: true,
                    line: token.span.start.line,
                };
                comments.entry(token.span.start.line).or_default().push(comment);
            }
            _ => {}
        }
    }

    comments
}

/// Build a map of original source lines for reference
fn build_line_map(source: &str) -> Vec<&str> {
    source.lines().collect()
}

/// Format Zymbol source code with default configuration
///
/// # Arguments
///
/// * `source` - The source code to format
///
/// # Returns
///
/// The formatted source code, or an error if parsing failed.
///
/// # Example
///
/// ```ignore
/// let formatted = zymbol_formatter::format("x=5\n>>x")?;
/// assert_eq!(formatted, "x = 5\n>> x\n");
/// ```
pub fn format(source: &str) -> Result<String, FormatError> {
    format_with_config(source, FormatterConfig::default())
}

/// Format Zymbol source code with custom configuration
///
/// # Arguments
///
/// * `source` - The source code to format
/// * `config` - The formatter configuration to use
///
/// # Returns
///
/// The formatted source code, or an error if parsing failed.
///
/// # Example
///
/// ```ignore
/// let config = FormatterConfig::new().with_indent_size(2);
/// let formatted = zymbol_formatter::format_with_config("x=5", config)?;
/// ```
pub fn format_with_config(source: &str, config: FormatterConfig) -> Result<String, FormatError> {
    // Lex the source
    let lexer = Lexer::new(source, FileId(0));
    let (tokens, lex_errors) = lexer.tokenize();

    if !lex_errors.is_empty() {
        let error_msgs: Vec<String> = lex_errors.iter().map(|e| e.message.clone()).collect();
        return Err(FormatError::LexerError(error_msgs.join("; ")));
    }

    // Extract comments from token stream
    let comments = extract_comments(&tokens);
    let original_lines = build_line_map(source);

    // Parse the tokens (parser skips comment tokens)
    let parser = Parser::new(tokens);
    let program = parser.parse().map_err(|errors| {
        let error_msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        FormatError::ParserError(error_msgs.join("; "))
    })?;

    // Format the AST
    let mut output = OutputBuilder::new(config);
    let mut visitor = FormatVisitor::new(&mut output);
    visitor.format_program(&program);

    let formatted = output.finish();

    // Now merge comments back into formatted output
    let result = merge_comments(source, &formatted, &comments, &original_lines);

    Ok(result)
}

/// Merge comments and blank lines from original source into formatted output
fn merge_comments(
    original: &str,
    formatted: &str,
    comments: &HashMap<u32, Vec<Comment>>,
    original_lines: &[&str],
) -> String {
    // If no comments and no blank lines to preserve, return formatted as-is
    if comments.is_empty() && !original.contains("\n\n") && !has_standalone_comments(original_lines) {
        return formatted.to_string();
    }

    // Strategy: Insert blank lines and standalone comments from original
    // into the formatted output at appropriate positions.
    //
    // 1. Find "anchor points" - lines with code that we can match
    // 2. Insert blank lines and comments between anchors

    let formatted_lines: Vec<&str> = formatted.lines().collect();
    let mut result = String::new();
    let mut fmt_idx = 0;

    // For each original line, determine what to do
    for (orig_idx, orig_line) in original_lines.iter().enumerate() {
        let line_num = (orig_idx + 1) as u32;
        let trimmed = orig_line.trim();

        // Case 1: Blank line - preserve it
        if trimmed.is_empty() {
            result.push('\n');
            continue;
        }

        // Case 2: Comment-only line - preserve as-is
        let code_part = extract_code_part(orig_line);
        if code_part.trim().is_empty() {
            result.push_str(trimmed);
            result.push('\n');
            continue;
        }

        // Case 3: Code line - find matching formatted line(s)
        let normalized_orig = normalize_code(&code_part);

        // Look for this code in the remaining formatted lines
        let mut found = false;
        while fmt_idx < formatted_lines.len() {
            let fmt_line = formatted_lines[fmt_idx];
            let normalized_fmt = normalize_code(fmt_line);

            // Check if this formatted line is part of the original code
            if !normalized_fmt.is_empty() && normalized_orig.contains(&normalized_fmt) {
                // Output this formatted line
                result.push_str(fmt_line);

                // If this is the last part of the original line, add trailing comment
                let remaining = normalized_orig.replacen(&normalized_fmt, "", 1);
                if remaining.is_empty() || !has_more_code_in_format(&formatted_lines, fmt_idx + 1, &remaining) {
                    if let Some(line_comments) = comments.get(&line_num) {
                        for comment in line_comments {
                            if comment.is_block {
                                result.push_str(" /*");
                                result.push_str(&comment.content);
                                result.push_str("*/");
                            } else {
                                result.push_str(" //");
                                result.push_str(&comment.content);
                            }
                        }
                    }
                }
                result.push('\n');
                fmt_idx += 1;
                found = true;

                // Check if we've output all the code from this original line
                if remaining.is_empty() {
                    break;
                }
            } else if normalized_fmt.is_empty() {
                // Empty formatted line, skip
                fmt_idx += 1;
            } else {
                // Doesn't match, maybe multi-line expansion - keep looking
                break;
            }
        }

        if !found {
            // Couldn't find match - output original with comment
            result.push_str(trimmed);
            if let Some(line_comments) = comments.get(&line_num) {
                for comment in line_comments {
                    if comment.is_block {
                        result.push_str(" /*");
                        result.push_str(&comment.content);
                        result.push_str("*/");
                    } else {
                        result.push_str(" //");
                        result.push_str(&comment.content);
                    }
                }
            }
            result.push('\n');
        }
    }

    // Output any remaining formatted lines
    while fmt_idx < formatted_lines.len() {
        let fmt_line = formatted_lines[fmt_idx];
        if !fmt_line.trim().is_empty() {
            result.push_str(fmt_line);
            result.push('\n');
        }
        fmt_idx += 1;
    }

    // Ensure single trailing newline
    while result.ends_with("\n\n") {
        result.pop();
    }
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }

    result
}

/// Check if there are standalone comment lines
fn has_standalone_comments(lines: &[&str]) -> bool {
    lines.iter().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && (trimmed.starts_with("//") || trimmed.starts_with("/*"))
    })
}

/// Normalize code for matching (remove all whitespace)
fn normalize_code(line: &str) -> String {
    line.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Check if remaining code exists in subsequent formatted lines
fn has_more_code_in_format(lines: &[&str], start_idx: usize, remaining: &str) -> bool {
    for line in &lines[start_idx..] {
        let normalized = normalize_code(line);
        if remaining.starts_with(&normalized) {
            return true;
        }
    }
    false
}

/// Extract the code part of a line (everything before comments)
fn extract_code_part(line: &str) -> String {
    // Extract code by removing both line comments (//) and block comments (/* */)
    let mut in_string = false;
    let mut string_char = '"';
    let chars: Vec<char> = line.chars().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if !in_string && (c == '"' || c == '\'') {
            in_string = true;
            string_char = c;
            result.push(c);
            i += 1;
            continue;
        }

        if in_string {
            result.push(c);
            if c == '\\' && i + 1 < chars.len() {
                i += 1;
                result.push(chars[i]);
                i += 1;
                continue;
            }
            if c == string_char {
                in_string = false;
            }
            i += 1;
            continue;
        }

        // Check for line comment //
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            // Rest of line is comment, stop here
            break;
        }

        // Check for block comment /*
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            // Skip until */
            i += 2;
            while i + 1 < chars.len() {
                if chars[i] == '*' && chars[i + 1] == '/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }

        result.push(c);
        i += 1;
    }

    result
}

/// Check if source code is already formatted according to the configuration
///
/// Returns `true` if reformatting would produce identical output.
///
/// # Example
///
/// ```ignore
/// let is_formatted = zymbol_formatter::is_formatted("x = 5\n")?;
/// ```
pub fn is_formatted(source: &str) -> Result<bool, FormatError> {
    is_formatted_with_config(source, FormatterConfig::default())
}

/// Check if source code is already formatted according to custom configuration
pub fn is_formatted_with_config(source: &str, config: FormatterConfig) -> Result<bool, FormatError> {
    let formatted = format_with_config(source, config)?;
    Ok(formatted == source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_simple_assignment() {
        let result = format("x=5").unwrap();
        assert_eq!(result, "x = 5\n");
    }

    #[test]
    fn test_format_with_line_comment() {
        let result = format("x=5 // comment").unwrap();
        assert!(result.contains("// comment"), "Result: {}", result);
        assert!(result.contains("x = 5"), "Result: {}", result);
    }

    #[test]
    fn test_format_with_block_comment() {
        let result = format("x=5 /* comment */").unwrap();
        assert!(result.contains("/* comment */"), "Result: {}", result);
    }

    #[test]
    fn test_format_const_decl() {
        let result = format("PI:=3.14").unwrap();
        assert_eq!(result, "PI := 3.14\n");
    }

    #[test]
    fn test_format_output_statement() {
        let result = format(">>\"Hello\"").unwrap();
        assert_eq!(result, ">> \"Hello\"\n");
    }

    #[test]
    fn test_format_binary_expression() {
        let result = format("x=5+3*2").unwrap();
        assert_eq!(result, "x = 5 + 3 * 2\n");
    }

    #[test]
    fn test_format_range_no_spaces() {
        let result = format("x=1..10").unwrap();
        assert_eq!(result, "x = 1..10\n");
    }

    #[test]
    fn test_format_boolean_literals() {
        let result = format("x=#1\ny=#0").unwrap();
        assert_eq!(result, "x = #1\ny = #0\n");
    }

    #[test]
    fn test_format_if_statement_inline() {
        let result = format("?x>0{>>\"yes\"}").unwrap();
        assert_eq!(result, "? x > 0 { >> \"yes\" }\n");
    }

    #[test]
    fn test_format_if_else() {
        let result = format("?x>0{>>\"yes\"}_{>>\"no\"}").unwrap();
        assert_eq!(result, "? x > 0 { >> \"yes\" }\n_{ >> \"no\" }\n");
    }

    #[test]
    fn test_format_loop() {
        let result = format("@x<10{x=x+1}").unwrap();
        assert_eq!(result, "@ x < 10 { x = x + 1 }\n");
    }

    #[test]
    fn test_format_foreach_loop() {
        let result = format("@i:1..10{>>i}").unwrap();
        assert_eq!(result, "@ i:1..10 { >> i }\n");
    }

    #[test]
    fn test_format_function_decl() {
        let result = format("add(a,b){<~a+b}").unwrap();
        assert_eq!(result, "add(a, b) { <~ a + b }\n");
    }

    #[test]
    fn test_format_lambda() {
        let result = format("f=x->x*2").unwrap();
        assert_eq!(result, "f = x -> x * 2\n");
    }

    #[test]
    fn test_format_array_literal_inline() {
        let result = format("arr=[1,2,3]").unwrap();
        assert_eq!(result, "arr = [1, 2, 3]\n");
    }

    #[test]
    fn test_format_tuple() {
        let result = format("t=(1,2,3)").unwrap();
        assert_eq!(result, "t = (1, 2, 3)\n");
    }

    #[test]
    fn test_format_named_tuple() {
        let result = format("p=(name:\"Alice\",age:25)").unwrap();
        assert_eq!(result, "p = (name: \"Alice\", age: 25)\n");
    }

    #[test]
    fn test_format_collection_length() {
        let result = format("len=arr$#").unwrap();
        assert_eq!(result, "len = arr$#\n");
    }

    #[test]
    fn test_format_collection_append() {
        let result = format("arr=arr$+4").unwrap();
        assert_eq!(result, "arr = arr$+ 4\n");
    }

    #[test]
    fn test_format_member_access() {
        let result = format("x=obj.field").unwrap();
        assert_eq!(result, "x = obj.field\n");
    }

    #[test]
    fn test_format_function_call() {
        let result = format("print(\"hello\")").unwrap();
        assert_eq!(result, "print(\"hello\")\n");
    }

    #[test]
    fn test_format_match() {
        let result = format("r=??x{1:\"one\"\n2:\"two\"\n_:\"other\"}").unwrap();
        assert!(result.contains("?? x"));
        assert!(result.contains("1"));
        assert!(result.contains("2"));
        assert!(result.contains("_"));
    }

    #[test]
    fn test_format_try_catch() {
        let result = format("!?{x=risky()}:!{>>\"error\"}").unwrap();
        assert!(result.contains("!?"));
        assert!(result.contains(":!"));
    }

    #[test]
    fn test_format_error_check() {
        let result = format("?x$!{>>\"error\"}").unwrap();
        assert!(result.contains("$!"));
    }

    #[test]
    fn test_format_string_escape() {
        let result = format("x=\"hello\\nworld\"").unwrap();
        assert_eq!(result, "x = \"hello\\nworld\"\n");
    }

    #[test]
    fn test_format_char_literal() {
        let result = format("c='A'").unwrap();
        assert_eq!(result, "c = 'A'\n");
    }

    #[test]
    fn test_format_unary_expression() {
        let result = format("x=-5").unwrap();
        assert_eq!(result, "x = -5\n");
    }

    #[test]
    fn test_format_not_expression() {
        let result = format("x=!flag").unwrap();
        assert_eq!(result, "x = !flag\n");
    }

    #[test]
    fn test_custom_config_indent() {
        let config = FormatterConfig::new().with_indent_size(2);
        let result = format_with_config("?x>0{>>\"a\"\n>>\"b\"}", config).unwrap();
        // With 2-space indent, indented lines should have 2 spaces
        assert!(result.contains("  >>"));
    }

    #[test]
    fn test_is_formatted_true() {
        let source = "x = 5\n";
        assert!(is_formatted(source).unwrap());
    }

    #[test]
    fn test_is_formatted_false() {
        let source = "x=5";
        assert!(!is_formatted(source).unwrap());
    }

    #[test]
    fn test_format_twice_is_idempotent() {
        let source = "x=5+3*2\n?x>0{>>x}";
        let first = format(source).unwrap();
        let second = format(&first).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn test_lexer_error() {
        // This should fail lexing (unclosed string)
        let result = format("x=\"unclosed");
        assert!(result.is_err());
        match result {
            Err(FormatError::LexerError(_)) => (),
            _ => panic!("Expected lexer error"),
        }
    }

    #[test]
    fn test_parser_error() {
        // This should fail parsing
        let result = format("? { }");  // Missing condition
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_code_part() {
        assert_eq!(extract_code_part("x = 5 // comment"), "x = 5 ");
        assert_eq!(extract_code_part("x = 5"), "x = 5");
        assert_eq!(extract_code_part("// only comment"), "");
        assert_eq!(extract_code_part("x = \"//not a comment\""), "x = \"//not a comment\"");
    }

    #[test]
    fn test_long_function_call_breaks() {
        // With a very short max line length, function calls should break
        let config = FormatterConfig::new().with_max_line_length(30);
        let result = format_with_config("func(\"very long argument one\", \"very long argument two\", \"three\")", config).unwrap();
        // Should have multiple lines
        assert!(result.contains('\n'), "Long function call should break: {}", result);
    }

    #[test]
    fn test_short_function_call_inline() {
        // With default line length, short calls stay inline
        let result = format("func(a, b, c)").unwrap();
        assert_eq!(result, "func(a, b, c)\n");
    }

    #[test]
    fn test_long_array_breaks() {
        let config = FormatterConfig::new().with_max_line_length(20);
        let result = format_with_config("arr = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]", config).unwrap();
        // Should have multiple lines due to max_inline_array_elements
        assert!(result.lines().count() > 1, "Long array should break: {}", result);
    }

    #[test]
    fn test_named_tuple_breaks() {
        let config = FormatterConfig::new().with_max_line_length(30);
        let result = format_with_config("p = (name: \"Alice Smith\", age: 25, city: \"New York\")", config).unwrap();
        // Should break due to length
        assert!(result.contains('\n'), "Long named tuple should break: {}", result);
    }
}
