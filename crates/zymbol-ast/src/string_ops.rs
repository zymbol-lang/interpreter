//! String operation AST nodes for Zymbol-Lang
//!
//! Handles AST structures for string-specific operators:
//! - $~~ (replace pattern with replacement text)
//!
//! Note: $?? (find all), $+[i] (insert), $-[i..j] (remove range) are handled
//! by the unified collection_ops structs (CollectionFindAllExpr, CollectionInsertExpr,
//! CollectionRemoveRangeExpr) since they apply to arrays, tuples, and strings alike.

use zymbol_span::Span;
use crate::Expr;

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
