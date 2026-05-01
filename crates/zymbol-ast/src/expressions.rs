//! Expression AST nodes for Zymbol-Lang
//!
//! Contains AST structures for all expression types:
//! - Binary expressions (arithmetic, comparison, logical)
//! - Unary expressions (negation, logical NOT)
//! - Pipe expressions (function composition)

use zymbol_common::{BinaryOp, UnaryOp};
use zymbol_span::Span;
use crate::Expr;

/// Binary expression: left op right
#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub left: Box<Expr>,
    pub right: Box<Expr>,
    pub span: Span,
}

/// Unary expression: op operand
#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub operand: Box<Expr>,
    pub span: Span,
}

/// Pipe argument: either placeholder _ or expression
#[derive(Debug, Clone)]
pub enum PipeArg {
    Placeholder,       // _ will be replaced with piped value
    Expr(Expr),        // Regular expression argument
}

/// Pipe expression: value |> func(_, args) or value |> lambda(_)
#[derive(Debug, Clone)]
pub struct PipeExpr {
    pub left: Box<Expr>,
    pub callable: Box<Expr>,
    pub arguments: Vec<PipeArg>,
    /// True when the user wrote `|> f` with no explicit argument list.
    /// The parser inserts a Placeholder in `arguments`, but the formatter
    /// must not emit `(_)` — it should reconstruct the no-args form (§2.1).
    pub implicit: bool,
    pub span: Span,
}

// Implementations

impl BinaryExpr {
    pub fn new(op: BinaryOp, left: Box<Expr>, right: Box<Expr>, span: Span) -> Self {
        Self { op, left, right, span }
    }
}

impl UnaryExpr {
    pub fn new(op: UnaryOp, operand: Box<Expr>, span: Span) -> Self {
        Self { op, operand, span }
    }
}
