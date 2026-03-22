//! Collection operation parsing for Zymbol-Lang
//!
//! Handles parsing of all collection operators:
//! - $# (length/size)
//! - $+ (append element)
//! - $- (remove by index)
//! - $? (contains/search)
//! - $~ (update element)
//! - $[ (slice with range)
//! - $> (map - transform collection)
//! - $| (filter - select elements)
//! - $< (reduce - accumulate)

use zymbol_ast::{
    CollectionAppendExpr, CollectionContainsExpr, CollectionLengthExpr,
    CollectionRemoveExpr, CollectionUpdateExpr, CollectionSliceExpr, Expr,
};
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;
use crate::Parser;

impl Parser {
    /// Parse collection length: collection$#
    pub(crate) fn parse_collection_length(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let op_token = self.advance(); // consume $#
        let span = collection.span().to(&op_token.span);

        Ok(Expr::CollectionLength(CollectionLengthExpr::new(
            Box::new(collection),
            span,
        )))
    }

    /// Parse collection append: collection$+ element
    pub(crate) fn parse_collection_append(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        self.advance(); // consume $+

        let element = self.parse_postfix()?; // Parse the element to append
        let span = start_span.to(&element.span());

        Ok(Expr::CollectionAppend(CollectionAppendExpr::new(
            Box::new(collection),
            Box::new(element),
            span,
        )))
    }

    /// Parse collection remove: collection$- index
    pub(crate) fn parse_collection_remove(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        self.advance(); // consume $-

        let index = self.parse_postfix()?; // Parse the index to remove
        let span = start_span.to(&index.span());

        Ok(Expr::CollectionRemove(CollectionRemoveExpr::new(
            Box::new(collection),
            Box::new(index),
            span,
        )))
    }

    /// Parse collection contains: collection$? element
    pub(crate) fn parse_collection_contains(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        self.advance(); // consume $?

        let element = self.parse_postfix()?; // Parse the element to search
        let span = start_span.to(&element.span());

        Ok(Expr::CollectionContains(CollectionContainsExpr::new(
            Box::new(collection),
            Box::new(element),
            span,
        )))
    }

    /// Parse collection update: collection[index]$~ value
    pub(crate) fn parse_collection_update(&mut self, target: Expr) -> Result<Expr, Diagnostic> {
        let start_span = target.span();

        // Target must be an IndexExpr (e.g., arr[0] or matrix[i][j])
        if !matches!(target, Expr::Index(_)) {
            return Err(Diagnostic::error("collection update ($~) requires indexed expression")
                .with_span(start_span)
                .with_help("use: arr[index]$~ value"));
        }

        self.advance(); // consume $~

        let value = self.parse_postfix()?; // Parse the new value
        let span = start_span.to(&value.span());

        Ok(Expr::CollectionUpdate(CollectionUpdateExpr::new(
            Box::new(target),
            Box::new(value),
            span,
        )))
    }

    /// Parse collection slice: collection$[start..end]
    pub(crate) fn parse_collection_slice(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        self.advance(); // consume $[

        let mut start = None;
        let mut end = None;

        // Check if starts with .. (e.g., $[..end])
        if !matches!(self.peek().kind, TokenKind::DotDot) {
            // Parse start (use parse_postfix to avoid parsing .. as range operator)
            start = Some(Box::new(self.parse_postfix()?));
        }

        // Must have ..
        if !matches!(self.peek().kind, TokenKind::DotDot) {
            return Err(Diagnostic::error("expected '..' in slice")
                .with_span(self.peek().span)
                .with_help("slice syntax: $[start..end], $[..end], or $[start..]"));
        }
        self.advance(); // consume ..

        // Check if ends immediately (e.g., $[start..])
        if !matches!(self.peek().kind, TokenKind::RBracket) {
            // Parse end (use parse_postfix to avoid parsing further operators)
            end = Some(Box::new(self.parse_postfix()?));
        }

        // Must have closing ]
        let close_token = self.peek().clone();
        if !matches!(close_token.kind, TokenKind::RBracket) {
            return Err(Diagnostic::error("expected ']' after slice range")
                .with_span(close_token.span));
        }
        self.advance(); // consume ]

        let span = start_span.to(&close_token.span);

        Ok(Expr::CollectionSlice(CollectionSliceExpr::new(
            Box::new(collection),
            start,
            end,
            span,
        )))
    }

    /// Parse collection map: collection$> lambda
    pub(crate) fn parse_collection_map(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        self.advance(); // consume $>

        let lambda = self.parse_lambda()?; // Parse the lambda function
        let span = start_span.to(&lambda.span());

        Ok(Expr::CollectionMap(zymbol_ast::CollectionMapExpr {
            collection: Box::new(collection),
            lambda: Box::new(lambda),
            span,
        }))
    }

    /// Parse collection filter: collection$| lambda
    pub(crate) fn parse_collection_filter(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        self.advance(); // consume $|

        let lambda = self.parse_lambda()?; // Parse the lambda function
        let span = start_span.to(&lambda.span());

        Ok(Expr::CollectionFilter(zymbol_ast::CollectionFilterExpr {
            collection: Box::new(collection),
            lambda: Box::new(lambda),
            span,
        }))
    }

    /// Parse collection reduce: collection$< (initial, lambda)
    pub(crate) fn parse_collection_reduce(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        self.advance(); // consume $<

        // Expect (
        let lparen_token = self.peek().clone();
        if !matches!(lparen_token.kind, TokenKind::LParen) {
            return Err(Diagnostic::error("expected '(' after $<")
                .with_span(lparen_token.span)
                .with_help("reduce syntax: collection$< (initial, lambda)"));
        }
        self.advance(); // consume (

        let initial = self.parse_expr()?; // Parse initial value

        // Expect ,
        let comma_token = self.peek().clone();
        if !matches!(comma_token.kind, TokenKind::Comma) {
            return Err(Diagnostic::error("expected ',' after initial value")
                .with_span(comma_token.span)
                .with_help("reduce syntax: collection$< (initial, lambda)"));
        }
        self.advance(); // consume ,

        let lambda = self.parse_lambda()?; // Parse the lambda function

        // Expect )
        let close_token = self.peek().clone();
        if !matches!(close_token.kind, TokenKind::RParen) {
            return Err(Diagnostic::error("expected ')' after lambda")
                .with_span(close_token.span)
                .with_help("reduce syntax: collection$< (initial, lambda)"));
        }
        self.advance(); // consume )

        let span = start_span.to(&close_token.span);

        Ok(Expr::CollectionReduce(zymbol_ast::CollectionReduceExpr {
            collection: Box::new(collection),
            initial: Box::new(initial),
            lambda: Box::new(lambda),
            span,
        }))
    }
}
