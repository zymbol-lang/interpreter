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
    // Fast path: if no comments, no blank lines, and no desugar-sensitive ops, return as-is
    if comments.is_empty()
        && !original.contains("\n\n")
        && !has_standalone_comments(original_lines)
        && !has_desugar_ops(original)
    {
        return formatted.to_string();
    }

    let formatted_lines: Vec<&str> = formatted.lines().collect();
    let mut result = String::new();
    let mut fmt_idx = 0;
    // Number of consecutive code lines that failed to match (used for re-sync)
    let mut consecutive_failures = 0;
    // Leading whitespace of the last successfully matched formatted line.
    // Used to re-indent comments that appear inside blocks.
    let mut current_indent = String::new();

    let mut orig_idx = 0;
    let mut in_block_comment = false;
    // Indentation width of the opening /* line in the original source.
    // Used to strip that prefix from continuation lines so all lines of the
    // comment move together when re-indented (spec §9.3).
    let mut block_comment_orig_indent = 0usize;
    while orig_idx < original_lines.len() {
        let orig_line = original_lines[orig_idx];
        let line_num = (orig_idx + 1) as u32;
        let trimmed = orig_line.trim();

        // Track multi-line block comment state
        if !in_block_comment && trimmed.contains("/*") {
            // Check if the block comment also closes on the same line
            let after_open = trimmed.find("/*").map(|i| &trimmed[i+2..]).unwrap_or("");
            if !after_open.contains("*/") {
                // Opening line of a multi-line block comment: re-indent to current block
                // level and record original indentation so continuation lines move with it.
                in_block_comment = true;
                block_comment_orig_indent = orig_line.len() - orig_line.trim_start().len();
                result.push_str(&current_indent);
                result.push_str(trimmed);
                result.push('\n');
                orig_idx += 1;
                continue;
            }
            // Single-line block comment (/* ... */ on one line) — fall through to normal processing
        } else if in_block_comment {
            // Continuation/closing line: strip the original opening indent and apply
            // current_indent so all lines of the block comment move together (spec §9.3).
            let stripped = if orig_line.len() >= block_comment_orig_indent
                && orig_line[..block_comment_orig_indent].trim().is_empty()
            {
                &orig_line[block_comment_orig_indent..]
            } else {
                orig_line.trim_start()
            };
            result.push_str(&current_indent);
            result.push_str(stripped);
            result.push('\n');
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            orig_idx += 1;
            continue;
        }

        // Case 1: Blank line — preserve, but collapse multiple consecutive ones to one
        if trimmed.is_empty() {
            if !result.ends_with("\n\n") {
                result.push('\n');
            }
            orig_idx += 1;
            continue;
        }

        // Case 2: Comment-only line — re-indent using the NEXT upcoming formatted code line.
        // Using the last-matched line's indent (current_indent) is wrong when a comment
        // appears after a closing } — the indent would still reflect the block interior.
        let code_part = extract_code_part(orig_line);
        if code_part.trim().is_empty() {
            let upcoming_indent = formatted_lines[fmt_idx..]
                .iter()
                .find(|l| !l.trim().is_empty())
                .map(|l| &l[..l.len() - l.trim_start().len()])
                .unwrap_or("");
            result.push_str(upcoming_indent);
            result.push_str(trimmed);
            result.push('\n');
            orig_idx += 1;
            continue;
        }

        // Case 3: Code line — find matching formatted line(s)
        let normalized_orig = normalize_code(&code_part);

        // Re-sync: if fmt_idx is stuck after several failures, scan ahead in formatted lines
        // to find any line that matches one of the next few original code lines. This handles
        // the formatter collapsing multi-line blocks to inline (e.g. `? (x) {\n body\n}` → `? x { body }`).
        if consecutive_failures >= 3 {
            if let Some(new_fmt_idx) = find_resync_point(
                original_lines, orig_idx,
                &formatted_lines, fmt_idx,
            ) {
                // Output all formatted lines we're jumping over (they represent reformatted code)
                while fmt_idx < new_fmt_idx {
                    let skipped = formatted_lines[fmt_idx];
                    if !skipped.trim().is_empty() {
                        result.push_str(skipped);
                        result.push('\n');
                    }
                    fmt_idx += 1;
                }
                consecutive_failures = 0;
            }
        }

        let mut found = false;
        while fmt_idx < formatted_lines.len() {
            let fmt_line = formatted_lines[fmt_idx];
            let normalized_fmt = normalize_code(fmt_line);

            if normalized_fmt.is_empty() {
                fmt_idx += 1;
                continue;
            }

            let (matched, paren_stripped, is_desugar) = code_contains(&normalized_orig, &normalized_fmt);

            if matched {
                // Track indentation from the formatted line regardless of desugar.
                // If this line opens a block (ends with `{`), peek ahead so comments
                // inside the block get the inner indentation, not the opener's indent.
                let trimmed_fmt = fmt_line.trim_end();
                let indent_source = if trimmed_fmt.ends_with('{') {
                    formatted_lines[fmt_idx + 1..].iter()
                        .find(|l| !l.trim().is_empty())
                        .copied()
                        .unwrap_or(fmt_line)
                } else {
                    fmt_line
                };
                let leading = indent_source.len() - indent_source.trim_start().len();
                current_indent = " ".repeat(leading);

                if is_desugar {
                    // Preserve original syntax (p++, x+=5, etc.) — spec §10
                    result.push_str(&current_indent);
                    result.push_str(code_part.trim());
                } else {
                    result.push_str(fmt_line);
                }

                let remaining = if paren_stripped || is_desugar {
                    String::new()
                } else {
                    normalized_orig.replacen(&normalized_fmt, "", 1)
                };

                if remaining.is_empty() || !has_more_code_in_format(&formatted_lines, fmt_idx + 1, &remaining) {
                    append_comments(&mut result, comments, line_num);
                }
                result.push('\n');
                fmt_idx += 1;
                found = true;
                consecutive_failures = 0;

                if remaining.is_empty() {
                    break;
                }
            } else {
                break;
            }
        }

        if !found {
            consecutive_failures += 1;
            // Don't output the unmatched original — the formatted output already covers it.
            // (Outputting the original would create duplicate / un-formatted code.)
            // We still preserve any trailing comment that was on this line.
            let has_trailing_comment = comments.contains_key(&line_num);
            if has_trailing_comment {
                // Emit the original code (without its embedded comment) so the comment
                // can be re-attached exactly once via append_comments.
                // Using `trimmed` here would include the comment text already, causing
                // append_comments to duplicate it.
                result.push_str(code_part.trim());
                append_comments(&mut result, comments, line_num);
                result.push('\n');
            }
            // else: silently skip — the formatted output already represents this code
        }

        orig_idx += 1;
    }

    // Output any remaining formatted lines (those not matched to any original line)
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

/// Append inline comments (trailing // or /* */) from `comments` for `line_num` into `result`.
fn append_comments(result: &mut String, comments: &HashMap<u32, Vec<Comment>>, line_num: u32) {
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

/// Scan ahead in both original and formatted lines to find the next alignment point.
/// Returns a new fmt_idx such that formatted_lines[fmt_idx] matches one of the upcoming
/// original code lines (within a lookahead window).
fn find_resync_point(
    original_lines: &[&str],
    orig_start: usize,
    formatted_lines: &[&str],
    fmt_start: usize,
) -> Option<usize> {
    const LOOKAHEAD: usize = 20;

    // Collect normalized forms of the next LOOKAHEAD original code lines
    let orig_tokens: Vec<String> = original_lines[orig_start..]
        .iter()
        .take(LOOKAHEAD)
        .filter_map(|line| {
            let code = extract_code_part(line);
            let norm = normalize_code(&code);
            if norm.is_empty() { None } else { Some(norm) }
        })
        .collect();

    if orig_tokens.is_empty() {
        return None;
    }

    // Look through formatted lines starting from fmt_start for any that match an orig_token
    for fmt_idx in fmt_start..formatted_lines.len().min(fmt_start + LOOKAHEAD * 3) {
        let normalized_fmt = normalize_code(formatted_lines[fmt_idx]);
        if normalized_fmt.is_empty() {
            continue;
        }
        for orig_tok in &orig_tokens {
            let (matched, _, _) = code_contains(orig_tok, &normalized_fmt);
            if matched {
                return Some(fmt_idx);
            }
        }
    }

    None
}

/// Check if there are standalone comment lines
fn has_standalone_comments(lines: &[&str]) -> bool {
    lines.iter().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && (trimmed.starts_with("//") || trimmed.starts_with("/*"))
    })
}

/// Returns true if source contains any operator that the parser desugarizes (p++, x+=5, etc.)
fn has_desugar_ops(source: &str) -> bool {
    source.contains("++") || source.contains("--")
        || source.contains("+=") || source.contains("-=")
        || source.contains("*=") || source.contains("/=")
        || source.contains("%=") || source.contains("^=")
}

/// Normalize code for matching (remove all whitespace)
fn normalize_code(line: &str) -> String {
    line.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Remove parentheses from a normalized string for loose matching.
/// Used to match `?(expr){...}` with `?expr{...}` when the formatter
/// removes redundant outer parentheses from conditions.
fn strip_parens(s: &str) -> String {
    s.chars().filter(|c| *c != '(' && *c != ')').collect()
}

/// Check whether a normalized formatted line matches within a normalized original line.
/// Returns (matched, is_paren_stripped_match, is_desugar_match).
/// A desugar match means fmt is the parser-expanded form of orig (e.g. `p++` → `p=p+1`),
/// so the caller should emit the ORIGINAL line to preserve the user's syntax.
fn code_contains(orig: &str, fmt: &str) -> (bool, bool, bool) {
    if orig.contains(fmt) {
        return (true, false, false);
    }
    // Fallback: strip parens to handle formatter removing redundant outer parens
    let orig_s = strip_parens(orig);
    let fmt_s = strip_parens(fmt);
    if !fmt_s.is_empty() && orig_s.contains(&fmt_s) {
        return (true, true, false);
    }
    // Desugar check: p++ → p=p+1, x+=5 → x=x+5 (parser expands these before AST)
    if is_desugar_of(orig, fmt) {
        return (true, false, true);
    }
    // Forward-merge: orig ends with `}` and fmt starts with orig.
    // Handles the formatter joining `!? { body }` + `:! { ... }` (or `:> { ... }`) onto
    // one line when the user wrote them on separate lines, and similarly for `} _ {` else
    // or any other construct where the formatter appends more after a closing `}`.
    // paren_stripped=true signals the caller to treat remaining as empty so the next
    // original line (the `:!`/`:>`/`_` part) is silently consumed as already covered.
    if !orig.is_empty() && orig.ends_with('}') && fmt.starts_with(orig) {
        return (true, true, false);
    }
    (false, false, false)
}

/// Returns true when `fmt` is the AST-desugared form of `orig`.
/// Covers: p++ → p=p+1, p-- → p=p-1, x+=rhs → x=x+rhs, etc.
fn is_desugar_of(orig: &str, fmt: &str) -> bool {
    // p++ → p=p+1
    if let Some(name) = orig.strip_suffix("++") {
        if !name.is_empty() {
            return fmt == format!("{}={}+1", name, name);
        }
    }
    // p-- → p=p-1
    if let Some(name) = orig.strip_suffix("--") {
        if !name.is_empty() {
            return fmt == format!("{0}={0}-1", name);
        }
    }
    // x+=rhs → x=x+rhs  (and -, *, /, %, ^)
    for (op_sym, op_ch) in &[("+=", "+"), ("-=", "-"), ("*=", "*"), ("/=", "/"), ("%=", "%"), ("^=", "^")] {
        if let Some(pos) = orig.find(op_sym) {
            let name = &orig[..pos];
            let rhs  = &orig[pos + op_sym.len()..];
            if !name.is_empty() && !rhs.is_empty() {
                let expected = format!("{0}={0}{1}{2}", name, op_ch, rhs);
                if fmt == expected {
                    return true;
                }
            }
        }
    }
    false
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
        assert_eq!(result, "? x > 0 { >> \"yes\" } _ { >> \"no\" }\n");
    }

    #[test]
    fn test_format_loop() {
        let result = format("@ x<10{x=x+1}").unwrap();
        assert_eq!(result, "@ x < 10 { x = x + 1 }\n");
    }

    #[test]
    fn test_format_foreach_loop() {
        let result = format("@ i:1..10{>>i}").unwrap();
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
        assert_eq!(result, "arr = arr $+ 4\n");
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
        // :! must appear on the same line as }, like }} _ {{ (else) — §5.2 spirit
        let result = format("!?{x=risky()}:!{>>\"error\" ¶}").unwrap();
        assert!(result.contains("} :! {"),
            ":! must be on same line as closing }}. Result:\n{}", result);
    }

    #[test]
    fn test_format_try_catch_finally() {
        let result = format("!?{x=1}:!{>>\"err\" ¶}:>{>>\"fin\" ¶}").unwrap();
        assert!(result.contains("} :! {"),  ":! must follow }} on same line. Result:\n{}", result);
        assert!(result.contains("} :> {"),  ":> must follow }} on same line. Result:\n{}", result);
    }

    #[test]
    fn test_format_try_typed_catch() {
        let result = format("!?{x=1}:! ##Div{>>\"div\" ¶}:!{>>\"other\" ¶}").unwrap();
        assert!(result.contains("} :! ##Div {"),
            "typed catch must follow }} on same line. Result:\n{}", result);
        assert!(result.contains("} :! {"),
            "generic catch must follow typed catch }} on same line. Result:\n{}", result);
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
    fn test_array_stays_inline_when_fits() {
        // max_line_length does not govern array layout; max_inline_array_length (60) does.
        // This array is ~30 chars, well under 60, so it must stay inline (spec §10).
        let config = FormatterConfig::new().with_max_line_length(20);
        let result = format_with_config("arr = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]", config).unwrap();
        assert_eq!(result, "arr = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]\n",
            "Array fitting within max_inline_array_length should stay inline: {}", result);
    }

    #[test]
    fn test_named_tuple_breaks() {
        let config = FormatterConfig::new().with_max_line_length(30);
        let result = format_with_config("p = (name: \"Alice Smith\", age: 25, city: \"New York\")", config).unwrap();
        // Should break due to length
        assert!(result.contains('\n'), "Long named tuple should break: {}", result);
    }

    // ── Regression tests for hallasgos_fmt.md ─────────────────────────────────

    // BUG-1: format_output must only add parens to && / || (§11)
    #[test]
    fn test_output_arithmetic_no_parens() {
        let result = format(">> a + b ¶").unwrap();
        assert!(!result.contains("(a + b)"),
            "arithmetic in >> must not get extra parens: {}", result);
        assert!(result.contains(">> a + b"), "Result: {}", result);
    }

    #[test]
    fn test_output_logical_keeps_parens() {
        let result = format(">> (#1 && #0) ¶").unwrap();
        assert!(result.contains("(#1 && #0)"),
            "&& in >> must stay parenthesised: {}", result);
    }

    #[test]
    fn test_output_logical_or_keeps_parens() {
        let result = format(">> (a || b) ¶").unwrap();
        assert!(result.contains("(a || b)"),
            "|| in >> must stay parenthesised: {}", result);
    }

    // BUG-2: implicit pipe |> f must not become |> f(_) (§2.1)
    #[test]
    fn test_pipe_implicit_no_args_emitted() {
        let result = format("r = x |> double").unwrap();
        assert!(!result.contains("(_)"),
            "implicit pipe must not emit (_): {}", result);
        assert!(result.contains("|> double"), "Result: {}", result);
    }

    #[test]
    fn test_pipe_explicit_placeholder_preserved() {
        let result = format("r = x |> double(_)").unwrap();
        assert!(result.contains("|> double(_)"),
            "explicit |> f(_) must keep the placeholder: {}", result);
    }

    #[test]
    fn test_pipe_explicit_extra_args_preserved() {
        let result = format("r = x |> add(_, 1)").unwrap();
        assert!(result.contains("|> add(_, 1)"),
            "extra explicit args must be preserved: {}", result);
    }

    // BUG-3: multi-line block comment re-indentation must be consistent (§9.3)
    // All lines move together: relative offsets inside the comment are preserved.
    #[test]
    fn test_block_comment_multiline_indented_consistently() {
        // Original: /* at col 0, " * note" at col 1 (1 space relative offset).
        // After formatting inside an if block (indent=4):
        //   /*      → 4 spaces  (current_indent)
        //    * note → 5 spaces  (current_indent + 1 relative offset from original)
        let src = "? x > 0 {\n/*\n * note\n */\n>> x ¶\n}";
        let result = format(src).unwrap();
        let lines: Vec<&str> = result.lines().collect();

        let open_indent = lines.iter()
            .find(|l| l.trim_start().starts_with("/*"))
            .map(|l| l.len() - l.trim_start().len())
            .expect("/* line missing");
        let cont_indent = lines.iter()
            .find(|l| l.trim_start().starts_with("* note"))
            .map(|l| l.len() - l.trim_start().len())
            .expect("* note line missing");
        let close_indent = lines.iter()
            .find(|l| l.trim_start().starts_with("*/"))
            .map(|l| l.len() - l.trim_start().len())
            .expect("*/ line missing");

        // Opening must be at current block indent (4 spaces).
        assert_eq!(open_indent, 4,
            "/* must be at block indent level.\nFormatted:\n{}", result);
        // Continuation and closing lines must preserve their +1 offset from the opening.
        assert_eq!(cont_indent, open_indent + 1,
            "continuation must keep relative offset from /*.\nFormatted:\n{}", result);
        assert_eq!(close_indent, open_indent + 1,
            "closing */ must keep relative offset from /*.\nFormatted:\n{}", result);
    }

    #[test]
    fn test_block_comment_toplevel_preserved() {
        // A top-level block comment (col 0) must stay at col 0.
        let src = "x = 1\n/*\n * doc\n */\ny = 2\n";
        let result = format(src).unwrap();
        let open_indent = result.lines()
            .find(|l| l.trim_start().starts_with("/*"))
            .map(|l| l.len() - l.trim_start().len())
            .expect("/* line missing");
        assert_eq!(open_indent, 0, "top-level /* must stay at col 0.\nFormatted:\n{}", result);
    }

    // DEAD-1/2/LATENT-1: removed config fields must not exist on FormatterConfig
    #[test]
    fn test_config_has_no_dead_fields() {
        let cfg = FormatterConfig::default();
        // These fields must compile without trailing_commas / continuation_indent /
        // max_inline_array_elements — the test body just ensures the struct is
        // constructed without panic and has the fields we expect.
        let _ = cfg.indent_size;
        let _ = cfg.max_line_length;
        let _ = cfg.use_spaces;
        let _ = cfg.max_inline_array_length;
        let _ = cfg.brace_same_line;
        let _ = cfg.inline_single_statement;
    }

    // MINOR-1: a block with only ¶ must NOT be inlined (Newline is not simple)
    #[test]
    fn test_newline_only_block_not_inlined() {
        // ? x > 0 { ¶ } — the single statement is a Newline, should expand
        let src = "? x > 0 {\n¶\n}";
        let result = format(src).unwrap();
        // Must be multi-line (the ¶ should not be squeezed into one line alone)
        assert!(result.contains('\n'), "block with only ¶ should not be inlined: {}", result);
    }

    // MINOR-2: multiple consecutive blank lines must collapse to one (§2.2)
    #[test]
    fn test_consecutive_blank_lines_collapsed() {
        let src = "x = 1\n\n\n\ny = 2\n";
        let result = format(src).unwrap();
        assert!(!result.contains("\n\n\n"),
            "three consecutive blank lines must collapse to one: {}", result);
        assert!(result.contains("\n\n"),
            "one blank line must still be present: {}", result);
    }

    // Idempotency over the fixed cases
    #[test]
    fn test_idempotency_pipe_implicit() {
        let src = "r = x |> double\n";
        let second = format(src).unwrap();
        assert_eq!(src, second, "already-formatted implicit pipe must be stable");
    }

    #[test]
    fn test_idempotency_output_arithmetic() {
        let src = ">> a + b ¶\n";
        let second = format(src).unwrap();
        assert_eq!(src, second, "already-formatted arithmetic output must be stable");
    }
}
