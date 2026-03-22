//! Collection AST nodes for Zymbol-Lang
//!
//! Contains AST structures for collections:
//! - Array literals: [expr1, expr2, ...]
//! - Tuples: (expr1, expr2, ...)
//! - Named tuples: (name: value, name2: value2)

use zymbol_span::Span;
use crate::Expr;

/// Array literal expression: [expr1, expr2, ...]
#[derive(Debug, Clone)]
pub struct ArrayLiteralExpr {
    pub elements: Vec<Expr>,
    pub span: Span,
}

/// Tuple expression: (expr1, expr2, ...) - positional, requires at least 2 elements
#[derive(Debug, Clone)]
pub struct TupleExpr {
    pub elements: Vec<Expr>,
    pub span: Span,
}

/// Named tuple expression: (name: expr, name2: expr2, ...)
/// Allows 1 or more named fields
#[derive(Debug, Clone)]
pub struct NamedTupleExpr {
    pub fields: Vec<(String, Expr)>,  // (field_name, value)
    pub span: Span,
}

// Implementations

impl ArrayLiteralExpr {
    pub fn new(elements: Vec<Expr>, span: Span) -> Self {
        Self { elements, span }
    }
}

impl TupleExpr {
    pub fn new(elements: Vec<Expr>, span: Span) -> Self {
        Self { elements, span }
    }
}

impl NamedTupleExpr {
    pub fn new(fields: Vec<(String, Expr)>, span: Span) -> Self {
        Self { fields, span }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Expr, IdentifierExpr, LiteralExpr, MemberAccessExpr};
    use zymbol_common::Literal;
    use zymbol_span::{FileId, Position, Span};

    fn dummy_span() -> Span {
        Span {
            start: Position {
                line: 1,
                column: 1,
                byte_offset: 0,
            },
            end: Position {
                line: 1,
                column: 1,
                byte_offset: 0,
            },
            file_id: FileId(0),
        }
    }

    // ========== Tuple Tests ==========

    #[test]
    fn test_tuple_creation() {
        let expr1 = Expr::Literal(LiteralExpr::new(Literal::Int(10), dummy_span()));
        let expr2 = Expr::Literal(LiteralExpr::new(Literal::Int(20), dummy_span()));
        let tuple = TupleExpr::new(vec![expr1, expr2], dummy_span());

        assert_eq!(tuple.elements.len(), 2);
    }

    #[test]
    fn test_tuple_with_multiple_types() {
        let expr1 = Expr::Literal(LiteralExpr::new(Literal::String("Alice".to_string()), dummy_span()));
        let expr2 = Expr::Literal(LiteralExpr::new(Literal::Int(25), dummy_span()));
        let expr3 = Expr::Literal(LiteralExpr::new(Literal::Bool(true), dummy_span()));
        let tuple = TupleExpr::new(vec![expr1, expr2, expr3], dummy_span());

        assert_eq!(tuple.elements.len(), 3);
    }

    #[test]
    fn test_tuple_expr_variant() {
        let expr1 = Expr::Literal(LiteralExpr::new(Literal::Int(1), dummy_span()));
        let expr2 = Expr::Literal(LiteralExpr::new(Literal::Int(2), dummy_span()));
        let tuple_expr = Expr::Tuple(TupleExpr::new(vec![expr1, expr2], dummy_span()));

        match tuple_expr {
            Expr::Tuple(t) => assert_eq!(t.elements.len(), 2),
            _ => panic!("Expected Tuple variant"),
        }
    }

    #[test]
    fn test_tuple_span() {
        let expr1 = Expr::Literal(LiteralExpr::new(Literal::Int(1), dummy_span()));
        let expr2 = Expr::Literal(LiteralExpr::new(Literal::Int(2), dummy_span()));
        let span = dummy_span();
        let tuple_expr = Expr::Tuple(TupleExpr::new(vec![expr1, expr2], span));

        assert_eq!(tuple_expr.span(), span);
    }

    // ========== Named Tuple Tests ==========

    #[test]
    fn test_named_tuple_creation() {
        let fields = vec![
            ("id".to_string(), Expr::Literal(LiteralExpr::new(Literal::Int(101), dummy_span()))),
            ("name".to_string(), Expr::Literal(LiteralExpr::new(Literal::String("Alice".to_string()), dummy_span()))),
        ];
        let named_tuple = NamedTupleExpr::new(fields.clone(), dummy_span());

        assert_eq!(named_tuple.fields.len(), 2);
        assert_eq!(named_tuple.fields[0].0, "id");
        assert_eq!(named_tuple.fields[1].0, "name");
    }

    #[test]
    fn test_named_tuple_single_field() {
        let fields = vec![
            ("value".to_string(), Expr::Literal(LiteralExpr::new(Literal::Int(42), dummy_span()))),
        ];
        let named_tuple = NamedTupleExpr::new(fields, dummy_span());

        assert_eq!(named_tuple.fields.len(), 1);
        assert_eq!(named_tuple.fields[0].0, "value");
    }

    #[test]
    fn test_named_tuple_expr_variant() {
        let fields = vec![
            ("x".to_string(), Expr::Literal(LiteralExpr::new(Literal::Int(10), dummy_span()))),
            ("y".to_string(), Expr::Literal(LiteralExpr::new(Literal::Int(20), dummy_span()))),
        ];
        let named_tuple_expr = Expr::NamedTuple(NamedTupleExpr::new(fields, dummy_span()));

        match named_tuple_expr {
            Expr::NamedTuple(nt) => assert_eq!(nt.fields.len(), 2),
            _ => panic!("Expected NamedTuple variant"),
        }
    }

    #[test]
    fn test_named_tuple_span() {
        let fields = vec![
            ("a".to_string(), Expr::Literal(LiteralExpr::new(Literal::Int(1), dummy_span()))),
        ];
        let span = dummy_span();
        let named_tuple_expr = Expr::NamedTuple(NamedTupleExpr::new(fields, span));

        assert_eq!(named_tuple_expr.span(), span);
    }

    // ========== Member Access Tests ==========

    #[test]
    fn test_member_access_creation() {
        let object = Box::new(Expr::Identifier(IdentifierExpr::new("person".to_string(), dummy_span())));
        let member_access = MemberAccessExpr::new(object, "name".to_string(), dummy_span());

        assert_eq!(member_access.field, "name");
    }

    #[test]
    fn test_member_access_expr_variant() {
        let object = Box::new(Expr::Identifier(IdentifierExpr::new("obj".to_string(), dummy_span())));
        let member_expr = Expr::MemberAccess(MemberAccessExpr::new(object, "field".to_string(), dummy_span()));

        match member_expr {
            Expr::MemberAccess(ma) => assert_eq!(ma.field, "field"),
            _ => panic!("Expected MemberAccess variant"),
        }
    }

    #[test]
    fn test_member_access_on_named_tuple() {
        let fields = vec![
            ("id".to_string(), Expr::Literal(LiteralExpr::new(Literal::Int(1), dummy_span()))),
        ];
        let named_tuple = Box::new(Expr::NamedTuple(NamedTupleExpr::new(fields, dummy_span())));
        let member_access = MemberAccessExpr::new(named_tuple, "id".to_string(), dummy_span());

        assert_eq!(member_access.field, "id");
    }

    #[test]
    fn test_member_access_span() {
        let object = Box::new(Expr::Identifier(IdentifierExpr::new("obj".to_string(), dummy_span())));
        let span = dummy_span();
        let member_expr = Expr::MemberAccess(MemberAccessExpr::new(object, "field".to_string(), span));

        assert_eq!(member_expr.span(), span);
    }

    #[test]
    fn test_chained_member_access() {
        let obj = Box::new(Expr::Identifier(IdentifierExpr::new("obj".to_string(), dummy_span())));
        let first_access = Box::new(Expr::MemberAccess(MemberAccessExpr::new(obj, "field1".to_string(), dummy_span())));
        let chained_access = MemberAccessExpr::new(first_access, "field2".to_string(), dummy_span());

        assert_eq!(chained_access.field, "field2");
    }
}
