//! Function parsing for Zymbol-Lang
//!
//! Handles parsing of function-related constructs:
//! - Function declarations: name(params) { body }
//! - Lambda expressions: x -> expr or (a, b) -> { block }
//! - Return statements: <~ expr
//! - Function call statements

use zymbol_ast::{Expr, ExprStatement, FunctionDecl, LambdaBody, Parameter, ParameterKind, ReturnStmt, Statement};
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;
use crate::Parser;

impl Parser {
    /// Parse function declaration: name(params) { body }
    pub(crate) fn parse_function_decl(&mut self) -> Result<Statement, Diagnostic> {
        let name_token = self.advance(); // consume name
        let name = match &name_token.kind {
            TokenKind::Ident(s) => s.clone(),
            _ => unreachable!(),
        };

        // Expect (
        let lparen_token = self.peek().clone();
        if !matches!(lparen_token.kind, TokenKind::LParen) {
            return Err(Diagnostic::error("expected '(' after function name")
                .with_span(lparen_token.span)
                .with_help("function syntax: name(params) { }"));
        }
        self.advance(); // consume (

        // Parse parameters
        let mut parameters = Vec::new();

        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                // Parse parameter name
                let param_token = self.peek().clone();
                let param_name = match &param_token.kind {
                    TokenKind::Ident(s) => {
                        self.advance(); // consume param name
                        s.clone()
                    }
                    _ => {
                        return Err(Diagnostic::error("expected parameter name")
                            .with_span(param_token.span)
                            .with_help("parameters must be identifiers"));
                    }
                };

                // Check for parameter modifiers (~ or <~)
                let kind = match &self.peek().kind {
                    TokenKind::Tilde => {
                        self.advance(); // consume ~
                        ParameterKind::Mutable
                    }
                    TokenKind::Return => {
                        self.advance(); // consume <~
                        ParameterKind::Output
                    }
                    _ => ParameterKind::Normal,
                };

                let param_span = param_token.span;
                parameters.push(Parameter::new(param_name, kind, param_span));

                // Check for comma (more parameters) or )
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.advance(); // consume ,
                    continue;
                } else {
                    break;
                }
            }
        }

        // Expect )
        let rparen_token = self.peek().clone();
        if !matches!(rparen_token.kind, TokenKind::RParen) {
            return Err(Diagnostic::error("expected ')' after parameters")
                .with_span(rparen_token.span)
                .with_help("function syntax: name(params) { }"));
        }
        self.advance(); // consume )

        // Parse body block
        let body = self.parse_block()?;

        let span = name_token.span.to(&body.span);

        Ok(Statement::FunctionDecl(FunctionDecl::new(
            name, parameters, body, span,
        )))
    }

    /// Parse return statement: <~ expr
    pub(crate) fn parse_return(&mut self) -> Result<Statement, Diagnostic> {
        let start_span = self.advance().span; // consume <~

        // Check for optional expression
        // Return is empty if followed by delimiter
        let value = if matches!(
            self.peek().kind,
            TokenKind::Newline | TokenKind::Backslash2 | TokenKind::RBrace | TokenKind::Eof
        ) {
            None
        } else {
            Some(Box::new(self.parse_expr()?))
        };

        let span = value
            .as_ref()
            .map(|v| start_span.to(&v.span()))
            .unwrap_or(start_span);

        Ok(Statement::Return(ReturnStmt::new(value, span)))
    }

    /// Parse function call as statement (discard return value)
    pub(crate) fn parse_function_call_statement(&mut self) -> Result<Statement, Diagnostic> {
        // Parse the function call as an expression
        let expr = self.parse_expr()?;
        let span = expr.span();

        match expr {
            Expr::FunctionCall(_) => {
                // Proper expression statement - evaluated for side effects, result discarded
                Ok(Statement::Expr(ExprStatement::new(expr, span)))
            }
            _ => Err(Diagnostic::error("expected function call")
                .with_span(span)
                .with_help("only function calls can be used as statements")),
        }
    }

    /// Parse lambda expression: x -> expr or (a, b) -> { block }
    pub(crate) fn parse_lambda(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.peek().span;

        // Track if we have a single-param lambda in parens
        let mut has_closing_paren = false;

        // Parse parameters
        let params = if matches!(self.peek().kind, TokenKind::LParen) {
            // Multi-param lambda: (a, b, c) -> expr OR single-param in parens: (x -> expr)
            self.advance(); // consume (
            let mut params = Vec::new();

            loop {
                if matches!(self.peek().kind, TokenKind::RParen) {
                    break;
                }

                // Expect identifier
                let param_token = self.peek().clone();
                let param_name = if let TokenKind::Ident(ref name) = param_token.kind {
                    name.clone()
                } else {
                    return Err(Diagnostic::error("expected parameter name in lambda")
                        .with_span(param_token.span)
                        .with_help("lambda parameters must be identifiers: (a, b) -> expr"));
                };
                self.advance(); // consume identifier
                params.push(param_name);

                // Check for comma (more params), closing paren, or arrow (single param in parens)
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.advance(); // consume ,
                } else if matches!(self.peek().kind, TokenKind::Arrow) {
                    // Single param in parens: (x -> expr)
                    // Mark that we need to consume closing paren later
                    has_closing_paren = true;
                    break;
                } else {
                    break;
                }
            }

            // Expect ) only if not followed by arrow (for multi-param case)
            if matches!(self.peek().kind, TokenKind::RParen) {
                self.advance(); // consume )
            } else if !matches!(self.peek().kind, TokenKind::Arrow) {
                return Err(Diagnostic::error("expected ')' after lambda parameters")
                    .with_span(self.peek().span)
                    .with_help("lambda syntax: (a, b) -> expr"));
            }

            params
        } else {
            // Single param lambda: x -> expr
            let param_token = self.peek().clone();
            let param_name = if let TokenKind::Ident(ref name) = param_token.kind {
                name.clone()
            } else {
                return Err(Diagnostic::error("expected parameter name in lambda")
                    .with_span(param_token.span)
                    .with_help("lambda syntax: x -> expr"));
            };
            self.advance(); // consume identifier
            vec![param_name]
        };

        // Expect ->
        if !matches!(self.peek().kind, TokenKind::Arrow) {
            return Err(Diagnostic::error("expected '->' in lambda expression")
                .with_span(self.peek().span)
                .with_help("lambda syntax: x -> expr or (a, b) -> expr"));
        }
        self.advance(); // consume ->

        // Parse body
        let (body, mut end_span) = if matches!(self.peek().kind, TokenKind::LBrace) {
            // Block lambda: x -> { <~ x * 2 }
            let block = self.parse_block()?;
            let end_span = block.span;
            (LambdaBody::Block(block), end_span)
        } else {
            // Simple lambda: x -> x * 2
            let expr = self.parse_expr()?;
            let end_span = expr.span();
            (LambdaBody::Expr(Box::new(expr)), end_span)
        };

        // If we have a closing paren for single-param lambda: (x -> expr)
        if has_closing_paren {
            let close_token = self.peek().clone();
            if matches!(close_token.kind, TokenKind::RParen) {
                self.advance(); // consume )
                end_span = close_token.span;
            }
        }

        let span = start.to(&end_span);

        Ok(Expr::Lambda(zymbol_ast::LambdaExpr {
            params,
            body,
            span,
        }))
    }
}
