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

/// Input statement: << variable
#[derive(Debug, Clone)]
pub struct Input {
    pub variable: String,
    pub prompt: Option<InputPrompt>, // Optional prompt (simple or interpolated)
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

// Implementations

impl Output {
    pub fn new(exprs: Vec<Expr>, span: Span) -> Self {
        Self { exprs, span }
    }
}

impl Input {
    pub fn new(variable: String, prompt: Option<InputPrompt>, span: Span) -> Self {
        Self {
            variable,
            prompt,
            span,
        }
    }

    /// Helper to create Input with simple string prompt
    pub fn with_simple_prompt(variable: String, prompt: String, span: Span) -> Self {
        Self::new(variable, Some(InputPrompt::Simple(prompt)), span)
    }

    /// Helper to create Input with interpolated prompt
    pub fn with_interpolated_prompt(variable: String, parts: Vec<zymbol_lexer::StringPart>, span: Span) -> Self {
        Self::new(variable, Some(InputPrompt::Interpolated(parts)), span)
    }
}

impl Newline {
    pub fn new(span: Span) -> Self {
        Self { span }
    }
}
