//! Data operation parsing for Zymbol-Lang
//!
//! Handles parsing of data transformation and introspection expressions:
//! - Numeric evaluation: #|expr| (safe string-to-number conversion)
//! - Type metadata: expr#? (returns type info tuple)
//! - Format expressions: #,|expr| (thousands), #^|expr| (scientific)
//! - Base conversion: 0x|expr|, 0b|expr|, 0o|expr|, 0d|expr| (char/int/text conversion)
//! - Precision expressions: #.N|expr| (round), #!N|expr| (truncate)

use zymbol_ast::{
    BaseConversionExpr, BasePrefix, Expr, FormatExpr, FormatKind, PrecisionOp,
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

    /// Parse format expression: #,|expr| or #^|expr|, with optional precision modifier.
    ///
    /// Grammar: format_kind [ precision_mod ] "|" expr "|"
    /// - precision_mod: "." integer (round) | "!" integer (truncate)
    pub(crate) fn parse_format_expr(&mut self, kind: FormatKind) -> Result<Expr, Diagnostic> {
        let start_token = self.advance(); // consume #, or #^

        let prefix_str = match kind {
            FormatKind::Thousands => "#,",
            FormatKind::Scientific => "#^",
        };

        // Optional precision: .N (round) or !N (truncate)
        let precision = match self.peek().kind.clone() {
            TokenKind::Dot => {
                self.advance(); // consume .
                let n = self.parse_format_precision(prefix_str)?;
                Some(PrecisionOp::Round(n))
            }
            TokenKind::Not => {
                self.advance(); // consume !
                let n = self.parse_format_precision(prefix_str)?;
                Some(PrecisionOp::Truncate(n))
            }
            _ => None,
        };

        // Expect opening |
        let pipe_token = self.peek().clone();
        if !matches!(pipe_token.kind, TokenKind::Pipe) {
            return Err(Diagnostic::error(format!("expected '|' after format operator '{}'", prefix_str))
                .with_span(pipe_token.span)
                .with_help(format!("format expression syntax: {}|expr| or {}.N|expr|", prefix_str, prefix_str)));
        }
        self.advance(); // consume |

        // Parse the expression inside
        let expr = Box::new(self.parse_expr()?);

        // Expect closing |
        let close_pipe_token = self.peek().clone();
        if !matches!(close_pipe_token.kind, TokenKind::Pipe) {
            return Err(Diagnostic::error("expected '|' to close format expression")
                .with_span(close_pipe_token.span)
                .with_help(format!("format expression syntax: {}|expr|", prefix_str)));
        }
        let end_token = self.advance(); // consume |

        let span = start_token.span.to(&end_token.span);
        Ok(Expr::Format(FormatExpr::new(kind, precision, expr, span)))
    }

    /// Parse the decimal count after '.' or '!' in a format expression.
    ///
    /// A literal keeps its fast path. Anything else — a name, a call, a
    /// parenthesised expression — is kept as an expression and evaluated when
    /// the program runs (GAP-ZYB-001): the number of decimals a money amount
    /// takes belongs to the currency, so it is configuration and cannot always
    /// be written in the source.
    fn parse_format_precision(&mut self, prefix_str: &str) -> Result<zymbol_ast::Precision, Diagnostic> {
        let precision_token = self.peek().clone();
        match &precision_token.kind {
            TokenKind::Integer(n) => {
                if *n < 0 {
                    return Err(Diagnostic::error("precision must be a non-negative integer")
                        .with_span(precision_token.span)
                        .with_help(format!("format expression syntax: {}.N|expr| where N >= 0", prefix_str)));
                }
                let n = *n as u32;
                self.advance(); // consume integer
                Ok(zymbol_ast::Precision::Literal(n))
            }
            // A computed count, written as a plain name.
            //
            // A name and not an expression, deliberately. The `|` that opens
            // the value is also how bitwise-or is spelled, so a general
            // expression here would have to stop at a delimiter that is itself
            // an operator — and the browser engine lexes this count as part of
            // the token, so anything it cannot scan in one pass would diverge.
            // A computed count comes from a variable or a parameter in every
            // case this exists for; compute it into a name first if it is more:
            //
            //     ancho = exp + 1
            //     >> #,.ancho|importe| ¶
            TokenKind::Ident(name) => {
                let name = name.clone();
                let span = precision_token.span;
                self.advance();
                Ok(zymbol_ast::Precision::Dynamic(Box::new(Expr::Identifier(
                    zymbol_ast::IdentifierExpr::new(name, span),
                ))))
            }
            _ => Err(Diagnostic::error(format!("expected a decimal count after '{}'", prefix_str))
                .with_span(precision_token.span)
                .with_help(format!(
                    "write the count or the name of a variable holding it: {}.2|value| or {}.n|value|",
                    prefix_str, prefix_str
                ))),
        }
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

        // The decimal count: a literal, or an expression evaluated at run time
        // (GAP-ZYB-001 — see `Precision`).
        let precision = self.parse_format_precision("#.")?;

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

        // The decimal count: a literal, or an expression evaluated at run time
        // (GAP-ZYB-001 — see `Precision`).
        let precision = self.parse_format_precision("#!")?;

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
