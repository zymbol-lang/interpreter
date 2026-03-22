//! String operation parsing for Zymbol-Lang
//!
//! Handles parsing of all string operators:
//! - $?? (find all positions of pattern in string)
//! - $++ (insert text at position)
//! - $-- (remove text by count)
//! - $~~ (replace pattern with replacement text)

use zymbol_ast::{
    StringFindPositionsExpr, StringInsertExpr, StringRemoveExpr, StringReplaceExpr, Expr,
};
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;
use crate::Parser;

impl Parser {
    /// Parse string find positions: string$?? pattern
    /// Returns an array of integer positions where the pattern is found
    pub(crate) fn parse_string_find_positions(&mut self, string: Expr) -> Result<Expr, Diagnostic> {
        let start_span = string.span();
        self.advance(); // consume $??

        let pattern = self.parse_postfix()?; // Parse the pattern to search for
        let span = start_span.to(&pattern.span());

        Ok(Expr::StringFindPositions(StringFindPositionsExpr::new(
            Box::new(string),
            Box::new(pattern),
            span,
        )))
    }

    /// Parse string insert: string$++[position:text]
    /// Inserts text at the specified position
    pub(crate) fn parse_string_insert(&mut self, string: Expr) -> Result<Expr, Diagnostic> {
        let start_span = string.span();
        self.advance(); // consume $++

        // Expect [
        let lbracket_token = self.peek().clone();
        if !matches!(lbracket_token.kind, TokenKind::LBracket) {
            return Err(Diagnostic::error("expected '[' after $++")
                .with_span(lbracket_token.span)
                .with_help("syntax: string$++[position:text]"));
        }
        self.advance(); // consume [

        // Parse position expression
        let position = self.parse_expr()?;

        // Expect :
        let colon_token = self.peek().clone();
        if !matches!(colon_token.kind, TokenKind::Colon) {
            return Err(Diagnostic::error("expected ':' after position")
                .with_span(colon_token.span)
                .with_help("syntax: string$++[position:text]"));
        }
        self.advance(); // consume :

        // Parse text expression
        let text = self.parse_expr()?;

        // Expect ]
        let rbracket_token = self.peek().clone();
        if !matches!(rbracket_token.kind, TokenKind::RBracket) {
            return Err(Diagnostic::error("expected ']' after text")
                .with_span(rbracket_token.span)
                .with_help("syntax: string$++[position:text]"));
        }
        self.advance(); // consume ]

        let span = start_span.to(&rbracket_token.span);

        Ok(Expr::StringInsert(StringInsertExpr::new(
            Box::new(string),
            Box::new(position),
            Box::new(text),
            span,
        )))
    }

    /// Parse string remove: string$--[position:count]
    /// Removes count characters starting at position
    pub(crate) fn parse_string_remove(&mut self, string: Expr) -> Result<Expr, Diagnostic> {
        let start_span = string.span();
        self.advance(); // consume $--

        // Expect [
        let lbracket_token = self.peek().clone();
        if !matches!(lbracket_token.kind, TokenKind::LBracket) {
            return Err(Diagnostic::error("expected '[' after $--")
                .with_span(lbracket_token.span)
                .with_help("syntax: string$--[position:count]"));
        }
        self.advance(); // consume [

        // Parse position expression
        let position = self.parse_expr()?;

        // Expect :
        let colon_token = self.peek().clone();
        if !matches!(colon_token.kind, TokenKind::Colon) {
            return Err(Diagnostic::error("expected ':' after position")
                .with_span(colon_token.span)
                .with_help("syntax: string$--[position:count]"));
        }
        self.advance(); // consume :

        // Parse count expression
        let count = self.parse_expr()?;

        // Expect ]
        let rbracket_token = self.peek().clone();
        if !matches!(rbracket_token.kind, TokenKind::RBracket) {
            return Err(Diagnostic::error("expected ']' after count")
                .with_span(rbracket_token.span)
                .with_help("syntax: string$--[position:count]"));
        }
        self.advance(); // consume ]

        let span = start_span.to(&rbracket_token.span);

        Ok(Expr::StringRemove(StringRemoveExpr::new(
            Box::new(string),
            Box::new(position),
            Box::new(count),
            span,
        )))
    }

    /// Parse string replace: string$~~[pattern:replacement] or string$~~[pattern:replacement:count]
    /// Replaces pattern with replacement text
    /// - If count not provided or is 0, replaces all occurrences
    /// - If count is N, replaces first N occurrences
    pub(crate) fn parse_string_replace(&mut self, string: Expr) -> Result<Expr, Diagnostic> {
        let start_span = string.span();
        self.advance(); // consume $~~

        // Expect [
        let lbracket_token = self.peek().clone();
        if !matches!(lbracket_token.kind, TokenKind::LBracket) {
            return Err(Diagnostic::error("expected '[' after $~~")
                .with_span(lbracket_token.span)
                .with_help("syntax: string$~~[pattern:replacement] or string$~~[pattern:replacement:count]"));
        }
        self.advance(); // consume [

        // Parse pattern expression
        let pattern = self.parse_expr()?;

        // Expect :
        let colon_token = self.peek().clone();
        if !matches!(colon_token.kind, TokenKind::Colon) {
            return Err(Diagnostic::error("expected ':' after pattern")
                .with_span(colon_token.span)
                .with_help("syntax: string$~~[pattern:replacement:count?]"));
        }
        self.advance(); // consume :

        // Parse replacement expression
        let replacement = self.parse_expr()?;

        // Check for optional count parameter
        let count = if matches!(self.peek().kind, TokenKind::Colon) {
            self.advance(); // consume :
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        // Expect ]
        let rbracket_token = self.peek().clone();
        if !matches!(rbracket_token.kind, TokenKind::RBracket) {
            return Err(Diagnostic::error("expected ']' after replacement or count")
                .with_span(rbracket_token.span)
                .with_help("syntax: string$~~[pattern:replacement:count?]"));
        }
        self.advance(); // consume ]

        let span = start_span.to(&rbracket_token.span);

        Ok(Expr::StringReplace(StringReplaceExpr::new(
            Box::new(string),
            Box::new(pattern),
            Box::new(replacement),
            count,
            span,
        )))
    }
}
