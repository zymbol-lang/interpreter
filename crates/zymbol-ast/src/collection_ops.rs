//! Collection operation AST nodes for Zymbol-Lang
//!
//! Handles AST structures for all collection operators:
//! - $# (length/size)
//! - $+ (append element by value)
//! - $+[i] (insert element at position)
//! - $- (remove first occurrence of value)
//! - $-- (remove all occurrences of value)
//! - $-[i] (remove element at index)
//! - $-[i..j] (remove range of elements)
//! - $? (contains/search by value)
//! - $?? (find all indices of value)
//! - $~ (update element at index)
//! - $[ (slice with range)
//! - $> (map - transform collection)
//! - $| (filter - select elements)
//! - $< (reduce - accumulate)
//! - $^+ (sort ascending)
//! - $^- (sort descending)

use zymbol_span::Span;
use crate::Expr;

/// Collection length expression: collection$#
#[derive(Debug, Clone)]
pub struct CollectionLengthExpr {
    pub collection: Box<Expr>,
    pub span: Span,
}

/// Collection append expression: collection$+ element
#[derive(Debug, Clone)]
pub struct CollectionAppendExpr {
    pub collection: Box<Expr>,
    pub element: Box<Expr>,
    pub span: Span,
}

/// Collection insert expression: collection$+[index] element
/// Inserts element at the specified position (arrays, tuples, strings)
#[derive(Debug, Clone)]
pub struct CollectionInsertExpr {
    pub collection: Box<Expr>,
    pub index: Box<Expr>,
    pub element: Box<Expr>,
    pub span: Span,
}

/// Collection remove value expression: collection$- value
/// Removes the first occurrence of value (by value, not by index)
#[derive(Debug, Clone)]
pub struct CollectionRemoveValueExpr {
    pub collection: Box<Expr>,
    pub value: Box<Expr>,
    pub span: Span,
}

/// Collection remove all expression: collection$-- value
/// Removes all occurrences of value
#[derive(Debug, Clone)]
pub struct CollectionRemoveAllExpr {
    pub collection: Box<Expr>,
    pub value: Box<Expr>,
    pub span: Span,
}

/// Collection remove at expression: collection$-[index]
/// Removes element at the specified index (arrays, tuples, strings)
#[derive(Debug, Clone)]
pub struct CollectionRemoveAtExpr {
    pub collection: Box<Expr>,
    pub index: Box<Expr>,
    pub span: Span,
}

/// Collection remove range expression: collection$-[start..end] or collection$-[start:count]
/// Removes elements in the specified range (arrays, tuples, strings).
/// When `count_based=true`, `end` holds the count; actual end = start + count.
#[derive(Debug, Clone)]
pub struct CollectionRemoveRangeExpr {
    pub collection: Box<Expr>,
    pub start: Option<Box<Expr>>,
    pub end: Option<Box<Expr>>,
    pub count_based: bool,
    pub span: Span,
}

/// Collection find all expression: collection$?? value
/// Returns an array of indices where value is found (arrays, tuples, strings)
#[derive(Debug, Clone)]
pub struct CollectionFindAllExpr {
    pub collection: Box<Expr>,
    pub value: Box<Expr>,
    pub span: Span,
}

/// Collection contains expression: collection$? element
#[derive(Debug, Clone)]
pub struct CollectionContainsExpr {
    pub collection: Box<Expr>,
    pub element: Box<Expr>,
    pub span: Span,
}

/// Collection update expression: collection[index]$~ value
#[derive(Debug, Clone)]
pub struct CollectionUpdateExpr {
    pub target: Box<Expr>,  // IndexExpr: collection[index]
    pub value: Box<Expr>,
    pub span: Span,
}

/// Collection slice expression: collection$[start..end] or collection$[start:count]
/// When `count_based=true`, `end` holds the count; actual end = start + count.
#[derive(Debug, Clone)]
pub struct CollectionSliceExpr {
    pub collection: Box<Expr>,
    pub start: Option<Box<Expr>>,  // None for $[..end]
    pub end: Option<Box<Expr>>,    // None for $[start..]
    pub count_based: bool,
    pub span: Span,
}

/// Collection map expression: collection$> (x -> x * 2)
#[derive(Debug, Clone)]
pub struct CollectionMapExpr {
    pub collection: Box<Expr>,
    pub lambda: Box<Expr>,  // Must evaluate to lambda
    pub span: Span,
}

/// Collection filter expression: collection$| (x -> x > 0)
#[derive(Debug, Clone)]
pub struct CollectionFilterExpr {
    pub collection: Box<Expr>,
    pub lambda: Box<Expr>,  // Must evaluate to lambda
    pub span: Span,
}

/// Collection reduce expression: collection$< (0, (acc, x) -> acc + x)
#[derive(Debug, Clone)]
pub struct CollectionReduceExpr {
    pub collection: Box<Expr>,
    pub initial: Box<Expr>,
    pub lambda: Box<Expr>,  // Must evaluate to lambda with 2 params
    pub span: Span,
}

// Implementations

impl CollectionLengthExpr {
    pub fn new(collection: Box<Expr>, span: Span) -> Self {
        Self { collection, span }
    }
}

impl CollectionAppendExpr {
    pub fn new(collection: Box<Expr>, element: Box<Expr>, span: Span) -> Self {
        Self { collection, element, span }
    }
}

impl CollectionInsertExpr {
    pub fn new(collection: Box<Expr>, index: Box<Expr>, element: Box<Expr>, span: Span) -> Self {
        Self { collection, index, element, span }
    }
}

impl CollectionRemoveValueExpr {
    pub fn new(collection: Box<Expr>, value: Box<Expr>, span: Span) -> Self {
        Self { collection, value, span }
    }
}

impl CollectionRemoveAllExpr {
    pub fn new(collection: Box<Expr>, value: Box<Expr>, span: Span) -> Self {
        Self { collection, value, span }
    }
}

impl CollectionRemoveAtExpr {
    pub fn new(collection: Box<Expr>, index: Box<Expr>, span: Span) -> Self {
        Self { collection, index, span }
    }
}

impl CollectionRemoveRangeExpr {
    pub fn new(collection: Box<Expr>, start: Option<Box<Expr>>, end: Option<Box<Expr>>, span: Span) -> Self {
        Self { collection, start, end, count_based: false, span }
    }
    pub fn new_count(collection: Box<Expr>, start: Option<Box<Expr>>, count: Option<Box<Expr>>, span: Span) -> Self {
        Self { collection, start, end: count, count_based: true, span }
    }
}

impl CollectionFindAllExpr {
    pub fn new(collection: Box<Expr>, value: Box<Expr>, span: Span) -> Self {
        Self { collection, value, span }
    }
}

impl CollectionContainsExpr {
    pub fn new(collection: Box<Expr>, element: Box<Expr>, span: Span) -> Self {
        Self { collection, element, span }
    }
}

impl CollectionUpdateExpr {
    pub fn new(target: Box<Expr>, value: Box<Expr>, span: Span) -> Self {
        Self { target, value, span }
    }
}

impl CollectionSliceExpr {
    pub fn new(collection: Box<Expr>, start: Option<Box<Expr>>, end: Option<Box<Expr>>, span: Span) -> Self {
        Self { collection, start, end, count_based: false, span }
    }
    pub fn new_count(collection: Box<Expr>, start: Option<Box<Expr>>, count: Option<Box<Expr>>, span: Span) -> Self {
        Self { collection, start, end: count, count_based: true, span }
    }
}

/// Collection sort expression: collection$^+ or collection$^-
/// `ascending=true`  → $^+  natural order or custom comparator
/// `ascending=false` → $^-  reverse order or custom comparator
/// `comparator=None` → natural order (numbers, strings)
/// `comparator=Some` → two-argument lambda: (a, b) -> Bool
#[derive(Debug, Clone)]
pub struct CollectionSortExpr {
    pub collection: Box<Expr>,
    pub ascending: bool,
    pub comparator: Option<Box<Expr>>,
    pub span: Span,
}

impl CollectionSortExpr {
    pub fn new(collection: Box<Expr>, ascending: bool, comparator: Option<Box<Expr>>, span: Span) -> Self {
        Self { collection, ascending, comparator, span }
    }
}

