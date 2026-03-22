//! Common types and utilities for Zymbol-Lang compiler
//!
//! This crate provides shared types used across all compiler crates:
//! - Symbol interning for efficient identifier storage
//! - Literal types (Int, Float, String, Char, Bool)
//! - Operator types (Binary, Unary, Collection)

use indexmap::IndexMap;
use std::fmt;

/// Symbol is an interned string represented as a 32-bit integer
/// This allows for fast comparison and reduces memory usage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol(pub u32);

impl Symbol {
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Symbol({})", self.0)
    }
}

/// String interner for efficient string storage and comparison
/// Strings are stored once and referred to by Symbol IDs
#[derive(Debug, Default)]
pub struct Interner {
    map: IndexMap<String, Symbol>,
    strings: Vec<String>,
}

impl Interner {
    pub fn new() -> Self {
        Self {
            map: IndexMap::new(),
            strings: Vec::new(),
        }
    }

    /// Intern a string and return its Symbol
    /// If the string already exists, returns the existing Symbol
    pub fn intern(&mut self, s: &str) -> Symbol {
        if let Some(&symbol) = self.map.get(s) {
            return symbol;
        }

        let symbol = Symbol(self.strings.len() as u32);
        self.strings.push(s.to_string());
        self.map.insert(s.to_string(), symbol);
        symbol
    }

    /// Get the string for a Symbol
    pub fn resolve(&self, symbol: Symbol) -> Option<&str> {
        self.strings.get(symbol.0 as usize).map(|s| s.as_str())
    }

    /// Get the string for a Symbol, panicking if it doesn't exist
    pub fn resolve_unchecked(&self, symbol: Symbol) -> &str {
        &self.strings[symbol.0 as usize]
    }

    /// Number of interned strings
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

/// Literal value types in Zymbol
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Int(n) => write!(f, "{}", n),
            Literal::Float(n) => write!(f, "{}", n),
            Literal::String(s) => write!(f, "\"{}\"", s),
            Literal::Char(c) => write!(f, "'{}'", c),
            Literal::Bool(b) => write!(f, "{}", if *b { "#1" } else { "#0" }),
        }
    }
}

/// Binary operators in Zymbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    // Arithmetic operators
    Add,  // +
    Sub,  // -
    Mul,  // *
    Div,  // /
    Mod,  // %
    Pow,  // ^ (power/exponentiation)

    // Comparison operators
    Eq,  // ==
    Neq, // <>
    Lt,  // <
    Gt,  // >
    Le,  // <=
    Ge,  // >=

    // Logical operators
    And, // &&
    Or,  // ||

    // Special operators
    Pipe,  // |>
    Comma, // , (concatenation)
    Range, // ..
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Mod => "%",
            BinaryOp::Pow => "^",
            BinaryOp::Eq => "==",
            BinaryOp::Neq => "<>",
            BinaryOp::Lt => "<",
            BinaryOp::Gt => ">",
            BinaryOp::Le => "<=",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
            BinaryOp::Pipe => "|>",
            BinaryOp::Comma => ",",
            BinaryOp::Range => "..",
        };
        write!(f, "{}", s)
    }
}

impl BinaryOp {
    /// Check if this is an arithmetic operator
    pub fn is_arithmetic(&self) -> bool {
        matches!(
            self,
            BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Div
                | BinaryOp::Mod
                | BinaryOp::Pow
        )
    }

    /// Check if this is a comparison operator
    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            BinaryOp::Eq
                | BinaryOp::Neq
                | BinaryOp::Lt
                | BinaryOp::Gt
                | BinaryOp::Le
                | BinaryOp::Ge
        )
    }

    /// Check if this is a logical operator
    pub fn is_logical(&self) -> bool {
        matches!(self, BinaryOp::And | BinaryOp::Or)
    }
}

/// Unary operators in Zymbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Neg, // - (negation)
    Not, // ! (logical not)
    Pos, // + (explicit positive)
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "!",
            UnaryOp::Pos => "+",
        };
        write!(f, "{}", s)
    }
}

/// Collection operators in Zymbol (all use $ prefix)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CollectionOp {
    Append, // $+ (append element)
    Delete, // $- (delete by index)
    Search, // $? (search/find element)
    Length, // $# (length/size)
    Update, // $~ (update by index)
    Slice,  // $[..] (slice)
}

impl fmt::Display for CollectionOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CollectionOp::Append => "$+",
            CollectionOp::Delete => "$-",
            CollectionOp::Search => "$?",
            CollectionOp::Length => "$#",
            CollectionOp::Update => "$~",
            CollectionOp::Slice => "$[..]",
        };
        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol() {
        let sym = Symbol(42);
        assert_eq!(sym.as_u32(), 42);
    }

    #[test]
    fn test_interner() {
        let mut interner = Interner::new();

        let hello1 = interner.intern("hello");
        let hello2 = interner.intern("hello");
        let world = interner.intern("world");

        // Same string should give same symbol
        assert_eq!(hello1, hello2);
        assert_ne!(hello1, world);

        // Resolve should work
        assert_eq!(interner.resolve(hello1), Some("hello"));
        assert_eq!(interner.resolve(world), Some("world"));

        // Only 2 unique strings
        assert_eq!(interner.len(), 2);
    }

    #[test]
    fn test_interner_unicode() {
        let mut interner = Interner::new();

        let emoji = interner.intern("😀");
        let spanish = interner.intern("año");

        assert_eq!(interner.resolve(emoji), Some("😀"));
        assert_eq!(interner.resolve(spanish), Some("año"));
    }

    #[test]
    fn test_literal_display() {
        assert_eq!(Literal::Int(42).to_string(), "42");
        assert_eq!(Literal::String("hello".to_string()).to_string(), "\"hello\"");
        assert_eq!(Literal::Bool(true).to_string(), "#1");
        assert_eq!(Literal::Bool(false).to_string(), "#0");
    }

    #[test]
    fn test_binary_op_display() {
        // Arithmetic
        assert_eq!(BinaryOp::Add.to_string(), "+");
        assert_eq!(BinaryOp::Sub.to_string(), "-");
        assert_eq!(BinaryOp::Mul.to_string(), "*");
        assert_eq!(BinaryOp::Div.to_string(), "/");
        assert_eq!(BinaryOp::Mod.to_string(), "%");
        assert_eq!(BinaryOp::Pow.to_string(), "^");

        // Comparison
        assert_eq!(BinaryOp::Eq.to_string(), "==");
        assert_eq!(BinaryOp::Neq.to_string(), "<>");
        assert_eq!(BinaryOp::Lt.to_string(), "<");
        assert_eq!(BinaryOp::Gt.to_string(), ">");
        assert_eq!(BinaryOp::Le.to_string(), "<=");
        assert_eq!(BinaryOp::Ge.to_string(), ">=");

        // Logical
        assert_eq!(BinaryOp::And.to_string(), "&&");
        assert_eq!(BinaryOp::Or.to_string(), "||");

        // Special
        assert_eq!(BinaryOp::Pipe.to_string(), "|>");
        assert_eq!(BinaryOp::Comma.to_string(), ",");
        assert_eq!(BinaryOp::Range.to_string(), "..");
    }

    #[test]
    fn test_binary_op_categories() {
        // Arithmetic operators
        assert!(BinaryOp::Add.is_arithmetic());
        assert!(BinaryOp::Pow.is_arithmetic());
        assert!(!BinaryOp::Eq.is_arithmetic());
        assert!(!BinaryOp::And.is_arithmetic());

        // Comparison operators
        assert!(BinaryOp::Eq.is_comparison());
        assert!(BinaryOp::Lt.is_comparison());
        assert!(!BinaryOp::Add.is_comparison());
        assert!(!BinaryOp::And.is_comparison());

        // Logical operators
        assert!(BinaryOp::And.is_logical());
        assert!(BinaryOp::Or.is_logical());
        assert!(!BinaryOp::Add.is_logical());
        assert!(!BinaryOp::Eq.is_logical());
    }

    #[test]
    fn test_unary_op_display() {
        assert_eq!(UnaryOp::Neg.to_string(), "-");
        assert_eq!(UnaryOp::Not.to_string(), "!");
        assert_eq!(UnaryOp::Pos.to_string(), "+");
    }

    #[test]
    fn test_collection_op_display() {
        assert_eq!(CollectionOp::Append.to_string(), "$+");
        assert_eq!(CollectionOp::Delete.to_string(), "$-");
        assert_eq!(CollectionOp::Search.to_string(), "$?");
        assert_eq!(CollectionOp::Length.to_string(), "$#");
        assert_eq!(CollectionOp::Update.to_string(), "$~");
        assert_eq!(CollectionOp::Slice.to_string(), "$[..]");
    }
}
