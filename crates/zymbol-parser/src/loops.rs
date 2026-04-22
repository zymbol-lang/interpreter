//! Loop parsing for Zymbol-Lang (GRUPO 6: LOOPS)
//!
//! Handles parsing of loop constructs:
//! - Universal loop: @ [condition] { }
//! - For-each loop: @ var:iterable { }
//! - Loop control: BREAK (@!), CONTINUE (@>)
//! - Labeled loops: @label { }  (fused — @label is a single token)

use zymbol_ast::{Break, Continue, Loop, Statement};
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;
use crate::Parser;

impl Parser {
    /// Parse break statement: @! [label] or @:label!
    pub(crate) fn parse_break(&mut self) -> Result<Statement, Diagnostic> {
        let token = self.advance(); // consume @! or @:label!
        let start_span = token.span;

        let label = match &token.kind {
            // @:label! — label is embedded in the token
            TokenKind::AtColonLabelBreak(name) => Some(name.clone()),
            // @! — check for optional space-separated label (legacy @! label)
            _ => {
                if matches!(self.peek().kind, TokenKind::Ident(_)) {
                    let label_token = self.advance();
                    match &label_token.kind {
                        TokenKind::Ident(name) => Some(name.clone()),
                        _ => unreachable!(),
                    }
                } else {
                    None
                }
            }
        };

        Ok(Statement::Break(Break::new(label, start_span)))
    }

    /// Parse continue statement: @> [label] or @:label>
    pub(crate) fn parse_continue(&mut self) -> Result<Statement, Diagnostic> {
        let token = self.advance(); // consume @> or @:label>
        let start_span = token.span;

        let label = match &token.kind {
            // @:label> — label is embedded in the token
            TokenKind::AtColonLabelContinue(name) => Some(name.clone()),
            // @> — check for optional space-separated label (legacy @> label)
            _ => {
                if matches!(self.peek().kind, TokenKind::Ident(_)) {
                    let label_token = self.advance();
                    match &label_token.kind {
                        TokenKind::Ident(name) => Some(name.clone()),
                        _ => unreachable!(),
                    }
                } else {
                    None
                }
            }
        };

        Ok(Statement::Continue(Continue::new(label, start_span)))
    }

    /// Parse loop statement: @ condition { } or @ var:iterable { }
    pub(crate) fn parse_loop(&mut self) -> Result<Statement, Diagnostic> {
        // Consume the opening token: @, @label (legacy), or @:label
        let opening = self.advance();
        let start_span = opening.span;

        // Extract label from @label (legacy) or @:label token
        let label = match &opening.kind {
            TokenKind::AtLabel(name) | TokenKind::AtColonLabel(name) => Some(name.clone()),
            _ => None,
        };

        // Check for for-each syntax: var:iterable
        // We need to look ahead to distinguish from while loop
        let is_for_each = matches!(self.peek().kind, TokenKind::Ident(_))
            && self.peek_ahead(1).map(|t| matches!(t.kind, TokenKind::Colon)).unwrap_or(false);

        if is_for_each {
            // For-each loop: @ var:iterable { }
            let var_token = self.advance();
            let iterator_var = match &var_token.kind {
                TokenKind::Ident(name) => name.clone(),
                _ => unreachable!(),
            };

            // Consume colon
            let colon_token = self.peek();
            if !matches!(colon_token.kind, TokenKind::Colon) {
                return Err(Diagnostic::error("expected ':' after iterator variable")
                    .with_span(colon_token.span)
                    .with_help("for-each syntax: @ var:iterable { }"));
            }
            self.advance(); // consume :

            // Parse iterable expression
            let iterable = Box::new(self.parse_expr()?);

            // Parse body block
            let body = self.parse_block()?;

            let span = start_span.to(&body.span);

            Ok(Statement::Loop(Loop::for_each(
                iterator_var,
                iterable,
                body,
                label,
                span,
            )))
        } else {
            // While loop or infinite loop
            let condition = if matches!(self.peek().kind, TokenKind::LBrace) {
                // No condition - infinite loop
                None
            } else {
                // Parse condition expression
                Some(Box::new(self.parse_expr()?))
            };

            // Parse body block
            let body = self.parse_block()?;

            let span = start_span.to(&body.span);

            Ok(Statement::Loop(Loop::new(condition, body, label, span)))
        }
    }
}
