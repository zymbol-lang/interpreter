//! Variable and constant AST nodes for Zymbol-Lang
//!
//! Contains AST structures for:
//! - Assignment: name = expr
//! - Constant declaration: name := expr (immutable)
//! - Lifetime end: \variable (explicit destruction)

use zymbol_span::Span;
use crate::Expr;

/// Assignment statement: name = expr
#[derive(Debug, Clone)]
pub struct Assignment {
    pub name: String,
    pub value: Expr,
    pub span: Span,
}

/// Constant declaration: name := expr (immutable)
#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub name: String,
    pub value: Expr,
    pub span: Span,
}

/// Lifetime end: \variable (explicit variable destruction)
#[derive(Debug, Clone)]
pub struct LifetimeEnd {
    pub variable_name: String,
    pub span: Span,
}

impl Assignment {
    pub fn new(name: String, value: Expr, span: Span) -> Self {
        Self { name, value, span }
    }
}

impl ConstDecl {
    pub fn new(name: String, value: Expr, span: Span) -> Self {
        Self { name, value, span }
    }
}

impl LifetimeEnd {
    pub fn new(variable_name: String, span: Span) -> Self {
        Self { variable_name, span }
    }
}
