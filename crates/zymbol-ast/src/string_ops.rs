//! String operation AST nodes for Zymbol-Lang
//!
//! Handles AST structures for all string operators:
//! - $?? (find all positions of pattern in string)
//! - $++ (insert text at position)
//! - $-- (remove text by count)
//! - $~~ (replace pattern with replacement text)

use zymbol_span::Span;
use crate::Expr;

/// String find positions expression: string$?? pattern
/// Returns an array of integer positions where the pattern is found
#[derive(Debug, Clone)]
pub struct StringFindPositionsExpr {
    pub string: Box<Expr>,
    pub pattern: Box<Expr>,  // String or Char to search for
    pub span: Span,
}

/// String insert expression: string$++[position:text]
/// Inserts text at the specified position
#[derive(Debug, Clone)]
pub struct StringInsertExpr {
    pub string: Box<Expr>,
    pub position: Box<Expr>,  // Integer position
    pub text: Box<Expr>,      // String to insert
    pub span: Span,
}

/// String remove expression: string$--[position:count]
/// Removes count characters starting at position
#[derive(Debug, Clone)]
pub struct StringRemoveExpr {
    pub string: Box<Expr>,
    pub position: Box<Expr>,  // Integer position
    pub count: Box<Expr>,     // Integer count of characters to remove
    pub span: Span,
}

// Implementations

impl StringFindPositionsExpr {
    pub fn new(string: Box<Expr>, pattern: Box<Expr>, span: Span) -> Self {
        Self { string, pattern, span }
    }
}

impl StringInsertExpr {
    pub fn new(string: Box<Expr>, position: Box<Expr>, text: Box<Expr>, span: Span) -> Self {
        Self { string, position, text, span }
    }
}

impl StringRemoveExpr {
    pub fn new(string: Box<Expr>, position: Box<Expr>, count: Box<Expr>, span: Span) -> Self {
        Self { string, position, count, span }
    }
}

/// String replace expression: string$~~[pattern:replacement] or string$~~[pattern:replacement:count]
/// Replaces pattern with replacement text
/// - If count is not provided or is 0, replaces all occurrences
/// - If count is N, replaces first N occurrences
#[derive(Debug, Clone)]
pub struct StringReplaceExpr {
    pub string: Box<Expr>,
    pub pattern: Box<Expr>,      // String or Char to search for
    pub replacement: Box<Expr>,   // String to replace with
    pub count: Option<Box<Expr>>, // Optional count (None or 0 = all, N = first N)
    pub span: Span,
}

impl StringReplaceExpr {
    pub fn new(
        string: Box<Expr>,
        pattern: Box<Expr>,
        replacement: Box<Expr>,
        count: Option<Box<Expr>>,
        span: Span,
    ) -> Self {
        Self { string, pattern, replacement, count, span }
    }
}
