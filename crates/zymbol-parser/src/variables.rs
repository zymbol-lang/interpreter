//! Variable and constant parsing for Zymbol-Lang
//!
//! Handles parsing of:
//! - Assignment: name = expr
//! - Constants: name := expr (immutable)
//! - Compound assignment: +=, -=, *=, /=, %=, ^=
//! - Increment/decrement: ++, --
//! - Lifetime end: \variable (explicit destruction)

use zymbol_ast::{Assignment, BinaryExpr, CollectionUpdateExpr, ConstDecl, DestructureAssign, DestructureItem, DestructurePattern, ErrorPropagateExpr, Expr, ExprStatement, IdentifierExpr, IndexExpr, LifetimeEnd, LiteralExpr, Statement};
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
            let compound_op = match assign_tok.kind {
                TokenKind::Assign => None,
                TokenKind::PlusAssign => Some(BinaryOp::Add),
                TokenKind::MinusAssign => Some(BinaryOp::Sub),
                TokenKind::StarAssign => Some(BinaryOp::Mul),
                TokenKind::SlashAssign => Some(BinaryOp::Div),
                TokenKind::PercentAssign => Some(BinaryOp::Mod),
                TokenKind::CaretAssign => Some(BinaryOp::Pow),
                _ => {
                    return Err(Diagnostic::error("expected '=' after index expression for indexed assignment")
                        .with_span(assign_tok.span)
                        .with_help("syntax: arr[i] = val  or  arr[i] += val"));
                }
            };
            self.advance(); // consume operator

            let rhs = self.parse_expr()?;

            // For compound ops: arr[i] += rhs  →  arr[i]$~ (arr[i] + rhs)
            let value_expr = if let Some(op) = compound_op {
                let arr_ident = Expr::Identifier(IdentifierExpr::new(name.clone(), ident_token.span));
                let current_elem = Expr::Index(IndexExpr::new(
                    Box::new(arr_ident),
                    Box::new(index_expr.clone()),
                    ident_token.span.to(&rbracket.span),
                ));
                let rhs_span = rhs.span();
                Expr::Binary(BinaryExpr::new(
                    op,
                    Box::new(current_elem),
                    Box::new(rhs),
                    ident_token.span.to(&rhs_span),
                ))
            } else {
                rhs
            };

            let span = ident_token.span.to(&value_expr.span());

            // Build arr[i] target expression
            let target_arr = Expr::Identifier(IdentifierExpr::new(name.clone(), ident_token.span));
            let index_node = Expr::Index(IndexExpr::new(
                Box::new(target_arr),
                Box::new(index_expr),
                ident_token.span.to(&rbracket.span),
            ));

            // Wrap in CollectionUpdate: arr[i]$~ value_expr
            let update_expr = Expr::CollectionUpdate(CollectionUpdateExpr::new(
                Box::new(index_node),
                Box::new(value_expr),
                span,
            ));

            return Ok(Statement::Assignment(Assignment::new(name, update_expr, span)));
        }

        let assign_token = self.peek();

        // Check for error propagation as statement: value$!!
        if matches!(assign_token.kind, TokenKind::DollarExclaimExclaim) {
            let op_token = self.advance(); // consume $!!
            let ident_expr = Expr::Identifier(IdentifierExpr::new(name.clone(), ident_token.span));
            let span = ident_token.span.to(&op_token.span);
            let propagate_expr = Expr::ErrorPropagate(ErrorPropagateExpr::new(Box::new(ident_expr), span));
            return Ok(Statement::Expr(ExprStatement::new(propagate_expr, span)));
        }

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

        // Check for compound assignment (+=, -=, *=, /=, %=, ^=)
        let op = match assign_token.kind {
            TokenKind::PlusAssign => Some(BinaryOp::Add),
            TokenKind::MinusAssign => Some(BinaryOp::Sub),
            TokenKind::StarAssign => Some(BinaryOp::Mul),
            TokenKind::SlashAssign => Some(BinaryOp::Div),
            TokenKind::PercentAssign => Some(BinaryOp::Mod),
            TokenKind::CaretAssign => Some(BinaryOp::Pow),
            TokenKind::Assign => None, // Regular assignment
            _ => {
                return Err(Diagnostic::error("expected assignment operator (=, +=, -=, *=, /=, %=, ^=, ++, --)")
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
            // Regular assignment: name = expr [expr ...]
            // Juxtaposition concatenation: s = "hello" ' ' name " world"
            // Same-line adjacent primaries are implicitly concatenated (no separator needed)
            let first = self.parse_expr()?;
            let value = self.parse_juxtapose_chain(first)?;
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

        // Parse value expression with juxtaposition concatenation: NAME := "hello" ' ' name
        let first = self.parse_expr()?;
        let value = self.parse_juxtapose_chain(first)?;
        let span = ident_token.span.to(&value.span());

        Ok(Statement::ConstDecl(ConstDecl::new(name, value, span)))
    }

    /// Build a juxtaposition-concatenation chain from an initial expression.
    /// Same-line adjacent primaries are treated as implicit string concatenation:
    ///   s = "hello" ' ' name " world"
    /// The comma operator is no longer used for concat — use juxtaposition or + for strings.
    pub(crate) fn parse_juxtapose_chain(&mut self, first: Expr) -> Result<Expr, Diagnostic> {
        let mut acc = first;
        loop {
            let next_tok = self.peek();
            let same_line = next_tok.span.start.line == acc.span().end.line;
            if same_line && Self::can_juxtapose(&next_tok.kind) {
                let next_expr = self.parse_expr()?;
                let span = acc.span().to(&next_expr.span());
                acc = Expr::Binary(BinaryExpr::new(
                    BinaryOp::Concat,
                    Box::new(acc),
                    Box::new(next_expr),
                    span,
                ));
            } else {
                break;
            }
        }
        Ok(acc)
    }

    /// Returns true if this token kind can start a juxtaposed expression.
    /// Used to detect implicit concatenation: s = "hello" ' ' name " world"
    /// Note: LParen is intentionally excluded — it is ambiguous with lambda
    /// comparators (e.g. arr$^+ (a, b -> ...)) and grouped expressions.
    pub(crate) fn can_juxtapose(kind: &TokenKind) -> bool {
        matches!(kind,
            TokenKind::String(_) |
            TokenKind::StringInterpolated(_) |
            TokenKind::Char(_) |
            TokenKind::Integer(_) |
            TokenKind::Float(_) |
            TokenKind::Boolean(_) |
            TokenKind::Ident(_)
        )
    }

    /// Returns true if current `[` starts a destructure assignment (not an array literal).
    /// Uses save/restore to look past the bracket group and check for `=`.
    pub(crate) fn is_array_destructure(&mut self) -> bool {
        let saved = self.current;
        self.advance(); // consume [
        let mut depth = 1i32;
        while depth > 0 && !self.is_at_end() {
            match self.peek().kind {
                TokenKind::LBracket => { depth += 1; self.advance(); }
                TokenKind::RBracket => { depth -= 1; self.advance(); }
                _ => { self.advance(); }
            }
        }
        let result = matches!(self.peek().kind, TokenKind::Assign);
        self.current = saved;
        result
    }

    /// Returns true if current `(` starts a destructure assignment (not a tuple expression).
    /// Uses save/restore to look past the paren group and check for `=`.
    pub(crate) fn is_tuple_destructure(&mut self) -> bool {
        let saved = self.current;
        self.advance(); // consume (
        let mut depth = 1i32;
        while depth > 0 && !self.is_at_end() {
            match self.peek().kind {
                TokenKind::LParen => { depth += 1; self.advance(); }
                TokenKind::RParen => { depth -= 1; self.advance(); }
                _ => { self.advance(); }
            }
        }
        let result = matches!(self.peek().kind, TokenKind::Assign);
        self.current = saved;
        result
    }

    /// Parse a destructure assignment: `[a, b] = expr` or `(name: n, age: a) = expr`
    pub(crate) fn parse_destructure_assign(&mut self) -> Result<Statement, Diagnostic> {
        let start_span = self.peek().span;

        let pattern = if matches!(self.peek().kind, TokenKind::LBracket) {
            self.parse_array_destructure_pattern()?
        } else {
            self.parse_tuple_destructure_pattern()?
        };

        let assign_tok = self.peek().clone();
        if !matches!(assign_tok.kind, TokenKind::Assign) {
            return Err(Diagnostic::error("expected '=' in destructure assignment")
                .with_span(assign_tok.span));
        }
        self.advance(); // consume =

        let value = self.parse_expr()?;
        let span = start_span.to(&value.span());
        Ok(Statement::DestructureAssign(DestructureAssign::new(pattern, value, span)))
    }

    fn parse_array_destructure_pattern(&mut self) -> Result<DestructurePattern, Diagnostic> {
        self.advance(); // consume [
        let mut items = Vec::new();
        loop {
            match self.peek().kind.clone() {
                TokenKind::RBracket => { self.advance(); break; }
                TokenKind::Comma => { self.advance(); }
                TokenKind::Star => {
                    self.advance(); // consume *
                    let ident = self.peek().clone();
                    let name = match &ident.kind {
                        TokenKind::Ident(n) => n.clone(),
                        _ => return Err(Diagnostic::error("expected identifier after '*' in destructure")
                            .with_span(ident.span)),
                    };
                    self.advance();
                    items.push(DestructureItem::Rest(name));
                }
                TokenKind::Underscore => {
                    self.advance();
                    items.push(DestructureItem::Ignore);
                }
                TokenKind::Ident(n) => {
                    let name = n.clone();
                    self.advance();
                    items.push(DestructureItem::Bind(name));
                }
                _ => return Err(Diagnostic::error("expected identifier, '*rest', or '_' in array destructure")
                    .with_span(self.peek().span)),
            }
        }
        Ok(DestructurePattern::Array(items))
    }

    fn parse_tuple_destructure_pattern(&mut self) -> Result<DestructurePattern, Diagnostic> {
        self.advance(); // consume (

        // Determine named vs positional: if first content is `ident :` → named
        let is_named = matches!(
            (self.peek().kind.clone(), self.peek_ahead(1).map(|t| t.kind.clone())),
            (TokenKind::Ident(_), Some(TokenKind::Colon))
        );

        let mut named_pairs: Vec<(String, String)> = Vec::new();
        let mut positional_items: Vec<DestructureItem> = Vec::new();

        loop {
            match self.peek().kind.clone() {
                TokenKind::RParen => { self.advance(); break; }
                TokenKind::Comma => { self.advance(); }
                TokenKind::Star if !is_named => {
                    self.advance();
                    let ident = self.peek().clone();
                    let name = match &ident.kind {
                        TokenKind::Ident(n) => n.clone(),
                        _ => return Err(Diagnostic::error("expected identifier after '*' in destructure")
                            .with_span(ident.span)),
                    };
                    self.advance();
                    positional_items.push(DestructureItem::Rest(name));
                }
                TokenKind::Ident(n) if is_named => {
                    let field = n.clone();
                    self.advance(); // consume field name
                    // Expect ':'
                    let colon = self.peek().clone();
                    if !matches!(colon.kind, TokenKind::Colon) {
                        return Err(Diagnostic::error("expected ':' after field name in named tuple destructure")
                            .with_span(colon.span));
                    }
                    self.advance(); // consume :
                    let var_tok = self.peek().clone();
                    let var_name = match &var_tok.kind {
                        TokenKind::Ident(n) => n.clone(),
                        _ => return Err(Diagnostic::error("expected variable name after ':' in named tuple destructure")
                            .with_span(var_tok.span)),
                    };
                    self.advance();
                    named_pairs.push((field, var_name));
                }
                TokenKind::Ident(n) if !is_named => {
                    let name = n.clone();
                    self.advance();
                    positional_items.push(DestructureItem::Bind(name));
                }
                _ => return Err(Diagnostic::error("unexpected token in tuple destructure pattern")
                    .with_span(self.peek().span)),
            }
        }

        if is_named {
            Ok(DestructurePattern::NamedTuple(named_pairs))
        } else {
            Ok(DestructurePattern::Positional(positional_items))
        }
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
