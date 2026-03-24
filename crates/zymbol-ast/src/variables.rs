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

// ── Destructuring assignment ─────────────────────────────────────────────────

/// A single item in an array or positional-tuple destructure pattern
#[derive(Debug, Clone)]
pub enum DestructureItem {
    /// `name` — bind element to variable
    Bind(String),
    /// `*name` — collect remaining elements into a new array
    Rest(String),
    /// `_` — discard element
    Ignore,
}

/// The overall pattern on the left-hand side of a destructure assignment
#[derive(Debug, Clone)]
pub enum DestructurePattern {
    /// `[a, b, *rest]` — array destructuring
    Array(Vec<DestructureItem>),
    /// `(a, b, c)` — positional tuple destructuring
    Positional(Vec<DestructureItem>),
    /// `(field: var, ...)` — named tuple destructuring
    NamedTuple(Vec<(String, String)>),
}

/// Destructure assignment: `[a, b] = expr` / `(name: n, age: a) = expr`
#[derive(Debug, Clone)]
pub struct DestructureAssign {
    pub pattern: DestructurePattern,
    pub value: Box<Expr>,
    pub span: Span,
}

impl DestructureAssign {
    pub fn new(pattern: DestructurePattern, value: Expr, span: Span) -> Self {
        Self { pattern, value: Box::new(value), span }
    }
}
