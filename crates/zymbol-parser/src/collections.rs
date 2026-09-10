//! Collection parsing for Zymbol-Lang
//!
//! Handles parsing of collection expressions:
//! - Array literals: [expr1, expr2, ...]
//! - Tuples: (expr1, expr2, ...)
//! - Named tuples: (name: value, name2: value2)
//! - Grouped expressions: (expr)

use zymbol_ast::{ArrayLiteralExpr, Expr, GroupExpr, NamedTupleExpr, TupleExpr};
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;
use crate::Parser;

impl Parser {
    /// Parse array literal: [1, 2, 3]
    /// `#[…]` — an array whose mix of element types is **declared**.
    ///
    /// Same collection and same type as `[…]`: `#?` answers `##]` for both and
    /// every operator behaves the same (decision 15). What changes is that the
    /// analyser does not check the elements against each other — and that it
    /// warns when the mix turns out not to be one (decision 18).
    ///
    /// The `#` is lexed on its own, so nothing in the lexer had to move: `#1`,
    /// `#?`, `##]` and `#०९#` are untouched. `#[` was a syntax error in all
    /// three engines, which is exactly what left it free for this.
    pub(crate) fn parse_mixed_array_literal(&mut self) -> Result<Expr, Diagnostic> {
        let hash = self.advance(); // consume #
        let arr = self.parse_array_literal()?;
        match arr {
            Expr::ArrayLiteral(a) => {
                let span = hash.span.to(&a.span);
                Ok(Expr::ArrayLiteral(ArrayLiteralExpr::new_mixed(a.elements, span)))
            }
            other => Ok(other),
        }
    }

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
        elements.push(self.parse_expr_juxt()?);

        // Parse remaining elements (comma-separated)
        while matches!(self.peek().kind, TokenKind::Comma) {
            self.advance(); // consume ,

            // Allow trailing comma
            if matches!(self.peek().kind, TokenKind::RBracket) {
                break;
            }

            elements.push(self.parse_expr_juxt()?);
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

    /// Parse a dictionary literal opened by `#(` (GAP-ZYB-003/004).
    ///
    /// `#(` says which of the two things a pair of parentheses opens, so no
    /// lookahead decides it and an empty one is writable: `#()` is the empty
    /// dictionary and `()` is not it. Keys may be strings as well as bare
    /// names — `d["gasto.alimentación"]$~ v` always added such a key and only
    /// the literal could not spell it, which left the keys a program actually
    /// needs (the ones from a database, from JSON, with a domain prefix)
    /// outside the notation.
    pub(crate) fn parse_dict_literal(&mut self) -> Result<Expr, Diagnostic> {
        let hash_lparen = self.advance(); // consume #(
        self.parse_dict_fields(hash_lparen, true)
    }

    /// The fields of a dictionary, shared by `#(…)` and the legacy `(a: 1)`.
    ///
    /// `allow_empty` is what `#(` buys: `#()` is the empty dictionary, while a
    /// bare `()` cannot be one — it would have to be the empty tuple as well,
    /// and the two are not the same value. The empty dictionary was reachable
    /// before this (take the only key out of `(a: 1)` and `$#` is 0) and simply
    /// could not be written, so every program that built one at run time had to
    /// start it with an invented key and remove it afterwards.
    fn parse_dict_fields(
        &mut self,
        open_token: zymbol_lexer::Token,
        allow_empty: bool,
    ) -> Result<Expr, Diagnostic> {
        let mut fields = Vec::new();
        // Which keys the source quoted — the one thing about a key that the
        // pair `(String, Expr)` cannot carry, and the formatter has to know
        // (see `NamedTupleExpr::quoted`).
        let mut quoted = Vec::new();

        if allow_empty && matches!(self.peek().kind, TokenKind::RParen) {
            let rparen = self.advance();
            let span = open_token.span.to(&rparen.span);
            return Ok(Expr::NamedTuple(NamedTupleExpr::new(fields, span)));
        }

        loop {
            // A key is a bare name or a string. `COLLECTIONS.md` § 5 calls the
            // computed key "what makes this a dictionary and not a record", and
            // the bracket exists because "a JSON key can be any string" — the
            // literal had stayed on the record's side of that line.
            let field_token = self.peek().clone();
            let field_name = match &field_token.kind {
                TokenKind::Ident(name) => {
                    quoted.push(false);
                    name.clone()
                }
                TokenKind::String(text) => {
                    quoted.push(true);
                    text.clone()
                }
                _ => {
                    return Err(Diagnostic::error("expected a key in the dictionary")
                        .with_span(field_token.span)
                        .with_help(
                            "a key is a name or a string: #(nombre: valor) or \
                             #(\"gasto.alimentación\": valor)",
                        ));
                }
            };
            self.advance(); // consume the key

            if !matches!(self.peek().kind, TokenKind::Colon) {
                return Err(Diagnostic::error("expected ':' after the key")
                    .with_span(self.peek().span));
            }
            self.advance(); // consume :

            let value_expr = self.parse_expr()?;
            fields.push((field_name, value_expr));

            if matches!(self.peek().kind, TokenKind::Comma) {
                self.advance(); // consume ,
                // A trailing comma before `)` closes the literal.
                if matches!(self.peek().kind, TokenKind::RParen) {
                    break;
                }
            } else {
                break;
            }
        }

        let rparen_token = self.peek().clone();
        if !matches!(rparen_token.kind, TokenKind::RParen) {
            return Err(Diagnostic::error("expected ')' to close the dictionary")
                .with_span(rparen_token.span)
                .with_help("a dictionary is written #(key: value, key2: value2)"));
        }
        let rparen_token = self.advance(); // consume )

        let span = open_token.span.to(&rparen_token.span);
        Ok(Expr::NamedTuple(NamedTupleExpr::with_quoted(fields, quoted, span)))
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
            // A dictionary is written `#(…)`. The bare form was how it was
            // written until v0.0.9, when the colon was the whole of what told
            // `(a: 1)` from `(1, 2)` — and the empty dictionary could not be
            // written at all, because `()` would have to be both. Refused
            // rather than accepted quietly, because two spellings for one thing
            // is what the mark was introduced to end.
            return Err(Diagnostic::error("a dictionary is written `#(…)`")
                .with_span(lparen_token.span)
                .with_help(
                    "write `#(` here: `#(nombre: valor)`. A bare `(…)` is a \
                     positional tuple, and `#()` is the empty dictionary — \
                     which `()` cannot be, since it would have to be the empty \
                     tuple as well",
                ));
        }

        {
            // Parse positional tuple or grouped expression
            let first_expr = self.parse_expr_juxt()?;

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

                    elements.push(self.parse_expr_juxt()?);
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
                // It's a grouped expression — wrap it so the formatter can
                // reproduce the user's parentheses (Expr::Group is
                // semantically transparent everywhere else).
                let rparen_token = self.peek().clone();
                if !matches!(rparen_token.kind, TokenKind::RParen) {
                    return Err(Diagnostic::error("expected ')' to close grouped expression")
                        .with_span(rparen_token.span)
                        .with_help("grouped expressions must be enclosed in parentheses: (expr)"));
                }
                let rparen_token = self.advance(); // consume )

                let span = lparen_token.span.to(&rparen_token.span);
                Ok(Expr::Group(GroupExpr::new(Box::new(first_expr), span)))
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
            Statement::Assignment(assign) => {
                // (42) is a Group preserving the user's parens, not a tuple;
                // unwrap_group() must see through to the literal.
                match &assign.value {
                    Expr::Group(_) => {}
                    _ => panic!("Expected group, not tuple"),
                }
                match assign.value.unwrap_group() {
                    Expr::Literal(_) => {}
                    _ => panic!("Expected literal inside group"),
                }
            }
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
