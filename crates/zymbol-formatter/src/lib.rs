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
//! Comments are re-emitted by source position (span interleaving, see the
//! `comments` module): trailing `//` comments stay on their line, standalone
//! comments keep their own line at the surrounding indent, and block comments
//! move as a unit. A safety gate (see the `gate` module) verifies that the
//! output is token-equivalent to the source — including the comment count —
//! and returns an error instead of ever emitting corrupted output.

mod comments;
mod config;
mod gate;
mod output;
mod visitor;

pub use config::FormatterConfig;

use thiserror::Error;
use zymbol_lexer::Lexer;
use zymbol_parser::Parser;
use zymbol_span::FileId;

use comments::CommentStream;
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

    /// The formatted output would not be token/shape equivalent to the
    /// source. The input file is left unchanged. See `gate` module.
    #[error("safety gate: {0}")]
    SafetyGate(String),
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

    // Collect comments (span-ordered) before the parser consumes the tokens
    let comments = CommentStream::from_tokens(&tokens);

    // Parse the tokens (parser skips comment tokens)
    let parser = Parser::new(tokens);
    let program = parser.parse().map_err(|errors| {
        let error_msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        FormatError::ParserError(error_msgs.join("; "))
    })?;

    // Format the AST, interleaving comments by source position
    let mut output = OutputBuilder::new(config);
    let mut visitor = FormatVisitor::new(&mut output, comments);
    visitor.format_program(&program);

    let result = output.finish();

    // Safety gate: never return output that is not equivalent to the source.
    gate::verify(source, &program, &result).map_err(FormatError::SafetyGate)?;

    Ok(result)
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
        let result = format("p=#(name:\"Alice\",age:25)").unwrap();
        assert_eq!(result, "p = #(name: \"Alice\", age: 25)\n");
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
        let result = format("r=??x{1=>\"one\"\n2=>\"two\"\n_=>\"other\"}").unwrap();
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
        let result = format_with_config("p = #(name: \"Alice Smith\", age: 25, city: \"New York\")", config).unwrap();
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
