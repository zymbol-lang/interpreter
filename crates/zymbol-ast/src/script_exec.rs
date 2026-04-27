//! Script execution AST nodes for Zymbol-Lang
//!
//! Contains AST structures for script execution:
//! - Execute expressions: </ path.zy /> (runs Zymbol scripts)
//! - Bash exec expressions: <\ expr1 expr2 ... \> (runs shell commands)

use zymbol_span::Span;
use crate::Expr;

/// Execute expression: </ path.zy />
#[derive(Debug, Clone)]
pub struct ExecuteExpr {
    pub path: String,  // Path to .zy file to execute
    pub span: Span,
}

/// Bash execute expression: <\ expr1 expr2 ... \>
/// Content is normal Zymbol expressions evaluated and concatenated to form the shell command.
/// Bare identifiers are variable references; string/char literals are literal.
/// No implicit separator — use char literals for spaces: <\ "ls" ' ' dir \>
#[derive(Debug, Clone)]
pub struct BashExecExpr {
    pub args: Vec<Expr>,  // Expressions to evaluate and concatenate
    pub span: Span,
}
