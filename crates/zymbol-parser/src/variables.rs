//! Variable and constant parsing for Zymbol-Lang
//!
//! Handles parsing of:
//! - Assignment: name = expr
//! - Constants: name := expr (immutable)
//! - Compound assignment: +=, -=, *=, /=, %=
//! - Increment/decrement: ++, --
//! - Lifetime end: \variable (explicit destruction)

use zymbol_ast::{Assignment, BinaryExpr, CollectionUpdateExpr, ConstDecl, Expr, IdentifierExpr, IndexExpr, LifetimeEnd, LiteralExpr, Statement};
use zymbol_common::{BinaryOp, Literal};
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;
use crate::Parser;

impl Parser {
    /// Parse assignment statement: name = expr (with compound ops and increment/decrement)
    pub(crate) fn parse_assignment(&mut self) -> Result<Statement, Diagnostic> {
        let ident_token = self.advance();
        let name = match &ident_token.kind {
            TokenKind::Ident(s) => s.clone(),
            _ => return Err(Diagnostic::error("expected identifier").with_span(ident_token.span)),
        };

        // Check for indexed assignment: arr[i] = val
        // Desugar to: arr = arr[i]$~ val  (CollectionUpdate)
        if matches!(self.peek().kind, TokenKind::LBracket) {
            self.advance(); // consume '['
            let index_expr = self.parse_expr()?;
            let rbracket = self.peek().clone();
            if !matches!(rbracket.kind, TokenKind::RBracket) {
                return Err(Diagnostic::error("expected ']' after index expression")
                    .with_span(rbracket.span));
            }
            self.advance(); // consume ']'

            let assign_tok = self.peek().clone();
            if !matches!(assign_tok.kind, TokenKind::Assign) {
                return Err(Diagnostic::error("expected '=' after index expression for indexed assignment")
                    .with_span(assign_tok.span)
                    .with_help("syntax: arr[i] = val"));
            }
            self.advance(); // consume '='

            let value_expr = self.parse_expr()?;
            let span = ident_token.span.to(&value_expr.span());

            // Build arr[i] target expression
            let target_arr = Expr::Identifier(IdentifierExpr::new(name.clone(), ident_token.span));
            let index_node = Expr::Index(IndexExpr::new(
                Box::new(target_arr),
                Box::new(index_expr),
                ident_token.span.to(&rbracket.span),
            ));

            // Wrap in CollectionUpdate: arr[i]$~ val
            let update_expr = Expr::CollectionUpdate(CollectionUpdateExpr::new(
                Box::new(index_node),
                Box::new(value_expr),
                span,
            ));

            return Ok(Statement::Assignment(Assignment::new(name, update_expr, span)));
        }

        let assign_token = self.peek();

        // Check for increment/decrement (++, --)
        if matches!(assign_token.kind, TokenKind::PlusPlus | TokenKind::MinusMinus) {
            let op_token = self.advance();
            let op = match op_token.kind {
                TokenKind::PlusPlus => BinaryOp::Add,
                TokenKind::MinusMinus => BinaryOp::Sub,
                _ => unreachable!(),
            };

            // Expand x++ to x = x + 1 (or x-- to x = x - 1)
            let var_expr = Expr::Identifier(IdentifierExpr::new(name.clone(), ident_token.span));
            let one_expr = Expr::Literal(LiteralExpr::new(Literal::Int(1), op_token.span));

            let binary_expr = Expr::Binary(BinaryExpr::new(
                op,
                Box::new(var_expr),
                Box::new(one_expr),
                ident_token.span.to(&op_token.span),
            ));

            let span = ident_token.span.to(&op_token.span);
            return Ok(Statement::Assignment(Assignment::new(name, binary_expr, span)));
        }

        // Check for compound assignment (+=, -=, *=, /=, %=)
        let op = match assign_token.kind {
            TokenKind::PlusAssign => Some(BinaryOp::Add),
            TokenKind::MinusAssign => Some(BinaryOp::Sub),
            TokenKind::StarAssign => Some(BinaryOp::Mul),
            TokenKind::SlashAssign => Some(BinaryOp::Div),
            TokenKind::PercentAssign => Some(BinaryOp::Mod),
            TokenKind::Assign => None, // Regular assignment
            _ => {
                return Err(Diagnostic::error("expected assignment operator (=, +=, -=, *=, /=, %=, ++, --)")
                    .with_span(assign_token.span)
                    .with_help("use '=' to assign a value to a variable"));
            }
        };

        self.advance(); // consume assignment operator

        if let Some(op) = op {
            // Compound assignment: expand a += b to a = a + b
            let right_expr = self.parse_expr()?;

            let var_expr = Expr::Identifier(IdentifierExpr::new(name.clone(), ident_token.span));
            let binary_expr = Expr::Binary(BinaryExpr::new(
                op,
                Box::new(var_expr),
                Box::new(right_expr.clone()),
                ident_token.span.to(&right_expr.span()),
            ));

            let span = ident_token.span.to(&binary_expr.span());
            Ok(Statement::Assignment(Assignment::new(name, binary_expr, span)))
        } else {
            // Regular assignment: name = expr [, expr ...]
            // Comma concatenation: fullname = first, ' ', last  (strings/values)
            let first = self.parse_expr()?;
            let value = if matches!(self.peek().kind, TokenKind::Comma) {
                // Collect comma-separated expressions and wrap in Concat chain
                let mut acc = first;
                while matches!(self.peek().kind, TokenKind::Comma) {
                    self.advance(); // consume ','
                    let next = self.parse_expr()?;
                    let span = acc.span().to(&next.span());
                    acc = Expr::Binary(BinaryExpr::new(
                        BinaryOp::Add,
                        Box::new(acc),
                        Box::new(next),
                        span,
                    ));
                }
                acc
            } else {
                first
            };
            let span = ident_token.span.to(&value.span());
            Ok(Statement::Assignment(Assignment::new(name, value, span)))
        }
    }

    /// Parse constant declaration: name := expr
    pub(crate) fn parse_const_decl(&mut self) -> Result<Statement, Diagnostic> {
        let ident_token = self.advance();
        let name = match &ident_token.kind {
            TokenKind::Ident(s) => s.clone(),
            _ => return Err(Diagnostic::error("expected identifier").with_span(ident_token.span)),
        };

        // Expect :=
        let const_assign_token = self.peek();
        if !matches!(const_assign_token.kind, TokenKind::ConstAssign) {
            return Err(Diagnostic::error("expected := for constant declaration")
                .with_span(const_assign_token.span)
                .with_help("use := to declare constants (immutable values)"));
        }

        self.advance(); // consume :=

        // Parse value expression (with comma concatenation: NAME := "a", " ", "b")
        let first = self.parse_expr()?;
        let value = if matches!(self.peek().kind, TokenKind::Comma) {
            let mut acc = first;
            while matches!(self.peek().kind, TokenKind::Comma) {
                self.advance(); // consume ','
                let next = self.parse_expr()?;
                let span = acc.span().to(&next.span());
                acc = Expr::Binary(BinaryExpr::new(
                    BinaryOp::Add,
                    Box::new(acc),
                    Box::new(next),
                    span,
                ));
            }
            acc
        } else {
            first
        };
        let span = ident_token.span.to(&value.span());

        Ok(Statement::ConstDecl(ConstDecl::new(name, value, span)))
    }

    /// Parse lifetime end: \variable
    pub(crate) fn parse_lifetime_end(&mut self) -> Result<Statement, Diagnostic> {
        let backslash_token = self.advance(); // consume \

        // Expect identifier
        let ident_token = self.peek();
        if !matches!(ident_token.kind, TokenKind::Ident(_)) {
            return Err(Diagnostic::error("expected variable name after \\")
                .with_span(ident_token.span)
                .with_help("syntax: \\variable to explicitly destroy a variable"));
        }

        let ident_token = self.advance(); // consume identifier
        let variable_name = match &ident_token.kind {
            TokenKind::Ident(s) => s.clone(),
            _ => unreachable!(),
        };

        let span = backslash_token.span.to(&ident_token.span);
        Ok(Statement::LifetimeEnd(LifetimeEnd::new(variable_name, span)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_lexer::Lexer;
    use zymbol_span::FileId;
    use zymbol_ast::Program;

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
    fn test_parse_assignment() {
        let program = parse("x = \"hello\"").expect("should parse");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Assignment(assign) => {
                assert_eq!(assign.name, "x");
                match &assign.value {
                    Expr::Literal(lit) => match &lit.value {
                        Literal::String(s) => assert_eq!(s, "hello"),
                        _ => panic!("Expected string"),
                    },
                    _ => panic!("Expected literal"),
                }
            }
            _ => panic!("Expected assignment"),
        }
    }
}
