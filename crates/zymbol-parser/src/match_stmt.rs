//! MATCH expression and pattern matching parsing for Zymbol-Lang
//!
//! Handles parsing of:
//! - MATCH expression: ?? expr { cases }
//! - MATCH statement: ?? expr { cases } (discards value)
//! - Pattern matching: All pattern types (literals, ranges, lists, guards)

use zymbol_ast::{Expr, LiteralExpr, MatchCase, MatchExpr, Pattern, Statement};
use zymbol_common::Literal;
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;
use crate::Parser;

impl Parser {
    /// Parse match expression: ?? expr { pattern : value }
    pub(crate) fn parse_match_expr(&mut self) -> Result<Expr, Diagnostic> {
        let match_expr = self.parse_match_expr_inner()?;
        Ok(Expr::Match(match_expr))
    }

    /// Parse match as a statement: ?? expr { cases } (discards return value)
    pub(crate) fn parse_match_statement(&mut self) -> Result<Statement, Diagnostic> {
        let match_expr = self.parse_match_expr_inner()?;
        Ok(Statement::Match(match_expr))
    }

    /// Parse match expression internals (common logic for both expression and statement forms)
    pub(crate) fn parse_match_expr_inner(&mut self) -> Result<MatchExpr, Diagnostic> {
        let start_token = self.advance(); // consume ??

        // Parse scrutinee expression
        let scrutinee = Box::new(self.parse_expr()?);

        // Expect {
        let lbrace_token = self.peek().clone();
        if !matches!(lbrace_token.kind, TokenKind::LBrace) {
            return Err(Diagnostic::error("expected '{' after match expression")
                .with_span(lbrace_token.span)
                .with_help("match syntax: ?? expr { pattern : value }"));
        }
        self.advance(); // consume {

        // Parse match cases
        let mut cases = Vec::new();

        while !matches!(self.peek().kind, TokenKind::RBrace) && !self.is_at_end() {
            let case_start = self.peek().span;

            // Parse pattern
            let pattern = self.parse_pattern()?;

            // Expect :
            let colon_token = self.peek().clone();
            if !matches!(colon_token.kind, TokenKind::Colon) {
                return Err(Diagnostic::error("expected ':' after pattern")
                    .with_span(colon_token.span)
                    .with_help("match case syntax: pattern : [value] [{ block }]"));
            }
            self.advance(); // consume :

            // Check if we have a block-only case (no value)
            let (value, block) = if matches!(self.peek().kind, TokenKind::LBrace) {
                // Block-only case: pattern : { block }
                let block = Some(self.parse_block()?);
                (None, block)
            } else {
                // Value case: pattern : value [ { block } ]
                let value = Some(self.parse_expr()?);
                let block = if matches!(self.peek().kind, TokenKind::LBrace) {
                    Some(self.parse_block()?)
                } else {
                    None
                };
                (value, block)
            };

            let case_end = if let Some(ref blk) = block {
                blk.span
            } else if let Some(ref val) = value {
                val.span()
            } else {
                // This shouldn't happen - we should have either value or block
                colon_token.span
            };

            let case_span = case_start.to(&case_end);
            cases.push(MatchCase::new(pattern, value, block, case_span));
        }

        // Expect }
        let rbrace_token = self.peek().clone();
        if !matches!(rbrace_token.kind, TokenKind::RBrace) {
            return Err(Diagnostic::error("expected '}' to close match expression")
                .with_span(rbrace_token.span)
                .with_help("match expression must be enclosed in braces"));
        }
        self.advance(); // consume }

        let span = start_token.span.to(&rbrace_token.span);
        Ok(MatchExpr::new(scrutinee, cases, span))
    }

    /// Parse pattern for match expressions
    pub(crate) fn parse_pattern(&mut self) -> Result<Pattern, Diagnostic> {
        let token = self.peek().clone();

        let pattern = match &token.kind {
            TokenKind::Underscore => {
                self.advance(); // consume _
                Pattern::Wildcard(token.span)
            }
            TokenKind::String(s) => {
                let s = s.clone();
                self.advance(); // consume string
                Pattern::Literal(Literal::String(s), token.span)
            }
            TokenKind::Integer(n) => {
                let n = *n;
                let start_span = token.span;
                self.advance(); // consume integer

                // Check for range pattern: int..int
                if matches!(self.peek().kind, TokenKind::DotDot) {
                    self.advance(); // consume ..

                    let end_token = self.peek().clone();
                    match &end_token.kind {
                        TokenKind::Integer(end_n) => {
                            let end_n = *end_n;
                            self.advance(); // consume end integer

                            let span = start_span.to(&end_token.span);
                            let start_expr = Box::new(Expr::Literal(LiteralExpr::new(
                                Literal::Int(n),
                                start_span,
                            )));
                            let end_expr = Box::new(Expr::Literal(LiteralExpr::new(
                                Literal::Int(end_n),
                                end_token.span,
                            )));
                            Pattern::Range(start_expr, end_expr, span)
                        }
                        _ => {
                            return Err(Diagnostic::error("expected integer after '..' in range pattern")
                                .with_span(end_token.span));
                        }
                    }
                } else {
                    Pattern::Literal(Literal::Int(n), token.span)
                }
            }
            TokenKind::Char(c) => {
                let c = *c;
                let start_span = token.span;
                self.advance(); // consume char

                // Check for range pattern: 'a'..'z'
                if matches!(self.peek().kind, TokenKind::DotDot) {
                    self.advance(); // consume ..

                    let end_token = self.peek().clone();
                    match &end_token.kind {
                        TokenKind::Char(end_c) => {
                            let end_c = *end_c;
                            self.advance(); // consume end char

                            let span = start_span.to(&end_token.span);
                            let start_expr = Box::new(Expr::Literal(LiteralExpr::new(
                                Literal::Char(c),
                                start_span,
                            )));
                            let end_expr = Box::new(Expr::Literal(LiteralExpr::new(
                                Literal::Char(end_c),
                                end_token.span,
                            )));
                            Pattern::Range(start_expr, end_expr, span)
                        }
                        _ => {
                            return Err(Diagnostic::error("expected char after '..' in range pattern")
                                .with_span(end_token.span));
                        }
                    }
                } else {
                    Pattern::Literal(Literal::Char(c), token.span)
                }
            }
            TokenKind::Float(f) => {
                let f = *f;
                self.advance(); // consume float
                Pattern::Literal(Literal::Float(f), token.span)
            }
            TokenKind::Boolean(b) => {
                let b = *b;
                self.advance(); // consume boolean
                Pattern::Literal(Literal::Bool(b), token.span)
            }
            TokenKind::LBracket => {
                // Parse list pattern: [pat1, pat2, ...]
                let start_token = self.advance(); // consume [
                let mut patterns = Vec::new();

                // Handle empty list []
                if matches!(self.peek().kind, TokenKind::RBracket) {
                    let end_token = self.advance(); // consume ]
                    let span = start_token.span.to(&end_token.span);
                    return Ok(Pattern::List(patterns, span));
                }

                // Parse first pattern
                patterns.push(self.parse_pattern()?);

                // Parse remaining patterns (comma-separated)
                while matches!(self.peek().kind, TokenKind::Comma) {
                    self.advance(); // consume ,

                    // Allow trailing comma
                    if matches!(self.peek().kind, TokenKind::RBracket) {
                        break;
                    }

                    patterns.push(self.parse_pattern()?);
                }

                // Expect ]
                let end_token = self.peek().clone();
                if !matches!(end_token.kind, TokenKind::RBracket) {
                    return Err(Diagnostic::error("expected ']' to close list pattern")
                        .with_span(end_token.span));
                }
                self.advance(); // consume ]

                let span = start_token.span.to(&end_token.span);
                Pattern::List(patterns, span)
            }
            // _? is tokenized as ElseIf — treat as guard pattern with Wildcard base
            TokenKind::ElseIf => {
                self.advance(); // consume _?
                let condition = Box::new(self.parse_expr()?);
                let span = token.span.to(&condition.span());
                return Ok(Pattern::Guard(
                    Box::new(Pattern::Wildcard(token.span)),
                    condition,
                    span,
                ));
            }
            _ => {
                return Err(Diagnostic::error(format!(
                    "expected pattern, found {:?}",
                    token.kind
                ))
                .with_span(token.span));
            }
        };

        // Check for guard: pattern ? condition
        if matches!(self.peek().kind, TokenKind::Question) {
            self.advance(); // consume ?

            let condition = Box::new(self.parse_expr()?);
            let span = pattern.span().to(&condition.span());
            Ok(Pattern::Guard(Box::new(pattern), condition, span))
        } else {
            Ok(pattern)
        }
    }
}
