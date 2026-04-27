//! Multi-dimensional indexing and restructuring AST nodes for Zymbol-Lang
//!
//! Handles:
//! - Deep scalar access:       arr[i>j>k]
//! - Flat extraction:          arr[p ; q] or arr[[i>j]]
//! - Structured extraction:    arr[[group] ; [group]]

use zymbol_span::Span;
use crate::Expr;

/// One step in a navigation path: an index atom with an optional inclusive range end.
///
/// Examples:
/// - `2`      → NavStep { index: 2, range_end: None }
/// - `2..4`   → NavStep { index: 2, range_end: Some(4) }
#[derive(Debug, Clone)]
pub struct NavStep {
    pub index: Box<Expr>,
    pub range_end: Option<Box<Expr>>,
}

/// A sequence of navigation steps separated by `>`.
///
/// Example: `1>2>3` → NavPath { steps: [step(1), step(2), step(3)] }
#[derive(Debug, Clone)]
pub struct NavPath {
    pub steps: Vec<NavStep>,
}

/// A group of comma-separated paths inside `[...]` in structured extraction.
///
/// Example: `[1>1, 1>3]` → ExtractGroup { paths: [NavPath{1,1}, NavPath{1,3}] }
#[derive(Debug, Clone)]
pub struct ExtractGroup {
    pub paths: Vec<NavPath>,
}

/// Deep scalar access: `arr[i>j>k]` — returns the single value at the given depth.
///
/// All steps must be plain atoms (no ranges). Use `FlatExtractExpr` when ranges are needed.
#[derive(Debug, Clone)]
pub struct DeepIndexExpr {
    pub array: Box<Expr>,
    pub path: NavPath,
    pub span: Span,
}

/// Flat extraction: `arr[p ; q ; r]` or `arr[[i>j]]` — returns a flat `Array` of values.
///
/// Both forms produce the same node:
/// - `arr[i>j ; k>l]`  — multiple top-level paths
/// - `arr[[i>j]]`      — single path wrapped in double brackets (returns `[value]`)
/// - `arr[[1>2..3]]`   — single path with range (returns `[v1, v2]`)
#[derive(Debug, Clone)]
pub struct FlatExtractExpr {
    pub array: Box<Expr>,
    pub paths: Vec<NavPath>,
    pub span: Span,
}

/// Structured extraction: `arr[[g] ; [g]]` — returns an `Array` of `Array`s.
///
/// Each `ExtractGroup` becomes one sub-array in the result.
#[derive(Debug, Clone)]
pub struct StructuredExtractExpr {
    pub array: Box<Expr>,
    pub groups: Vec<ExtractGroup>,
    pub span: Span,
}
