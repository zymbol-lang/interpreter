//! Loop AST nodes for Zymbol-Lang (GRUPO 6: LOOPS)
//!
//! Contains AST structures for loops:
//! - Universal loop: @ [condition] { }
//! - For-each loop: @ var:iterable { }
//! - Loop control: BREAK (@!), CONTINUE (@>)
//! - Labeled loops: @ @label { }

use zymbol_span::Span;
use crate::{Block, Expr};

// ===== LOOP STRUCTURES (GRUPO 6: LOOPS) =====

/// Loop statement: @ condition { } or @ var:iterable { }
#[derive(Debug, Clone)]
pub struct Loop {
    pub condition: Option<Box<Expr>>, // None for infinite loop, Some for while loop
    pub iterator_var: Option<String>,  // For for-each loops: @ i:range { }
    /// `@ (k, v):pares { … }` — a destructuring pattern where a single name
    /// would go, binding each element the way `(k, v) = par` binds one.
    ///
    /// It is the same pattern language as the assignment, deliberately: the loop
    /// stops needing a first line whose only job is to unpack. That line was
    /// already in `forma/tuplas.zy` § 7 for every walk over an array of tuples,
    /// and it is what made `@ k:d` name the dictionary twice.
    ///
    /// Mutually exclusive with `iterator_var`.
    pub iterator_pattern: Option<crate::DestructurePattern>,
    pub iterable: Option<Box<Expr>>,   // For for-each loops: the range/array to iterate
    pub body: Block,
    pub label: Option<String>, // Optional label for break/continue
    pub span: Span,
}

/// Break statement: @! [label]
#[derive(Debug, Clone)]
pub struct Break {
    pub label: Option<String>,
    pub span: Span,
}

/// Continue statement: @> [label]
#[derive(Debug, Clone)]
pub struct Continue {
    pub label: Option<String>,
    pub span: Span,
}

// ===== LOOP IMPLEMENTATIONS =====

impl Loop {
    /// Create a while/infinite loop: @ condition { } or @ { }
    pub fn new(condition: Option<Box<Expr>>, body: Block, label: Option<String>, span: Span) -> Self {
        Self {
            condition,
            iterator_var: None,
            iterator_pattern: None,
            iterable: None,
            body,
            label,
            span,
        }
    }

    /// Create a for-each loop: @ var:iterable { }
    pub fn for_each(
        iterator_var: String,
        iterable: Box<Expr>,
        body: Block,
        label: Option<String>,
        span: Span,
    ) -> Self {
        Self {
            condition: None,
            iterator_var: Some(iterator_var),
            iterator_pattern: None,
            iterable: Some(iterable),
            body,
            label,
            span,
        }
    }

    /// Create a destructuring for-each loop: `@ (k, v):iterable { }`
    pub fn for_each_pattern(
        pattern: crate::DestructurePattern,
        iterable: Box<Expr>,
        body: Block,
        label: Option<String>,
        span: Span,
    ) -> Self {
        Self {
            condition: None,
            iterator_var: None,
            iterator_pattern: Some(pattern),
            iterable: Some(iterable),
            body,
            label,
            span,
        }
    }
}

impl Break {
    pub fn new(label: Option<String>, span: Span) -> Self {
        Break { label, span }
    }
}

impl Continue {
    pub fn new(label: Option<String>, span: Span) -> Self {
        Continue { label, span }
    }
}

/// Sleep statement: @~ N (milliseconds)
///
/// Unlike `@!` and `@>`, this carries no loop requirement: it pauses execution
/// without acting on any loop's control flow, and every engine has always run it
/// at top level. See `zymbol-semantic::check_loop_context`.
#[derive(Debug, Clone)]
pub struct Sleep {
    pub duration: Box<Expr>,
    pub span: Span,
}

impl Sleep {
    pub fn new(duration: Box<Expr>, span: Span) -> Self { Self { duration, span } }
}
