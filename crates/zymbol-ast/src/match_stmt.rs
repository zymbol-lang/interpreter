//! MATCH expression and pattern matching AST nodes for Zymbol-Lang
//!
//! Contains AST structures for pattern matching:
//! - MATCH expression: ?? expr { cases }
//! - Pattern types: literals, ranges, lists, wildcards, comparisons, identifiers

use zymbol_common::{BinaryOp, Literal};
use zymbol_span::Span;
use crate::{Block, Expr};

/// Match expression: ?? expr { cases }
#[derive(Debug, Clone)]
pub struct MatchExpr {
    pub scrutinee: Box<Expr>,
    pub cases: Vec<MatchCase>,
    pub span: Span,
}

/// Single match case: pattern : [ value ] [ block ]
/// Either value or block must be present (or both)
#[derive(Debug, Clone)]
pub struct MatchCase {
    pub pattern: Pattern,
    pub value: Option<Expr>,
    pub block: Option<Block>,
    pub span: Span,
}

/// Pattern types for match expressions
#[derive(Debug, Clone)]
pub enum Pattern {
    /// Literal pattern: 5, "hello", #1
    Literal(Literal, Span),
    /// Range pattern: 1..10, 'a'..'z'
    Range(Box<Expr>, Box<Expr>, Span),
    /// List pattern: [1, 2, 3] — structural if scrutinee is array; containment if scalar
    List(Vec<Pattern>, Span),
    /// Wildcard pattern: _
    Wildcard(Span),
    /// Comparison pattern: < 0, >= 100, == "x" — implicit scrutinee
    Comparison(BinaryOp, Box<Expr>, Span),
    /// Identifier pattern: variable — scalar equality or array containment at runtime
    Ident(String, Span),
}

impl MatchExpr {
    pub fn new(scrutinee: Box<Expr>, cases: Vec<MatchCase>, span: Span) -> Self {
        Self {
            scrutinee,
            cases,
            span,
        }
    }
}

impl MatchCase {
    pub fn new(pattern: Pattern, value: Option<Expr>, block: Option<Block>, span: Span) -> Self {
        Self {
            pattern,
            value,
            block,
            span,
        }
    }
}

impl Pattern {
    pub fn literal(literal: Literal, span: Span) -> Self {
        Pattern::Literal(literal, span)
    }

    pub fn range(start: Box<Expr>, end: Box<Expr>, span: Span) -> Self {
        Pattern::Range(start, end, span)
    }

    pub fn list(patterns: Vec<Pattern>, span: Span) -> Self {
        Pattern::List(patterns, span)
    }

    pub fn wildcard(span: Span) -> Self {
        Pattern::Wildcard(span)
    }

    pub fn comparison(op: BinaryOp, expr: Box<Expr>, span: Span) -> Self {
        Pattern::Comparison(op, expr, span)
    }

    pub fn ident(name: String, span: Span) -> Self {
        Pattern::Ident(name, span)
    }

    /// Get the span of a pattern
    pub fn span(&self) -> Span {
        match self {
            Pattern::Literal(_, span) => *span,
            Pattern::Range(_, _, span) => *span,
            Pattern::List(_, span) => *span,
            Pattern::Wildcard(span) => *span,
            Pattern::Comparison(_, _, span) => *span,
            Pattern::Ident(_, span) => *span,
        }
    }
}
