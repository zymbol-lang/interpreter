//! Literal parsing for Zymbol-Lang
//!
//! Handles parsing of primitive literals: floats, chars, scientific notation

use crate::Parser;

impl Parser {}

#[cfg(test)]
#[allow(clippy::approx_constant)]
mod tests {
    use zymbol_ast::{Expr, Statement, Program};
    use zymbol_common::Literal;
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
    fn test_parse_float_literal() {
        let program = parse(">> 3.14").expect("should parse");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Output(output) => {
                assert_eq!(output.exprs.len(), 1);
                match &output.exprs[0] {
                    Expr::Literal(lit) => match &lit.value {
                        Literal::Float(f) => assert_eq!(*f, 3.14),
                        _ => panic!("Expected float literal"),
                    },
                    _ => panic!("Expected literal in output"),
                }
            }
            _ => panic!("Expected output statement"),
        }
    }

    #[test]
    fn test_parse_float_scientific() {
        let program = parse("x = 3e8").expect("should parse");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Assignment(assign) => {
                assert_eq!(assign.name, "x");
                match &assign.value {
                    Expr::Literal(lit) => match &lit.value {
                        Literal::Float(f) => assert_eq!(*f, 3e8),
                        _ => panic!("Expected float"),
                    },
                    _ => panic!("Expected literal"),
                }
            }
            _ => panic!("Expected assignment"),
        }
    }

    #[test]
    fn test_parse_char_literal() {
        let program = parse(">> 'A'").expect("should parse");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Output(output) => {
                assert_eq!(output.exprs.len(), 1);
                match &output.exprs[0] {
                    Expr::Literal(lit) => match &lit.value {
                        Literal::Char(c) => assert_eq!(*c, 'A'),
                        _ => panic!("Expected char literal"),
                    },
                    _ => panic!("Expected literal in output"),
                }
            }
            _ => panic!("Expected output statement"),
        }
    }

    #[test]
    fn test_parse_char_unicode() {
        let program = parse("emoji = '😀'").expect("should parse");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Assignment(assign) => {
                assert_eq!(assign.name, "emoji");
                match &assign.value {
                    Expr::Literal(lit) => match &lit.value {
                        Literal::Char(c) => assert_eq!(*c, '😀'),
                        _ => panic!("Expected char"),
                    },
                    _ => panic!("Expected literal"),
                }
            }
            _ => panic!("Expected assignment"),
        }
    }

    #[test]
    fn test_parse_mixed_numeric_types() {
        // Test that we can parse both int and float in same expression
        let program = parse(">> 42 3.14").expect("should parse");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Output(output) => {
                assert_eq!(output.exprs.len(), 2);

                // First should be integer
                match &output.exprs[0] {
                    Expr::Literal(lit) => match &lit.value {
                        Literal::Int(n) => assert_eq!(*n, 42),
                        _ => panic!("Expected int"),
                    },
                    _ => panic!("Expected literal"),
                }

                // Second should be float
                match &output.exprs[1] {
                    Expr::Literal(lit) => match &lit.value {
                        Literal::Float(f) => assert_eq!(*f, 3.14),
                        _ => panic!("Expected float"),
                    },
                    _ => panic!("Expected literal"),
                }
            }
            _ => panic!("Expected output"),
        }
    }
}
