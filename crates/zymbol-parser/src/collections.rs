//! Collection parsing for Zymbol-Lang
//!
//! Handles parsing of collection expressions:
//! - Array literals: [expr1, expr2, ...]
//! - Tuples: (expr1, expr2, ...)
//! - Named tuples: (name: value, name2: value2)
//! - Grouped expressions: (expr)

use zymbol_ast::{ArrayLiteralExpr, Expr, NamedTupleExpr, TupleExpr};
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;
use crate::Parser;

impl Parser {
    /// Parse array literal: [1, 2, 3]
    pub(crate) fn parse_array_literal(&mut self) -> Result<Expr, Diagnostic> {
        let start_token = self.advance(); // consume [
        let mut elements = Vec::new();

        // Handle empty array []
        if matches!(self.peek().kind, TokenKind::RBracket) {
            let end_token = self.advance(); // consume ]
            let span = start_token.span.to(&end_token.span);
            return Ok(Expr::ArrayLiteral(ArrayLiteralExpr::new(elements, span)));
        }

        // Parse first element
        elements.push(self.parse_expr()?);

        // Parse remaining elements (comma-separated)
        while matches!(self.peek().kind, TokenKind::Comma) {
            self.advance(); // consume ,

            // Allow trailing comma
            if matches!(self.peek().kind, TokenKind::RBracket) {
                break;
            }

            elements.push(self.parse_expr()?);
        }

        // Expect closing ]
        let end_token = self.peek().clone();
        if !matches!(end_token.kind, TokenKind::RBracket) {
            return Err(Diagnostic::error("expected ']' to close array literal")
                .with_span(end_token.span)
                .with_help("array literals must be enclosed in brackets"));
        }
        self.advance(); // consume ]

        let span = start_token.span.to(&end_token.span);
        Ok(Expr::ArrayLiteral(ArrayLiteralExpr::new(elements, span)))
    }

    /// Parse tuple, named tuple, or grouped expression
    /// Handles: (expr), (expr, expr), (name: value, ...)
    pub(crate) fn parse_tuple_or_grouped(&mut self) -> Result<Expr, Diagnostic> {
        let lparen_token = self.advance(); // consume (

        // Check if it's a named tuple by looking ahead for "identifier :"
        let is_named_tuple = if let TokenKind::Ident(ref _name) = self.peek().kind {
            // Look ahead one more token to check for colon
            if let Some(next_token) = self.peek_ahead(1) {
                matches!(next_token.kind, TokenKind::Colon)
            } else {
                false
            }
        } else {
            false
        };

        if is_named_tuple {
            // Parse named tuple: (name: expr, name2: expr2, ...)
            let mut fields = Vec::new();

            loop {
                // Parse field name
                let field_token = self.peek().clone();
                let field_name = if let TokenKind::Ident(ref name) = field_token.kind {
                    name.clone()
                } else {
                    return Err(Diagnostic::error("expected field name in named tuple")
                        .with_span(field_token.span)
                        .with_help("named tuples require field names: (name: value, name2: value2)"));
                };
                self.advance(); // consume identifier

                // Expect colon
                if !matches!(self.peek().kind, TokenKind::Colon) {
                    return Err(Diagnostic::error("expected ':' after field name in named tuple")
                        .with_span(self.peek().span));
                }
                self.advance(); // consume :

                // Parse value expression
                let value_expr = self.parse_expr()?;
                fields.push((field_name, value_expr));

                // Check for comma (more fields) or closing paren
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.advance(); // consume ,

                    // Check for trailing comma before )
                    if matches!(self.peek().kind, TokenKind::RParen) {
                        break;
                    }
                } else {
                    break;
                }
            }

            // Expect closing )
            let rparen_token = self.peek().clone();
            if !matches!(rparen_token.kind, TokenKind::RParen) {
                return Err(Diagnostic::error("expected ')' to close named tuple")
                    .with_span(rparen_token.span)
                    .with_help("named tuples must be enclosed in parentheses: (name: value, ...)"));
            }
            let rparen_token = self.advance(); // consume )

            // Create named tuple with span from ( to )
            let span = lparen_token.span.to(&rparen_token.span);
            Ok(Expr::NamedTuple(NamedTupleExpr::new(fields, span)))
        } else {
            // Parse positional tuple or grouped expression
            let first_expr = self.parse_expr()?;

            // Check if it's a tuple (has comma) or just grouping
            if matches!(self.peek().kind, TokenKind::Comma) {
                // It's a tuple - parse remaining elements
                let mut elements = vec![first_expr];

                while matches!(self.peek().kind, TokenKind::Comma) {
                    self.advance(); // consume ,

                    // Check for trailing comma before )
                    if matches!(self.peek().kind, TokenKind::RParen) {
                        break;
                    }

                    elements.push(self.parse_expr()?);
                }

                // Expect closing )
                let rparen_token = self.peek().clone();
                if !matches!(rparen_token.kind, TokenKind::RParen) {
                    return Err(Diagnostic::error("expected ')' to close tuple")
                        .with_span(rparen_token.span)
                        .with_help("tuples must be enclosed in parentheses: (expr, expr, ...)"));
                }
                let rparen_token = self.advance(); // consume )

                // Create tuple with span from ( to )
                let span = lparen_token.span.to(&rparen_token.span);
                Ok(Expr::Tuple(TupleExpr::new(elements, span)))
            } else {
                // It's a grouped expression - just return the expression
                let rparen_token = self.peek().clone();
                if !matches!(rparen_token.kind, TokenKind::RParen) {
                    return Err(Diagnostic::error("expected ')' to close grouped expression")
                        .with_span(rparen_token.span)
                        .with_help("grouped expressions must be enclosed in parentheses: (expr)"));
                }
                self.advance(); // consume )

                Ok(first_expr)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use zymbol_ast::{Expr, Statement, Program};
    use zymbol_error::Diagnostic;
    use zymbol_lexer::Lexer;
    use zymbol_span::FileId;

    fn parse(source: &str) -> Result<Program, Vec<Diagnostic>> {
        let lexer = Lexer::new(source, FileId(0));
        let (tokens, lex_diagnostics) = lexer.tokenize();

        if !lex_diagnostics.is_empty() {
            return Err(lex_diagnostics);
        }

        let parser = crate::Parser::new(tokens);
        parser.parse()
    }

    #[test]
    fn test_parse_tuple_basic() {
        let program = parse("x = (10, 20)").expect("should parse tuple");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Assignment(assign) => {
                assert_eq!(assign.name, "x");
                match &assign.value {
                    Expr::Tuple(tuple) => {
                        assert_eq!(tuple.elements.len(), 2);
                    }
                    _ => panic!("Expected tuple expression"),
                }
            }
            _ => panic!("Expected assignment"),
        }
    }

    #[test]
    fn test_parse_tuple_three_elements() {
        let program = parse("person = (\"Alice\", 25, #1)").expect("should parse");
        match &program.statements[0] {
            Statement::Assignment(assign) => match &assign.value {
                Expr::Tuple(tuple) => {
                    assert_eq!(tuple.elements.len(), 3);
                }
                _ => panic!("Expected tuple"),
            },
            _ => panic!("Expected assignment"),
        }
    }

    #[test]
    fn test_parse_grouping_not_tuple() {
        let program = parse("x = (5 + 3) * 2").expect("should parse grouping");
        match &program.statements[0] {
            Statement::Assignment(assign) => match &assign.value {
                Expr::Binary(_) => {}, // Should be binary, not tuple
                _ => panic!("Expected binary expression, got {:?}", assign.value),
            },
            _ => panic!("Expected assignment"),
        }
    }

    #[test]
    fn test_parse_single_element_grouping() {
        let program = parse("x = (42)").expect("should parse");
        match &program.statements[0] {
            Statement::Assignment(assign) => match &assign.value {
                Expr::Literal(_) => {}, // Should be literal, not tuple
                _ => panic!("Expected literal, not tuple"),
            },
            _ => panic!("Expected assignment"),
        }
    }

    #[test]
    fn test_parse_nested_tuple() {
        let program = parse("x = ((1, 2), (3, 4))").expect("should parse nested tuple");
        match &program.statements[0] {
            Statement::Assignment(assign) => match &assign.value {
                Expr::Tuple(tuple) => {
                    assert_eq!(tuple.elements.len(), 2);
                    // Both elements should be tuples
                    match &tuple.elements[0] {
                        Expr::Tuple(inner) => assert_eq!(inner.elements.len(), 2),
                        _ => panic!("Expected nested tuple"),
                    }
                }
                _ => panic!("Expected tuple"),
            },
            _ => panic!("Expected assignment"),
        }
    }

    #[test]
    fn test_parse_tuple_with_trailing_comma() {
        let program = parse("x = (1, 2, 3,)").expect("should parse tuple with trailing comma");
        match &program.statements[0] {
            Statement::Assignment(assign) => match &assign.value {
                Expr::Tuple(tuple) => {
                    assert_eq!(tuple.elements.len(), 3);
                }
                _ => panic!("Expected tuple"),
            },
            _ => panic!("Expected assignment"),
        }
    }

    #[test]
    fn test_parse_tuple_in_array() {
        let program = parse("points = [(0, 0), (10, 20)]").expect("should parse");
        match &program.statements[0] {
            Statement::Assignment(assign) => match &assign.value {
                Expr::ArrayLiteral(arr) => {
                    assert_eq!(arr.elements.len(), 2);
                    // Both elements should be tuples
                    match &arr.elements[0] {
                        Expr::Tuple(tuple) => assert_eq!(tuple.elements.len(), 2),
                        _ => panic!("Expected tuple in array"),
                    }
                }
                _ => panic!("Expected array"),
            },
            _ => panic!("Expected assignment"),
        }
    }

    #[test]
    fn test_parse_array_in_tuple() {
        let program = parse("x = ([1, 2, 3], \"data\")").expect("should parse");
        match &program.statements[0] {
            Statement::Assignment(assign) => match &assign.value {
                Expr::Tuple(tuple) => {
                    assert_eq!(tuple.elements.len(), 2);
                    match &tuple.elements[0] {
                        Expr::ArrayLiteral(arr) => assert_eq!(arr.elements.len(), 3),
                        _ => panic!("Expected array in tuple"),
                    }
                }
                _ => panic!("Expected tuple"),
            },
            _ => panic!("Expected assignment"),
        }
    }
}
