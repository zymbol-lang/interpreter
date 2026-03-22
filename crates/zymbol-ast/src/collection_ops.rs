//! Collection operation AST nodes for Zymbol-Lang
//!
//! Handles AST structures for all collection operators:
//! - $# (length/size)
//! - $+ (append element)
//! - $- (remove by index)
//! - $? (contains/search)
//! - $~ (update element)
//! - $[ (slice with range)
//! - $> (map - transform collection)
//! - $| (filter - select elements)
//! - $< (reduce - accumulate)

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

/// Collection remove expression: collection$- index
#[derive(Debug, Clone)]
pub struct CollectionRemoveExpr {
    pub collection: Box<Expr>,
    pub index: Box<Expr>,
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

/// Collection slice expression: collection$[start..end]
#[derive(Debug, Clone)]
pub struct CollectionSliceExpr {
    pub collection: Box<Expr>,
    pub start: Option<Box<Expr>>,  // None for $[..end]
    pub end: Option<Box<Expr>>,    // None for $[start..]
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

impl CollectionRemoveExpr {
    pub fn new(collection: Box<Expr>, index: Box<Expr>, span: Span) -> Self {
        Self { collection, index, span }
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
        Self { collection, start, end, span }
    }
}
