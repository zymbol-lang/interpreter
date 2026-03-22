//! Literal AST nodes for Zymbol-Lang
//!
//! Contains the AST representation for literal expressions.

use zymbol_common::Literal;
use zymbol_span::Span;

/// Literal expression
#[derive(Debug, Clone)]
pub struct LiteralExpr {
    pub value: Literal,
    pub span: Span,
}

impl LiteralExpr {
    pub fn new(value: Literal, span: Span) -> Self {
        Self { value, span }
    }

    pub fn string(s: String, span: Span) -> Self {
        Self {
            value: Literal::String(s),
            span,
        }
    }
}
