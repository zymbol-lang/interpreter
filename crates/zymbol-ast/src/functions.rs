//! Function-related AST nodes for Zymbol-Lang
//!
//! Contains AST structures for functions:
//! - Function declarations
//! - Lambda expressions
//! - Return statements
//! - Parameters (normal, mutable, output)

use zymbol_span::Span;
use crate::{Block, Expr};

/// Lambda expression: x -> expr or (a, b) -> { block }
#[derive(Debug, Clone)]
pub struct LambdaExpr {
    pub params: Vec<String>,  // Parameter names
    pub body: LambdaBody,      // Expression or block
    pub span: Span,
}

/// Lambda body - either simple expression or block
#[derive(Debug, Clone)]
pub enum LambdaBody {
    /// Simple lambda: x -> x * 2 (implicit return)
    Expr(Box<Expr>),
    /// Block lambda: x -> { <~ x * 2 } (explicit return required)
    Block(Block),
}

/// Function declaration: name(params) { }
#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub body: Block,
    pub span: Span,
}

/// Return statement: <~ expr
#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub value: Option<Box<Expr>>,
    pub span: Span,
}

/// Function parameter with modifiers
#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub kind: ParameterKind,
    pub span: Span,
}

/// Parameter kind (normal, mutable ~, output <~)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterKind {
    /// Normal parameter (immutable, pass-by-value)
    Normal,
    /// Mutable parameter ~ (pass-by-reference)
    Mutable,
    /// Output parameter <~ (creates/modifies in caller scope)
    Output,
}

// Implementations

impl FunctionDecl {
    pub fn new(name: String, parameters: Vec<Parameter>, body: Block, span: Span) -> Self {
        Self {
            name,
            parameters,
            body,
            span,
        }
    }
}

impl ReturnStmt {
    pub fn new(value: Option<Box<Expr>>, span: Span) -> Self {
        Self { value, span }
    }
}

impl Parameter {
    pub fn new(name: String, kind: ParameterKind, span: Span) -> Self {
        Self { name, kind, span }
    }
}
