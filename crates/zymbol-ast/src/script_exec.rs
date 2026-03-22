//! Script execution AST nodes for Zymbol-Lang
//!
//! Contains AST structures for script execution:
//! - Execute expressions: </ path.zy /> (runs Zymbol scripts)
//! - Bash exec expressions: <\ command \> (runs shell commands with interpolation)

use zymbol_span::Span;

/// Execute expression: </ path.zy />
#[derive(Debug, Clone)]
pub struct ExecuteExpr {
    pub path: String,  // Path to .zy file to execute
    pub span: Span,
}

/// Bash execute expression: <\ command \>
/// Supports variable interpolation with {variable} syntax
#[derive(Debug, Clone)]
pub struct BashExecExpr {
    pub parts: Vec<String>,      // Literal parts of the command
    pub variables: Vec<String>,  // Variable names to interpolate
    pub span: Span,
}
