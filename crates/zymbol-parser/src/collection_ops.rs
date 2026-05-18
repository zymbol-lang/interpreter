//! Collection operation parsing for Zymbol-Lang
//!
//! Handles parsing of all collection operators:
//! - $# (length/size)
//! - $+ (append element by value)
//! - $+[i] val (insert element at position — DollarPlusLBracket)
//! - $- val (remove first occurrence of value)
//! - $-- val (remove all occurrences of value)
//! - $-[i] (remove element at index — DollarMinusLBracket)
//! - $-[i..j] (remove range of elements — DollarMinusLBracket)
//! - $? (contains/search by value)
//! - $?? (find all indices of value)
//! - $~ (update element at index)
//! - $[ (slice with range)
//! - $> (map - transform collection)
//! - $| (filter - select elements)
//! - $< (reduce - accumulate)

use zymbol_ast::{
    BinaryExpr, CollectionAppendExpr, CollectionContainsExpr, CollectionInsertExpr,
    CollectionLengthExpr, CollectionRemoveAllExpr, CollectionRemoveAtExpr,
    CollectionRemoveRangeExpr, CollectionRemoveValueExpr, CollectionUpdateExpr,
    CollectionSliceExpr, CollectionSortExpr, Expr,
};
use zymbol_common::BinaryOp;
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

        let element = self.parse_postfix_structural()?; // Stop before next $+ to allow chaining
        let span = start_span.to(&element.span());

        Ok(Expr::CollectionAppend(CollectionAppendExpr::new(
            Box::new(collection),
            Box::new(element),
            span,
        )))
    }

    /// Parse collection remove value: collection$- value
    /// Removes the first occurrence of value (by value, not by index)
    pub(crate) fn parse_collection_remove(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        self.advance(); // consume $-

        let value = self.parse_postfix()?;
        let span = start_span.to(&value.span());

        Ok(Expr::CollectionRemoveValue(CollectionRemoveValueExpr::new(
            Box::new(collection),
            Box::new(value),
            span,
        )))
    }

    /// Parse collection remove all: collection$-- value
    /// Removes all occurrences of value.
    /// Emits a migration error if followed by '[' (retired $--[pos:n] syntax).
    pub(crate) fn parse_collection_remove_all(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        self.advance(); // consume $--

        // Retired $--[position:count] syntax
        if matches!(self.peek().kind, TokenKind::LBracket) {
            let bracket_token = self.peek().clone();
            return Err(Diagnostic::error("$--[position:count] is retired — use $-[start..end] instead")
                .with_span(bracket_token.span)
                .with_help("v0.0.2: s$--[0:6] → s$-[0..6]"));
        }

        let value = self.parse_postfix()?;
        let span = start_span.to(&value.span());

        Ok(Expr::CollectionRemoveAll(CollectionRemoveAllExpr::new(
            Box::new(collection),
            Box::new(value),
            span,
        )))
    }

    /// Parse collection insert: collection$+[index] element
    /// `DollarPlusLBracket` (`$+[`) is a single token — the `[` is already consumed.
    pub(crate) fn parse_collection_insert(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        self.advance(); // consume $+[

        // Parse index expression
        let index = self.parse_expr()?;

        // Expect ]
        let close = self.peek().clone();
        if !matches!(close.kind, TokenKind::RBracket) {
            return Err(Diagnostic::error("expected ']' after index in $+[index]")
                .with_span(close.span)
                .with_help("insert syntax: collection$+[index] element"));
        }
        self.advance(); // consume ]

        // Parse element to insert
        let element = self.parse_postfix()?;
        let span = start_span.to(&element.span());

        Ok(Expr::CollectionInsert(CollectionInsertExpr::new(
            Box::new(collection),
            Box::new(index),
            Box::new(element),
            span,
        )))
    }

    /// Parse collection remove positional: collection$-[index] or collection$-[start..end]
    /// `DollarMinusLBracket` (`$-[`) is a single token — the `[` is already consumed.
    pub(crate) fn parse_collection_remove_positional(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        self.advance(); // consume $-[

        // Case: $-[..end] or $-[..] — open start range
        if matches!(self.peek().kind, TokenKind::DotDot) {
            self.advance(); // consume ..
            let end = if !matches!(self.peek().kind, TokenKind::RBracket) {
                Some(Box::new(self.parse_postfix()?))
            } else {
                None // $-[..] → remove all (empty collection)
            };
            let close = self.peek().clone();
            if !matches!(close.kind, TokenKind::RBracket) {
                return Err(Diagnostic::error("expected ']' after range")
                    .with_span(close.span)
                    .with_help("range syntax: $-[..end] or $-[..]"));
            }
            self.advance(); // consume ]
            let span = start_span.to(&close.span);
            return Ok(Expr::CollectionRemoveRange(CollectionRemoveRangeExpr::new(
                Box::new(collection), None, end, span,
            )));
        }

        // Parse first expression (use parse_postfix to avoid consuming .. as range operator)
        let first = self.parse_postfix()?;

        // Case: $-[start..end] or $-[start..] — range with explicit start
        if matches!(self.peek().kind, TokenKind::DotDot) {
            self.advance(); // consume ..
            let end = if !matches!(self.peek().kind, TokenKind::RBracket) {
                Some(Box::new(self.parse_postfix()?))
            } else {
                None // $-[start..] → remove from start to end
            };
            let close = self.peek().clone();
            if !matches!(close.kind, TokenKind::RBracket) {
                return Err(Diagnostic::error("expected ']' after range")
                    .with_span(close.span)
                    .with_help("range syntax: $-[start..end] or $-[start..]"));
            }
            self.advance(); // consume ]
            let span = start_span.to(&close.span);
            return Ok(Expr::CollectionRemoveRange(CollectionRemoveRangeExpr::new(
                Box::new(collection), Some(Box::new(first)), end, span,
            )));
        }

        // Case: $-[start:count] — count-based range (alternative syntax)
        if matches!(self.peek().kind, TokenKind::Colon) {
            self.advance(); // consume :
            let count = self.parse_postfix()?;
            let close = self.peek().clone();
            if !matches!(close.kind, TokenKind::RBracket) {
                return Err(Diagnostic::error("expected ']' after count")
                    .with_span(close.span)
                    .with_help("count-based range syntax: $-[start:count]"));
            }
            self.advance(); // consume ]
            let span = start_span.to(&close.span);
            return Ok(Expr::CollectionRemoveRange(CollectionRemoveRangeExpr::new_count(
                Box::new(collection), Some(Box::new(first)), Some(Box::new(count)), span,
            )));
        }

        // Case: $-[index] — single positional remove
        let close = self.peek().clone();
        if !matches!(close.kind, TokenKind::RBracket) {
            return Err(Diagnostic::error("expected ']', '..', or ':' after index")
                .with_span(close.span)
                .with_help("remove syntax: $-[index], $-[start..end], or $-[start:count]"));
        }
        self.advance(); // consume ]
        let span = start_span.to(&close.span);
        Ok(Expr::CollectionRemoveAt(CollectionRemoveAtExpr::new(
            Box::new(collection),
            Box::new(first),
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
    /// Parse a slice bound: postfix expression with optional additive arithmetic (+/-).
    /// Does NOT parse `..` or higher-precedence operators so the slice separator is safe.
    /// Supports: p, p-1, p+1, arr$#-1, -1 (GAP-001 fix).
    fn parse_slice_bound(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_postfix()?;
        while matches!(self.peek().kind, TokenKind::Plus | TokenKind::Minus) {
            let op_token = self.advance();
            let op = match op_token.kind {
                TokenKind::Plus  => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => unreachable!(),
            };
            let right = self.parse_postfix()?;
            let span = left.span().to(&right.span());
            left = Expr::Binary(BinaryExpr::new(op, Box::new(left), Box::new(right), span));
        }
        Ok(left)
    }

    pub(crate) fn parse_collection_slice(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        self.advance(); // consume $[

        let mut start = None;
        let mut end = None;

        // Check if starts with .. (e.g., $[..end])
        if !matches!(self.peek().kind, TokenKind::DotDot) {
            start = Some(Box::new(self.parse_slice_bound()?));
        }

        // Case: $[start:count] — count-based slice (alternative syntax)
        if start.is_some() && matches!(self.peek().kind, TokenKind::Colon) {
            self.advance(); // consume :
            let count = self.parse_slice_bound()?;
            let close_token = self.peek().clone();
            if !matches!(close_token.kind, TokenKind::RBracket) {
                return Err(Diagnostic::error("expected ']' after count")
                    .with_span(close_token.span)
                    .with_help("count-based slice syntax: $[start:count]"));
            }
            self.advance(); // consume ]
            let span = start_span.to(&close_token.span);
            return Ok(Expr::CollectionSlice(CollectionSliceExpr::new_count(
                Box::new(collection),
                start,
                Some(Box::new(count)),
                span,
            )));
        }

        // Must have ..
        if !matches!(self.peek().kind, TokenKind::DotDot) {
            return Err(Diagnostic::error("expected '..', or ':' in slice")
                .with_span(self.peek().span)
                .with_help("slice syntax: $[start..end], $[..end], $[start..], or $[start:count]"));
        }
        self.advance(); // consume ..

        // Check if ends immediately (e.g., $[start..])
        if !matches!(self.peek().kind, TokenKind::RBracket) {
            end = Some(Box::new(self.parse_slice_bound()?));
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

        let lambda = self.parse_lambda_or_ident()?; // Parse lambda or function reference
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

        let lambda = self.parse_lambda_or_ident()?; // Parse lambda or function reference
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

        let lambda = self.parse_lambda_or_ident()?; // Parse lambda or function reference

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

    /// Parse natural-order sort: collection$^+ (ascending) or collection$^- (descending).
    /// No comparator — direction is encoded in the token.
    pub(crate) fn parse_collection_sort(&mut self, collection: Expr, ascending: bool) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        let op_token = self.advance(); // consume $^+ or $^-
        let span = start_span.to(&op_token.span);
        let sort_expr = CollectionSortExpr::new(Box::new(collection), ascending, None, span);
        if ascending {
            Ok(Expr::CollectionSortAsc(sort_expr))
        } else {
            Ok(Expr::CollectionSortDesc(sort_expr))
        }
    }

    /// Parse custom-comparator sort: collection$^ (a, b -> expr).
    /// The lambda fully encodes the ordering — no direction sign needed.
    pub(crate) fn parse_collection_sort_custom(&mut self, collection: Expr) -> Result<Expr, Diagnostic> {
        let start_span = collection.span();
        self.advance(); // consume $^

        // Comparator lambda is required for $^
        if !matches!(self.peek().kind, TokenKind::LParen) {
            return Err(Diagnostic::error("expected comparator lambda after '$^', e.g. $^ (a, b -> a < b)")
                .with_span(self.peek().span));
        }
        let lambda = self.parse_lambda()?;
        let span = start_span.to(&lambda.span());
        let sort_expr = CollectionSortExpr::new(
            Box::new(collection),
            true, // ascending field unused for custom sort
            Some(Box::new(lambda)),
            span,
        );
        Ok(Expr::CollectionSortCustom(sort_expr))
    }
}
