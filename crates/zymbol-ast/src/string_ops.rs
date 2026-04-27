//! String operation AST nodes for Zymbol-Lang
//!
//! Handles AST structures for string-specific operators:
//! - $~~ (replace pattern with replacement text)
//! - $/ (split string by delimiter)
//! - $++ (concat-build: append multiple items into a string or array)
//!
//! Note: $?? (find all), $+[i] (insert), $-[i..j] (remove range) are handled
//! by the unified collection_ops structs (CollectionFindAllExpr, CollectionInsertExpr,
//! CollectionRemoveRangeExpr) since they apply to arrays, tuples, and strings alike.

use zymbol_span::Span;
use crate::Expr;

/// String split expression: string$/ delimiter → Array(String)
/// Splits a string by a char or string delimiter.
#[derive(Debug, Clone)]
pub struct StringSplitExpr {
    pub string: Box<Expr>,
    pub delimiter: Box<Expr>, // Char or String
    pub span: Span,
}

impl StringSplitExpr {
    pub fn new(string: Box<Expr>, delimiter: Box<Expr>, span: Span) -> Self {
        Self { string, delimiter, span }
    }
}

/// Concat-build expression: base$++ item1 item2 item3 ...
/// If base is String → string concatenation of all items.
/// If base is Array  → array append of all items.
#[derive(Debug, Clone)]
pub struct ConcatBuildExpr {
    pub base: Box<Expr>,
    pub items: Vec<Expr>,
    pub span: Span,
}

impl ConcatBuildExpr {
    pub fn new(base: Box<Expr>, items: Vec<Expr>, span: Span) -> Self {
        Self { base, items, span }
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
