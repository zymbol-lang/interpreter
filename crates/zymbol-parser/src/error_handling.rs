//! Error handling statement parsing for Zymbol-Lang
//!
//! Parses:
//! - TRY: !? { }
//! - CATCH: :! { } or :! ##Type { }
//! - FINALLY: :> { }

use zymbol_ast::{Statement, TryStmt, CatchClause, ErrorType, FinallyClause};
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;

use crate::Parser;

impl Parser {
    /// Parse a try statement: !? { } :! { } :> { }
    ///
    /// Syntax:
    /// ```zymbol
    /// !?{
    ///     // try block
    /// } :! ##IO {
    ///     // typed catch
    /// } :! {
    ///     // generic catch
    /// } :>{
    ///     // finally
    /// }
    /// ```
    pub(crate) fn parse_try_statement(&mut self) -> Result<Statement, Diagnostic> {
        let start_token = self.peek().clone();

        // Consume !?
        if !matches!(start_token.kind, TokenKind::TryBlock) {
            return Err(Diagnostic::error("expected '!?' to start try block")
                .with_span(start_token.span)
                .with_help("try blocks start with !?{ }"));
        }
        self.advance();

        // Parse try block
        let try_block = self.parse_block()?;

        // Parse catch clauses (zero or more)
        let mut catch_clauses = Vec::new();
        while matches!(self.peek().kind, TokenKind::CatchBlock) {
            catch_clauses.push(self.parse_catch_clause()?);
        }

        // Parse optional finally clause
        let finally_clause = if matches!(self.peek().kind, TokenKind::FinallyBlock) {
            Some(self.parse_finally_clause()?)
        } else {
            None
        };

        // Calculate span
        let end_span = if let Some(ref finally) = finally_clause {
            finally.block.span
        } else if let Some(last_catch) = catch_clauses.last() {
            last_catch.block.span
        } else {
            try_block.span
        };

        let span = start_token.span.to(&end_span);

        Ok(Statement::Try(TryStmt::new(
            try_block,
            catch_clauses,
            finally_clause,
            span,
        )))
    }

    /// Parse a catch clause: :! { } or :! ##Type { }
    fn parse_catch_clause(&mut self) -> Result<CatchClause, Diagnostic> {
        let start_token = self.peek().clone();

        // Consume :!
        if !matches!(start_token.kind, TokenKind::CatchBlock) {
            return Err(Diagnostic::error("expected ':!' to start catch block")
                .with_span(start_token.span)
                .with_help("catch blocks start with :! { } or :! ##Type { }"));
        }
        self.advance();

        // Check for optional error type: ##Type
        let error_type = if matches!(self.peek().kind, TokenKind::Hash) {
            Some(self.parse_error_type()?)
        } else {
            None
        };

        // Parse catch block
        let block = self.parse_block()?;

        let span = start_token.span.to(&block.span);

        Ok(CatchClause::new(error_type, block, span))
    }

    /// Parse an error type: ##IO, ##Network, ##Parse, etc.
    fn parse_error_type(&mut self) -> Result<ErrorType, Diagnostic> {
        let start_token = self.peek().clone();

        // Expect first #
        if !matches!(start_token.kind, TokenKind::Hash) {
            return Err(Diagnostic::error("expected '##' for error type")
                .with_span(start_token.span)
                .with_help("error types use ## prefix: ##IO, ##Network, ##Parse"));
        }
        self.advance();

        // Expect second #
        let second_hash = self.peek().clone();
        if !matches!(second_hash.kind, TokenKind::Hash) {
            return Err(Diagnostic::error("expected '##' for error type (missing second #)")
                .with_span(second_hash.span)
                .with_help("error types use ## prefix: ##IO, ##Network, ##Parse"));
        }
        self.advance();

        // Expect identifier (error type name)
        let name_token = self.peek().clone();
        let name = match &name_token.kind {
            TokenKind::Ident(name) => name.clone(),
            TokenKind::Underscore => "_".to_string(), // ##_ for generic error
            _ => {
                return Err(Diagnostic::error("expected error type name after '##'")
                    .with_span(name_token.span)
                    .with_help("valid error types: ##IO, ##Network, ##Parse, ##Index, ##Type, ##Div, ##_"));
            }
        };
        self.advance();

        let span = start_token.span.to(&name_token.span);

        Ok(ErrorType::new(name, span))
    }

    /// Parse a finally clause: :> { }
    fn parse_finally_clause(&mut self) -> Result<FinallyClause, Diagnostic> {
        let start_token = self.peek().clone();

        // Consume :>
        if !matches!(start_token.kind, TokenKind::FinallyBlock) {
            return Err(Diagnostic::error("expected ':>' to start finally block")
                .with_span(start_token.span)
                .with_help("finally blocks start with :>{ }"));
        }
        self.advance();

        // Parse finally block
        let block = self.parse_block()?;

        let span = start_token.span.to(&block.span);

        Ok(FinallyClause::new(block, span))
    }
}

#[cfg(test)]
mod tests {
    use zymbol_lexer::Lexer;
    use zymbol_span::FileId;
    use crate::Parser;
    use zymbol_ast::Statement;

    fn parse(source: &str) -> Result<Vec<Statement>, Vec<zymbol_error::Diagnostic>> {
        let lexer = Lexer::new(source, FileId(0));
        let (tokens, _) = lexer.tokenize();
        let parser = Parser::new(tokens);
        parser.parse().map(|p| p.statements)
    }

    #[test]
    fn test_parse_try_only() {
        let result = parse("!?{ x = 1 }");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);
        assert!(matches!(stmts[0], Statement::Try(_)));
    }

    #[test]
    fn test_parse_try_catch_generic() {
        let result = parse("!?{ x = 1 } :! { y = 2 }");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);

        if let Statement::Try(try_stmt) = &stmts[0] {
            assert_eq!(try_stmt.catch_clauses.len(), 1);
            assert!(try_stmt.catch_clauses[0].error_type.is_none());
            assert!(try_stmt.finally_clause.is_none());
        } else {
            panic!("Expected Try statement");
        }
    }

    #[test]
    fn test_parse_try_catch_typed() {
        let result = parse("!?{ x = 1 } :! ##IO { y = 2 }");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);

        if let Statement::Try(try_stmt) = &stmts[0] {
            assert_eq!(try_stmt.catch_clauses.len(), 1);
            assert!(try_stmt.catch_clauses[0].error_type.is_some());
            assert_eq!(try_stmt.catch_clauses[0].error_type.as_ref().unwrap().name, "IO");
        } else {
            panic!("Expected Try statement");
        }
    }

    #[test]
    fn test_parse_try_finally() {
        let result = parse("!?{ x = 1 } :>{ cleanup() }");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);

        if let Statement::Try(try_stmt) = &stmts[0] {
            assert!(try_stmt.catch_clauses.is_empty());
            assert!(try_stmt.finally_clause.is_some());
        } else {
            panic!("Expected Try statement");
        }
    }

    #[test]
    fn test_parse_try_catch_finally() {
        let result = parse("!?{ x = 1 } :! { y = 2 } :>{ cleanup() }");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);

        if let Statement::Try(try_stmt) = &stmts[0] {
            assert_eq!(try_stmt.catch_clauses.len(), 1);
            assert!(try_stmt.finally_clause.is_some());
        } else {
            panic!("Expected Try statement");
        }
    }

    #[test]
    fn test_parse_try_multiple_catches() {
        let result = parse("!?{ x = 1 } :! ##IO { a = 1 } :! ##Network { b = 2 } :! { c = 3 }");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);

        if let Statement::Try(try_stmt) = &stmts[0] {
            assert_eq!(try_stmt.catch_clauses.len(), 3);
            assert_eq!(try_stmt.catch_clauses[0].error_type.as_ref().unwrap().name, "IO");
            assert_eq!(try_stmt.catch_clauses[1].error_type.as_ref().unwrap().name, "Network");
            assert!(try_stmt.catch_clauses[2].error_type.is_none()); // Generic
        } else {
            panic!("Expected Try statement");
        }
    }

    #[test]
    fn test_parse_try_with_generic_error_type() {
        let result = parse("!?{ x = 1 } :! ##_ { y = 2 }");
        assert!(result.is_ok());
        let stmts = result.unwrap();

        if let Statement::Try(try_stmt) = &stmts[0] {
            assert_eq!(try_stmt.catch_clauses[0].error_type.as_ref().unwrap().name, "_");
        } else {
            panic!("Expected Try statement");
        }
    }

    #[test]
    fn test_parse_full_try_catch_finally_structure() {
        let source = r#"
            !?{
                data = read_file("config.txt")
                process(data)
            } :! ##IO {
                >> "File error" ¶
            } :! ##Parse {
                >> "Parse error" ¶
            } :! {
                >> "Unknown error" ¶
            } :>{
                cleanup()
            }
        "#;

        let result = parse(source);
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);

        if let Statement::Try(try_stmt) = &stmts[0] {
            assert_eq!(try_stmt.catch_clauses.len(), 3);
            assert!(try_stmt.finally_clause.is_some());
        } else {
            panic!("Expected Try statement");
        }
    }
}
