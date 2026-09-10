//! Loop parsing for Zymbol-Lang (GRUPO 6: LOOPS)
//!
//! Handles parsing of loop constructs:
//! - Universal loop: @ [condition] { }
//! - For-each loop: @ var:iterable { }
//! - Loop control: BREAK (@!), CONTINUE (@>)
//! - Labeled loops: @label { }  (fused — @label is a single token)

use zymbol_ast::{Break, Continue, Loop, Sleep, Statement};
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;
use crate::Parser;

impl Parser {
    /// Parse sleep statement: @~ N (milliseconds)
    pub(crate) fn parse_sleep(&mut self) -> Result<Statement, Diagnostic> {
        let start_span = self.advance().span; // consume @~
        let duration = self.parse_expr()?;
        let span = start_span.to(&duration.span());
        Ok(Statement::Sleep(Sleep::new(Box::new(duration), span)))
    }

    /// Parse break statement: @! or @:label!
    pub(crate) fn parse_break(&mut self) -> Result<Statement, Diagnostic> {
        let token = self.advance(); // consume @! or @:label!
        let start_span = token.span;

        let label = match &token.kind {
            TokenKind::AtColonLabelBreak(name) => Some(name.clone()),
            _ => None,
        };

        Ok(Statement::Break(Break::new(label, start_span)))
    }

    /// Parse continue statement: @> or @:label>
    pub(crate) fn parse_continue(&mut self) -> Result<Statement, Diagnostic> {
        let token = self.advance(); // consume @> or @:label>
        let start_span = token.span;

        let label = match &token.kind {
            TokenKind::AtColonLabelContinue(name) => Some(name.clone()),
            _ => None,
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

        // `@ (k, v):pares { … }` — a destructuring pattern where a single name
        // would go. It binds each element the way `(k, v) = par` binds one, so
        // the loop stops needing a first line whose only job is to unpack.
        //
        // `@ (` is already taken: `@ (n + 1) { }` is a valid count loop. The
        // disambiguator is the `:` after the `)`, which is the same kind of scan
        // `is_tuple_destructure` already does at statement level.
        if matches!(self.peek().kind, TokenKind::LParen | TokenKind::LBracket)
            && self.is_loop_pattern()
        {
            let pattern = if matches!(self.peek().kind, TokenKind::LBracket) {
                self.parse_array_destructure_pattern()?
            } else {
                self.parse_tuple_destructure_pattern()?
            };
            self.advance(); // consume :
            let iterable = Box::new(self.parse_expr()?);
            let body = self.parse_block()?;
            let span = start_span.to(&body.span);
            return Ok(Statement::Loop(Loop::for_each_pattern(
                pattern, iterable, body, label, span,
            )));
        }

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

impl Parser {
    /// True when a loop head opens with a destructuring pattern rather than a
    /// condition: `@ (k, v):pares` and `@ [a, b]:filas`, but not `@ (n + 1) { }`.
    ///
    /// Saves and restores the position, like the other `is_*` probes; the answer
    /// is simply whether a `:` follows the balanced bracket.
    fn is_loop_pattern(&mut self) -> bool {
        let saved = self.current;
        let (open, close) = match self.peek().kind {
            TokenKind::LBracket => (TokenKind::LBracket, TokenKind::RBracket),
            _ => (TokenKind::LParen, TokenKind::RParen),
        };
        self.advance();
        let mut depth = 1i32;
        while depth > 0 && !self.is_at_end() {
            let k = self.peek().kind.clone();
            if k == open { depth += 1; } else if k == close { depth -= 1; }
            self.advance();
        }
        let answer = matches!(self.peek().kind, TokenKind::Colon);
        self.current = saved;
        answer
    }
}
