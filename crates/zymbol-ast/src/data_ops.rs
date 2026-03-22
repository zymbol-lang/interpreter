//! Data operation AST nodes for Zymbol-Lang
//!
//! Contains AST structures for data transformation and introspection:
//! - Numeric evaluation: #|expr| (safe string-to-number conversion)
//! - Type metadata: expr#? (type introspection)
//! - Format expressions: e|expr| (scientific), c|expr| (comma-separated)
//! - Base conversions: 0x|expr|, 0b|expr|, 0o|expr|, 0d|expr| (char/int/text)
//! - Precision expressions: #.N|expr| (round), #!N|expr| (truncate)

use zymbol_span::Span;
use crate::Expr;

/// Numeric evaluation expression: #|expr|
/// Safe string-to-number conversion that never fails
/// - Detects: integers, floats, scientific notation
/// - Safe operation: never throws errors
#[derive(Debug, Clone)]
pub struct NumericEvalExpr {
    pub expr: Box<Expr>,
    pub span: Span,
}

/// Type metadata expression: expr#?
/// Returns a tuple with (type_symbol, count, value):
/// - type_symbol: Standardized with ## prefix (language-agnostic)
///   - ### = Int (triple hash - visual exception)
///   - ##. = Float, ##" = String, ##' = Char
///   - ##? = Bool, ##] = Array, ##) = Tuple, ##_ = Unit
/// - count: digit count for numbers, length for strings/arrays, element count for tuples
/// - value: the original value unchanged
#[derive(Debug, Clone)]
pub struct TypeMetadataExpr {
    pub expr: Box<Expr>,
    pub span: Span,
}

/// Format expression: e|expr| or c|expr|
#[derive(Debug, Clone)]
pub struct FormatExpr {
    pub prefix: FormatPrefix,
    pub expr: Box<Expr>,
    pub span: Span,
}

/// Format prefix for format expressions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatPrefix {
    /// e or E - scientific/exponential notation
    Scientific,
    /// c or C - comma-separated thousands
    Comma,
}

/// Base conversion expression: 0x|expr| or 0b|expr| or 0o|expr| or 0d|expr|
/// Tridirectional conversion: char→text, int→char, text→char
#[derive(Debug, Clone)]
pub struct BaseConversionExpr {
    pub prefix: BasePrefix,
    pub expr: Box<Expr>,
    pub span: Span,
}

/// Base prefix for base conversion expressions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasePrefix {
    /// 0b - binary (base 2)
    Binary,
    /// 0o - octal (base 8)
    Octal,
    /// 0d - decimal (base 10)
    Decimal,
    /// 0x - hexadecimal/Unicode (base 16, full Unicode range)
    Hex,
}

/// Round expression: #.N|expr|
/// Rounds the result of expr to N decimal places using standard mathematical rounding.
/// - #.2|19.876| → 19.88 (0.006 rounds up)
/// - #.2|19.874| → 19.87 (0.004 rounds down)
/// - #.0|19.5| → 20.0 (rounds to nearest integer)
///
/// Always returns Float type.
#[derive(Debug, Clone)]
pub struct RoundExpr {
    /// Number of decimal places to round to
    pub precision: u32,
    /// Expression to evaluate and round
    pub expr: Box<Expr>,
    pub span: Span,
}

/// Truncate expression: #!N|expr|
/// Truncates the result of expr to N decimal places (cuts without rounding).
/// - #!2|19.879| → 19.87 (simply cuts)
/// - #!0|19.9| → 19.0 (truncates to integer)
///
/// Always returns Float type.
#[derive(Debug, Clone)]
pub struct TruncExpr {
    /// Number of decimal places to truncate to
    pub precision: u32,
    /// Expression to evaluate and truncate
    pub expr: Box<Expr>,
    pub span: Span,
}

// Implementations

impl NumericEvalExpr {
    pub fn new(expr: Box<Expr>, span: Span) -> Self {
        Self { expr, span }
    }
}

impl TypeMetadataExpr {
    pub fn new(expr: Box<Expr>, span: Span) -> Self {
        Self { expr, span }
    }
}

impl FormatExpr {
    pub fn new(prefix: FormatPrefix, expr: Box<Expr>, span: Span) -> Self {
        Self { prefix, expr, span }
    }
}

impl BaseConversionExpr {
    pub fn new(prefix: BasePrefix, expr: Box<Expr>, span: Span) -> Self {
        Self { prefix, expr, span }
    }
}

impl RoundExpr {
    pub fn new(precision: u32, expr: Box<Expr>, span: Span) -> Self {
        Self { precision, expr, span }
    }
}

impl TruncExpr {
    pub fn new(precision: u32, expr: Box<Expr>, span: Span) -> Self {
        Self { precision, expr, span }
    }
}
