//! String operation parsing for Zymbol-Lang
//!
//! Handles parsing of string-specific operators:
//! - $?? (find all indices of value — now unified across arrays, tuples, strings)
//! - $++ (RETIRED in v0.0.2 — emits migration error, use $+[ instead)
//! - $~~ (replace pattern with replacement text)
//!
//! Note: $+[i] (insert) and $-[i..j] (remove range) are handled in collection_ops
//! since they apply uniformly to arrays, tuples, and strings.

use zymbol_ast::{
    CollectionFindAllExpr, StringReplaceExpr, Expr,
};
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;
use crate::Parser;

impl Parser {
    /// Parse collection find all: collection$?? value
    /// Returns an array of indices where value is found (arrays, tuples, strings)
    pub(crate) fn parse_string_find_positions(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        self.advance(); // consume $??

        let value = self.parse_postfix()?; // Parse the value to search for
        let span = start_span.to(&value.span());

        Ok(Expr::CollectionFindAll(CollectionFindAllExpr::new(
            Box::new(collection),
            Box::new(value),
            span,
        )))
    }

    /// RETIRED: $++ was string insert in v0.0.1 — emits a migration error
    /// Use string$+[position] text instead
    pub(crate) fn parse_string_insert(&mut self, _collection: Expr) -> Result<Expr, Diagnostic> {
        let op_token = self.advance(); // consume $++
        Err(Diagnostic::error("$++ is retired — use $+[position] element instead")
            .with_span(op_token.span)
            .with_help("v0.0.2: string$++[p:text] → string$+[p] text"))
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
