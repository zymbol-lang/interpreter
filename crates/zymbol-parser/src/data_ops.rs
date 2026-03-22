//! Data operation parsing for Zymbol-Lang
//!
//! Handles parsing of data transformation and introspection expressions:
//! - Numeric evaluation: #|expr| (safe string-to-number conversion)
//! - Type metadata: expr#? (returns type info tuple)
//! - Format expressions: e|expr| (scientific), c|expr| (comma-separated)
//! - Base conversion: 0x|expr|, 0b|expr|, 0o|expr|, 0d|expr| (char/int/text conversion)
//! - Precision expressions: #.N|expr| (round), #!N|expr| (truncate)

use zymbol_ast::{
    BaseConversionExpr, BasePrefix, Expr, FormatExpr, FormatPrefix,
    NumericEvalExpr, RoundExpr, TruncExpr,
};
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;

use crate::Parser;

impl Parser {
    /// Parse numeric evaluation expression: #|expr|
    pub(crate) fn parse_numeric_eval(&mut self) -> Result<Expr, Diagnostic> {
        let start_token = self.advance(); // consume #|

        // Parse the expression inside
        let expr = Box::new(self.parse_expr()?);

        // Expect closing |
        let pipe_token = self.peek().clone();
        if !matches!(pipe_token.kind, TokenKind::Pipe) {
            return Err(Diagnostic::error("expected '|' to close numeric evaluation")
                .with_span(pipe_token.span)
                .with_help("numeric evaluation syntax: #|expr|"));
        }
        let end_token = self.advance(); // consume |

        let span = start_token.span.to(&end_token.span);
        Ok(Expr::NumericEval(NumericEvalExpr::new(expr, span)))
    }

    /// Parse format expression: e|expr| or c|expr|
    pub(crate) fn parse_format_expr(&mut self, prefix: FormatPrefix) -> Result<Expr, Diagnostic> {
        let start_token = self.advance(); // consume format prefix (e or c)

        // Expect opening |
        let pipe_token = self.peek().clone();
        if !matches!(pipe_token.kind, TokenKind::Pipe) {
            let prefix_str = match prefix {
                FormatPrefix::Scientific => "e",
                FormatPrefix::Comma => "c",
            };
            return Err(Diagnostic::error(format!("expected '|' after format prefix '{}'", prefix_str))
                .with_span(pipe_token.span)
                .with_help(format!("format expression syntax: {}|expr|", prefix_str)));
        }
        self.advance(); // consume |

        // Parse the expression inside
        let expr = Box::new(self.parse_expr()?);

        // Expect closing |
        let close_pipe_token = self.peek().clone();
        if !matches!(close_pipe_token.kind, TokenKind::Pipe) {
            let prefix_str = match prefix {
                FormatPrefix::Scientific => "e",
                FormatPrefix::Comma => "c",
            };
            return Err(Diagnostic::error("expected '|' to close format expression")
                .with_span(close_pipe_token.span)
                .with_help(format!("format expression syntax: {}|expr|", prefix_str)));
        }
        let end_token = self.advance(); // consume |

        let span = start_token.span.to(&end_token.span);
        Ok(Expr::Format(FormatExpr::new(prefix, expr, span)))
    }

    /// Parse base conversion expression: 0b|expr| or 0o|expr| or 0d|expr| or 0x|expr|
    /// Tridirectional conversion: char→text, int→char, text→char
    pub(crate) fn parse_base_conversion(&mut self, prefix: BasePrefix) -> Result<Expr, Diagnostic> {
        let start_token = self.advance(); // consume base prefix (0b, 0o, 0d, 0x)

        // Expect opening |
        let pipe_token = self.peek().clone();
        if !matches!(pipe_token.kind, TokenKind::Pipe) {
            let prefix_str = match prefix {
                BasePrefix::Binary => "0b",
                BasePrefix::Octal => "0o",
                BasePrefix::Decimal => "0d",
                BasePrefix::Hex => "0x",
            };
            return Err(Diagnostic::error(format!("expected '|' after base prefix '{}'", prefix_str))
                .with_span(pipe_token.span)
                .with_help(format!("base conversion syntax: {}|expr|", prefix_str)));
        }
        self.advance(); // consume |

        // Parse the expression inside
        let expr = Box::new(self.parse_expr()?);

        // Expect closing |
        let close_pipe_token = self.peek().clone();
        if !matches!(close_pipe_token.kind, TokenKind::Pipe) {
            let prefix_str = match prefix {
                BasePrefix::Binary => "0b",
                BasePrefix::Octal => "0o",
                BasePrefix::Decimal => "0d",
                BasePrefix::Hex => "0x",
            };
            return Err(Diagnostic::error("expected '|' to close base conversion expression")
                .with_span(close_pipe_token.span)
                .with_help(format!("base conversion syntax: {}|expr|", prefix_str)));
        }
        let end_token = self.advance(); // consume |

        let span = start_token.span.to(&end_token.span);
        Ok(Expr::BaseConversion(BaseConversionExpr::new(prefix, expr, span)))
    }

    /// Parse round expression: #.N|expr|
    /// Rounds the result to N decimal places using standard mathematical rounding.
    pub(crate) fn parse_round_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_token = self.advance(); // consume #.

        // Expect integer for precision
        let precision_token = self.peek().clone();
        let precision = match &precision_token.kind {
            TokenKind::Integer(n) => {
                if *n < 0 {
                    return Err(Diagnostic::error("precision must be a non-negative integer")
                        .with_span(precision_token.span)
                        .with_help("round expression syntax: #.N|expr| where N >= 0"));
                }
                *n as u32
            }
            _ => {
                return Err(Diagnostic::error("expected integer precision after '#.'")
                    .with_span(precision_token.span)
                    .with_help("round expression syntax: #.N|expr| (e.g., #.2|price|)"));
            }
        };
        self.advance(); // consume precision

        // Expect opening |
        let pipe_token = self.peek().clone();
        if !matches!(pipe_token.kind, TokenKind::Pipe) {
            return Err(Diagnostic::error("expected '|' after precision")
                .with_span(pipe_token.span)
                .with_help("round expression syntax: #.N|expr|"));
        }
        self.advance(); // consume |

        // Parse the expression inside
        let expr = Box::new(self.parse_expr()?);

        // Expect closing |
        let close_pipe_token = self.peek().clone();
        if !matches!(close_pipe_token.kind, TokenKind::Pipe) {
            return Err(Diagnostic::error("expected '|' to close round expression")
                .with_span(close_pipe_token.span)
                .with_help("round expression syntax: #.N|expr|"));
        }
        let end_token = self.advance(); // consume |

        let span = start_token.span.to(&end_token.span);
        Ok(Expr::Round(RoundExpr::new(precision, expr, span)))
    }

    /// Parse truncate expression: #!N|expr|
    /// Truncates the result to N decimal places (cuts without rounding).
    pub(crate) fn parse_trunc_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_token = self.advance(); // consume #!

        // Expect integer for precision
        let precision_token = self.peek().clone();
        let precision = match &precision_token.kind {
            TokenKind::Integer(n) => {
                if *n < 0 {
                    return Err(Diagnostic::error("precision must be a non-negative integer")
                        .with_span(precision_token.span)
                        .with_help("truncate expression syntax: #!N|expr| where N >= 0"));
                }
                *n as u32
            }
            _ => {
                return Err(Diagnostic::error("expected integer precision after '#!'")
                    .with_span(precision_token.span)
                    .with_help("truncate expression syntax: #!N|expr| (e.g., #!2|price|)"));
            }
        };
        self.advance(); // consume precision

        // Expect opening |
        let pipe_token = self.peek().clone();
        if !matches!(pipe_token.kind, TokenKind::Pipe) {
            return Err(Diagnostic::error("expected '|' after precision")
                .with_span(pipe_token.span)
                .with_help("truncate expression syntax: #!N|expr|"));
        }
        self.advance(); // consume |

        // Parse the expression inside
        let expr = Box::new(self.parse_expr()?);

        // Expect closing |
        let close_pipe_token = self.peek().clone();
        if !matches!(close_pipe_token.kind, TokenKind::Pipe) {
            return Err(Diagnostic::error("expected '|' to close truncate expression")
                .with_span(close_pipe_token.span)
                .with_help("truncate expression syntax: #!N|expr|"));
        }
        let end_token = self.advance(); // consume |

        let span = start_token.span.to(&end_token.span);
        Ok(Expr::Trunc(TruncExpr::new(precision, expr, span)))
    }
}
