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

/// Type conversion (and validation) applied to the raw string after reading input.
///
/// The typed variants (`Float`/`Decimal`/`Int`/`Text`/`Char`) come from the input
/// typespec syntax `<< <typespec> "prompt" var`, where `<typespec>` reuses the cast
/// symbols: `##.` Float, `###` Int, `##"` String, `##'` Char, with an optional size
/// constraint in parentheses. They re-prompt until the input is valid.
#[derive(Debug, Clone, PartialEq)]
pub enum InputCast {
    /// Store as raw `String` (default): `<< var`.
    String,
    /// Apply `#|...|` numeric eval: parse to `Int` or `Float`, fall back to `String`.
    Numeric,
    /// `##.` — any valid floating-point number → `Float`.
    Float,
    /// `##.(total, decimals)` — decimal with at most `total` digits overall and at
    /// most `decimals` fractional digits → `Float`.
    Decimal { total: u32, decimals: u32 },
    /// `###` (max_digits None) or `###(N)` — integer with at most N digits → `Int`.
    Int { max_digits: Option<u32> },
    /// `##"` (max None) or `##"(N)` — string of at most N characters → `String`.
    Text { max: Option<u32> },
    /// `##'` — exactly one character → `Char`.
    Char,
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
    /// `true` when written as `\\`, `false` when written as `¶`
    pub backslash: bool,
}

/// Clear screen statement: >>!
#[derive(Debug, Clone)]
pub struct ClearScreen {
    pub span: Span,
}

/// Key input statement: <<| var (blocking) or <<|? var (non-blocking), and
/// `<<| var, mods` — `mods` receives the modifiers held, as an Int (D-14).
#[derive(Debug, Clone)]
pub struct KeyInput {
    pub variable: String,
    /// The second variable of `<<| k, m`: Shift = 1, Alt = 2, Ctrl = 4.
    pub modifiers: Option<String>,
    pub blocking: bool,
    pub span: Span,
}

/// Positioned output: >>~ (fila, col, BKS, fg, bg) > items
/// Sparse inline syntax: >>~(,,,15,0)> — None = slot absent
#[derive(Debug, Clone)]
pub struct OutputPos {
    pub slots: Vec<Option<crate::Expr>>,  // [fila, col, BKS, fg, bg] — up to 5, None = absent
    pub items: Vec<crate::Expr>,
    /// `false` for the bare-variable form `>>~ pos > items` (no parens)
    pub parenthesized: bool,
    /// `>>~ (row, col)` with no `>`: nothing is printed, the terminal's cursor
    /// is shown there — or hidden, by `(0, 0)` (D-16). `items` is empty.
    pub caret: bool,
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
    pub fn new(span: Span) -> Self { Self { span, backslash: false } }

    pub fn new_backslash(span: Span) -> Self { Self { span, backslash: true } }
}

impl ClearScreen {
    pub fn new(span: Span) -> Self { Self { span } }
}

impl KeyInput {
    pub fn new(variable: String, blocking: bool, span: Span) -> Self {
        Self { variable, modifiers: None, blocking, span }
    }

    pub fn with_modifiers(variable: String, modifiers: String, blocking: bool, span: Span) -> Self {
        Self { variable, modifiers: Some(modifiers), blocking, span }
    }

    /// The variables it assigns, in order.
    pub fn variables(&self) -> impl Iterator<Item = &String> {
        std::iter::once(&self.variable).chain(self.modifiers.iter())
    }
}

impl OutputPos {
    pub fn new(slots: Vec<Option<crate::Expr>>, items: Vec<crate::Expr>, span: Span) -> Self {
        Self { slots, items, parenthesized: true, caret: false, span }
    }

    pub fn new_bare(slots: Vec<Option<crate::Expr>>, items: Vec<crate::Expr>, span: Span) -> Self {
        Self { slots, items, parenthesized: false, caret: false, span }
    }

    /// `>>~ (row, col)` or `>>~ pos`, with nothing to print: the cursor (D-16).
    pub fn cursor(slots: Vec<Option<crate::Expr>>, parenthesized: bool, span: Span) -> Self {
        Self { slots, items: Vec::new(), parenthesized, caret: true, span }
    }
}

impl TuiBlock {
    pub fn new(body: crate::Block, span: Span) -> Self { Self { body, span } }
}
