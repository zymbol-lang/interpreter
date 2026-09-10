//! Variable and constant AST nodes for Zymbol-Lang
//!
//! Contains AST structures for:
//! - Assignment: name = expr
//! - Constant declaration: name := expr (immutable)
//! - Lifetime end: \variable (explicit destruction)

use zymbol_common::BinaryOp;
use zymbol_span::Span;
use crate::Expr;

/// Surface syntax the parser desugared into a plain assignment.
///
/// `value` always holds the desugared expression (e.g. `x + 1` for `x += 1`),
/// so the interpreter, compiler and analyses never need to look at this field.
/// The formatter uses it to reprint exactly what the user wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssignSugar {
    /// Plain `name = expr`
    #[default]
    None,
    /// Compound assignment `name op= expr` (e.g. `+=`, `*=`)
    Compound(BinaryOp),
    /// `name++`
    Increment,
    /// `name--`
    Decrement,
    /// Indexed assignment `name[i] = expr` (desugared to CollectionUpdate)
    IndexedAssign,
    /// Indexed compound assignment `name[i] op= expr`
    IndexedCompound(BinaryOp),
    /// A bare `$` edit statement: `arr$+ 3`, `arr[2]$~ 99`, `arr$-[1]`, `d["k"]$~ v`.
    ///
    /// The parser desugars these into `name = <the same $ expression>`, which is
    /// observably identical because Zymbol assigns collections by value and has
    /// no aliasing (`DI-04`) — nobody else holds the old one. What the marker
    /// carries is the *source form*, and two things need it: the formatter, to
    /// reprint what was written, and the tuple guard, because `t$+ 3` and
    /// `t[1]$~ 99` written as statements mean "modify this tuple" and a tuple
    /// does not change, while `u = t$+ 3` derives a second one and is fine.
    ///
    /// This is decision 12 of `Divergente_ES/forma/README.md`, the rule of the
    /// result: a `$` whose result is *used* builds, a `$` that *is* the whole
    /// statement modifies in place. Before it, a bare `$+` statement did nothing
    /// at all, without a warning (`DI-01`), and `arr[i]$~ v` as a statement did
    /// not even parse.
    InPlaceEdit,
}

/// Assignment statement: name = expr
#[derive(Debug, Clone)]
pub struct Assignment {
    pub name: String,
    pub value: Expr,
    pub span: Span,
    /// `x° op= n` — anchor to nearest enclosing `@` scope
    pub hot: bool,
    /// `°x op= n` — anchor to the scope above the nearest enclosing `@`
    pub pre_hot: bool,
    /// Surface form this assignment was written in (`+=`, `++`, …)
    pub sugar: AssignSugar,
    /// The edit exactly as it was WRITTEN, when `value` is a rewritten form.
    ///
    /// A statement-level edit whose receiver lives inside the name — `d.x$+ 3`,
    /// `d.x["y"]$~ 5` — is executed as a deep write at that path, because that
    /// is the only shape that can be assigned back to `d` without replacing it.
    /// That is not what the author wrote, and FORMATTER_RULES §2.1 says the
    /// surface reprints as written; §12 names this remedy — record the form the
    /// AST would otherwise lose. `None` everywhere else, which is every
    /// assignment that was not rewritten.
    pub written: Option<Box<Expr>>,
}

/// Constant declaration: name := expr (immutable)
#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub name: String,
    pub value: Expr,
    pub span: Span,
}

/// Lifetime end: \variable (explicit variable destruction)
#[derive(Debug, Clone)]
pub struct LifetimeEnd {
    pub variable_name: String,
    pub span: Span,
}

impl Assignment {
    pub fn new(name: String, value: Expr, span: Span) -> Self {
        Self { name, value, span, hot: false, pre_hot: false, sugar: AssignSugar::None, written: None }
    }

    pub fn new_hot(name: String, value: Expr, span: Span) -> Self {
        Self { name, value, span, hot: true, pre_hot: false, sugar: AssignSugar::None, written: None }
    }

    pub fn new_pre_hot(name: String, value: Expr, span: Span) -> Self {
        Self { name, value, span, hot: false, pre_hot: true, sugar: AssignSugar::None, written: None }
    }
}

impl ConstDecl {
    pub fn new(name: String, value: Expr, span: Span) -> Self {
        Self { name, value, span }
    }
}

impl LifetimeEnd {
    pub fn new(variable_name: String, span: Span) -> Self {
        Self { variable_name, span }
    }
}

// ── Destructuring assignment ─────────────────────────────────────────────────

/// A single item in an array or positional-tuple destructure pattern
#[derive(Debug, Clone)]
pub enum DestructureItem {
    /// `name` — bind element to variable
    Bind(String),
    /// `*name` — collect remaining elements into a new array
    Rest(String),
    /// `_` — discard element
    Ignore,
}

/// The overall pattern on the left-hand side of a destructure assignment
#[derive(Debug, Clone)]
pub enum DestructurePattern {
    /// `[a, b, *rest]` — array destructuring
    Array(Vec<DestructureItem>),
    /// `(a, b, c)` — positional tuple destructuring
    Positional(Vec<DestructureItem>),
    /// `(field: var, ...)` — named tuple destructuring
    NamedTuple(Vec<(String, String)>),
}

impl DestructurePattern {
    /// The names this pattern binds, in order — what a loop head has to declare
    /// so `@ (k, v):pares` is not "undefined variable 'k'".
    pub fn bound_names(&self) -> Vec<String> {
        match self {
            DestructurePattern::Array(items) | DestructurePattern::Positional(items) => items
                .iter()
                .filter_map(|i| match i {
                    DestructureItem::Bind(n) | DestructureItem::Rest(n) => Some(n.clone()),
                    DestructureItem::Ignore => None,
                })
                .collect(),
            DestructurePattern::NamedTuple(pairs) => {
                pairs.iter().map(|(_, v)| v.clone()).collect()
            }
        }
    }
}

/// Destructure assignment: `[a, b] = expr` / `(name: n, age: a) = expr`
#[derive(Debug, Clone)]
pub struct DestructureAssign {
    pub pattern: DestructurePattern,
    pub value: Box<Expr>,
    pub span: Span,
}

impl DestructureAssign {
    pub fn new(pattern: DestructurePattern, value: Expr, span: Span) -> Self {
        Self { pattern, value: Box::new(value), span }
    }
}
