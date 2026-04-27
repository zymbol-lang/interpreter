//! Script execution parsing for Zymbol-Lang
//!
//! Handles parsing of script execution expressions:
//! - Execute expressions: </ file.zy /> (runs Zymbol scripts)
//! - Bash exec expressions: <\ expr1 expr2 ... \> (runs shell commands)

use zymbol_ast::{BashExecExpr, Expr, ExecuteExpr};
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;

use crate::Parser;

impl Parser {
    /// Parse execute expression: </ file.zy />
    /// The lexer already consumed everything between </ and /> as a raw string.
    pub(crate) fn parse_execute_expr(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.advance(); // consume ExecuteCommand token

        let path = match &token.kind {
            TokenKind::ExecuteCommand(raw) => raw.clone(),
            _ => unreachable!("parse_execute_expr called on non-ExecuteCommand token"),
        };

        if path.is_empty() {
            return Err(Diagnostic::error("expected file path after </")
                .with_span(token.span)
                .with_help("execute syntax: </ path.zy />"));
        }

        // Strip surrounding quotes if user wrote </ "path.zy" />
        let path = if path.starts_with('"') && path.ends_with('"') && path.len() > 1 {
            path[1..path.len() - 1].to_string()
        } else {
            path
        };

        Ok(Expr::Execute(ExecuteExpr {
            path,
            span: token.span,
        }))
    }

    /// Parse bash execute expression: <\ expr1 expr2 ... \>
    /// Content is tokenized normally; expressions are evaluated and concatenated.
    /// Bare identifiers are variable references; string/char literals are literal.
    pub(crate) fn parse_bash_exec_expr(&mut self) -> Result<Expr, Diagnostic> {
        let open_token = self.advance(); // consume BashOpen token

        if !matches!(open_token.kind, TokenKind::BashOpen) {
            unreachable!("parse_bash_exec_expr called on non-BashOpen token");
        }

        let mut args = Vec::new();

        loop {
            if matches!(self.peek().kind, TokenKind::BashClose | TokenKind::Eof) {
                break;
            }
            args.push(self.parse_expr()?);
        }

        let close_span = self.peek().span;
        if matches!(self.peek().kind, TokenKind::BashClose) {
            self.advance(); // consume BashClose
        } else {
            return Err(Diagnostic::error("unterminated bash execute expression")
                .with_span(open_token.span)
                .with_help("bash execute syntax: <\\ expr1 expr2 ... \\>"));
        }

        let span = open_token.span.to(&close_span);

        Ok(Expr::BashExec(BashExecExpr { args, span }))
    }
}
