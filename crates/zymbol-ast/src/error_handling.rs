//! Error handling AST nodes for Zymbol-Lang
//!
//! Contains AST structures for error handling:
//! - TRY: !? { }
//! - CATCH: :! { } or :! ##Type { }
//! - FINALLY: :> { }
//! - ERROR CHECK: expr$!
//! - ERROR PROPAGATE: expr$!!

use zymbol_span::Span;
use crate::{Block, Expr};

/// Try statement: !? { } :! { } :> { }
///
/// Example:
/// ```zymbol
/// !?{
///     data = read_file("config.txt")
/// } :! ##IO {
///     >> "File error" ¶
/// } :! {
///     >> "Error: " + _err ¶
/// } :>{
///     cleanup()
/// }
/// ```
#[derive(Debug, Clone)]
pub struct TryStmt {
    pub try_block: Block,
    pub catch_clauses: Vec<CatchClause>,
    pub finally_clause: Option<FinallyClause>,
    pub span: Span,
}

/// Catch clause: :! { } or :! ##Type { }
///
/// - Generic catch: :! { } catches any error
/// - Typed catch: :! ##IO { } catches specific error type
///
/// Built-in _err variable is available in the catch block
#[derive(Debug, Clone)]
pub struct CatchClause {
    pub error_type: Option<ErrorType>,
    pub block: Block,
    pub span: Span,
}

/// Error type for typed catch: ##IO, ##Network, ##Parse, etc.
#[derive(Debug, Clone)]
pub struct ErrorType {
    pub name: String,  // "IO", "Network", "Parse", "Index", "Type", "Div", "_"
    pub span: Span,
}

/// Finally clause: :> { }
///
/// Always executes regardless of whether an error occurred
#[derive(Debug, Clone)]
pub struct FinallyClause {
    pub block: Block,
    pub span: Span,
}

/// Error check expression: expr$!
///
/// Returns #1 if the expression is an error, #0 otherwise
///
/// Example:
/// ```zymbol
/// ? result$! {
///     >> "Operation failed" ¶
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ErrorCheckExpr {
    pub expr: Box<Expr>,
    pub span: Span,
}

/// Error propagate expression: expr$!!
///
/// Propagates the error to the caller if the expression is an error.
/// Used for early return on error.
///
/// Example:
/// ```zymbol
/// process_file(path) {
///     content = read(path)
///     ? content$! { content$!! }  // propagate if error
///     <~ transform(content)
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ErrorPropagateExpr {
    pub expr: Box<Expr>,
    pub span: Span,
}

impl TryStmt {
    pub fn new(
        try_block: Block,
        catch_clauses: Vec<CatchClause>,
        finally_clause: Option<FinallyClause>,
        span: Span,
    ) -> Self {
        Self {
            try_block,
            catch_clauses,
            finally_clause,
            span,
        }
    }
}

impl CatchClause {
    pub fn new(error_type: Option<ErrorType>, block: Block, span: Span) -> Self {
        Self {
            error_type,
            block,
            span,
        }
    }

    /// Create a generic catch clause (catches any error)
    pub fn generic(block: Block, span: Span) -> Self {
        Self {
            error_type: None,
            block,
            span,
        }
    }

    /// Create a typed catch clause (catches specific error type)
    pub fn typed(error_type: ErrorType, block: Block, span: Span) -> Self {
        Self {
            error_type: Some(error_type),
            block,
            span,
        }
    }
}

impl ErrorType {
    pub fn new(name: String, span: Span) -> Self {
        Self { name, span }
    }
}

impl FinallyClause {
    pub fn new(block: Block, span: Span) -> Self {
        Self { block, span }
    }
}

impl ErrorCheckExpr {
    pub fn new(expr: Box<Expr>, span: Span) -> Self {
        Self { expr, span }
    }
}

impl ErrorPropagateExpr {
    pub fn new(expr: Box<Expr>, span: Span) -> Self {
        Self { expr, span }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_span::{FileId, Position};
    use crate::LiteralExpr;

    fn dummy_span() -> Span {
        Span::new(Position::start(), Position::start(), FileId(0))
    }

    #[test]
    fn test_try_stmt_creation() {
        let try_block = Block::new(vec![], dummy_span());
        let catch_block = Block::new(vec![], dummy_span());
        let catch_clause = CatchClause::generic(catch_block, dummy_span());

        let try_stmt = TryStmt::new(
            try_block,
            vec![catch_clause],
            None,
            dummy_span(),
        );

        assert_eq!(try_stmt.catch_clauses.len(), 1);
        assert!(try_stmt.finally_clause.is_none());
    }

    #[test]
    fn test_typed_catch_clause() {
        let block = Block::new(vec![], dummy_span());
        let error_type = ErrorType::new("IO".to_string(), dummy_span());
        let catch = CatchClause::typed(error_type, block, dummy_span());

        assert!(catch.error_type.is_some());
        assert_eq!(catch.error_type.unwrap().name, "IO");
    }

    #[test]
    fn test_finally_clause() {
        let block = Block::new(vec![], dummy_span());
        let finally = FinallyClause::new(block, dummy_span());

        assert_eq!(finally.block.statements.len(), 0);
    }

    #[test]
    fn test_error_check_expr() {
        let inner = Box::new(Expr::Literal(LiteralExpr::new(
            zymbol_common::Literal::Int(42),
            dummy_span(),
        )));
        let check = ErrorCheckExpr::new(inner, dummy_span());

        match check.expr.as_ref() {
            Expr::Literal(lit) => {
                assert!(matches!(lit.value, zymbol_common::Literal::Int(42)));
            }
            _ => panic!("Expected literal"),
        }
    }

    #[test]
    fn test_error_propagate_expr() {
        let inner = Box::new(Expr::Literal(LiteralExpr::new(
            zymbol_common::Literal::Int(42),
            dummy_span(),
        )));
        let propagate = ErrorPropagateExpr::new(inner, dummy_span());

        match propagate.expr.as_ref() {
            Expr::Literal(lit) => {
                assert!(matches!(lit.value, zymbol_common::Literal::Int(42)));
            }
            _ => panic!("Expected literal"),
        }
    }

    #[test]
    fn test_try_with_multiple_catches_and_finally() {
        let try_block = Block::new(vec![], dummy_span());

        // Typed catch for IO
        let io_catch = CatchClause::typed(
            ErrorType::new("IO".to_string(), dummy_span()),
            Block::new(vec![], dummy_span()),
            dummy_span(),
        );

        // Typed catch for Network
        let network_catch = CatchClause::typed(
            ErrorType::new("Network".to_string(), dummy_span()),
            Block::new(vec![], dummy_span()),
            dummy_span(),
        );

        // Generic catch
        let generic_catch = CatchClause::generic(
            Block::new(vec![], dummy_span()),
            dummy_span(),
        );

        // Finally
        let finally = FinallyClause::new(Block::new(vec![], dummy_span()), dummy_span());

        let try_stmt = TryStmt::new(
            try_block,
            vec![io_catch, network_catch, generic_catch],
            Some(finally),
            dummy_span(),
        );

        assert_eq!(try_stmt.catch_clauses.len(), 3);
        assert!(try_stmt.finally_clause.is_some());

        // Verify catch types
        assert!(try_stmt.catch_clauses[0].error_type.is_some());
        assert_eq!(try_stmt.catch_clauses[0].error_type.as_ref().unwrap().name, "IO");
        assert!(try_stmt.catch_clauses[1].error_type.is_some());
        assert_eq!(try_stmt.catch_clauses[1].error_type.as_ref().unwrap().name, "Network");
        assert!(try_stmt.catch_clauses[2].error_type.is_none()); // Generic
    }
}
