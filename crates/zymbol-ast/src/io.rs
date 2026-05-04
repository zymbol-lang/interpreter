//! IO AST nodes for Zymbol-Lang
//!
//! Contains AST structures for IO statements:
//! - Output: >> expr1 expr2 ...
//! - Input: << variable (with optional prompt)
//! - Newline: ¶ OR \\

use zymbol_span::Span;
use crate::Expr;

/// Output statement: >> expr1 expr2 expr3 ...
#[derive(Debug, Clone)]
pub struct Output {
    pub exprs: Vec<Expr>,  // Multiple expressions without commas (Haskell-style)
    pub span: Span,
}

/// Type conversion applied to the raw string after reading input.
#[derive(Debug, Clone, PartialEq)]
pub enum InputCast {
    /// Store as raw `String` (default).
    String,
    /// Apply `#|...|` numeric eval: parse to `Int` or `Float`, fall back to `String`.
    Numeric,
}

/// Input statement: << variable  OR  << #|variable|
#[derive(Debug, Clone)]
pub struct Input {
    pub variable: String,
    pub prompt: Option<InputPrompt>, // Optional prompt (simple or interpolated)
    pub cast: InputCast,             // Type conversion applied after reading
    pub span: Span,
}

/// Prompt for input statement
#[derive(Debug, Clone)]
pub enum InputPrompt {
    /// Simple string prompt
    Simple(String),
    /// Interpolated string prompt with {var}
    Interpolated(Vec<zymbol_lexer::StringPart>),
}

/// Newline statement: ¶ or \\
#[derive(Debug, Clone)]
pub struct Newline {
    pub span: Span,
}

/// Clear screen statement: >>!
#[derive(Debug, Clone)]
pub struct ClearScreen {
    pub span: Span,
}

/// Key input statement: <<| var (blocking) or <<|? var (non-blocking)
#[derive(Debug, Clone)]
pub struct KeyInput {
    pub variable: String,
    pub blocking: bool,
    pub span: Span,
}

/// Positioned output: >>~ (row, col [, fg [, bg]]) > items
#[derive(Debug, Clone)]
pub struct OutputPos {
    pub pos: Box<crate::Expr>,
    pub items: Vec<crate::Expr>,
    pub span: Span,
}

/// TUI block: >>| { } — alternate screen + raw mode scope
#[derive(Debug, Clone)]
pub struct TuiBlock {
    pub body: crate::Block,
    pub span: Span,
}

// Implementations

impl Output {
    pub fn new(exprs: Vec<Expr>, span: Span) -> Self {
        Self { exprs, span }
    }
}

impl Input {
    pub fn new(variable: String, prompt: Option<InputPrompt>, cast: InputCast, span: Span) -> Self {
        Self { variable, prompt, cast, span }
    }
}

impl Newline {
    pub fn new(span: Span) -> Self { Self { span } }
}

impl ClearScreen {
    pub fn new(span: Span) -> Self { Self { span } }
}

impl KeyInput {
    pub fn new(variable: String, blocking: bool, span: Span) -> Self {
        Self { variable, blocking, span }
    }
}

impl OutputPos {
    pub fn new(pos: Box<crate::Expr>, items: Vec<crate::Expr>, span: Span) -> Self {
        Self { pos, items, span }
    }
}

impl TuiBlock {
    pub fn new(body: crate::Block, span: Span) -> Self { Self { body, span } }
}
