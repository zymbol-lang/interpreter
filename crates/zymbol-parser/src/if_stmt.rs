//! IF statement parsing for Zymbol-Lang
//!
//! Handles parsing of conditional statements:
//! - IF: ? condition { }
//! - ELSE-IF: _? condition { }
//! - ELSE: _ { }

use zymbol_ast::{ElseIfBranch, IfStmt, Statement};
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;
use crate::Parser;

impl Parser {
    /// Parse IF statement: ? condition { } _? condition { } _ { }
    pub(crate) fn parse_if(&mut self) -> Result<Statement, Diagnostic> {
        let start_span = self.advance().span; // consume ?

        // Parse condition
        let condition = Box::new(self.parse_expr()?);

        // Parse then block
        let then_block = self.parse_block()?;

        // Parse else-if branches
        let mut else_if_branches = Vec::new();
        while matches!(self.peek().kind, TokenKind::ElseIf) {
            let else_if_start = self.advance().span; // consume _?

            // Parse else-if condition
            let else_if_condition = Box::new(self.parse_expr()?);

            // Parse else-if block
            let else_if_block = self.parse_block()?;

            let else_if_span = else_if_start.to(&else_if_block.span);
            else_if_branches.push(ElseIfBranch::new(
                else_if_condition,
                else_if_block,
                else_if_span,
            ));
        }

        // Check for else block
        let else_block = if matches!(self.peek().kind, TokenKind::Underscore) {
            self.advance(); // consume _
            Some(self.parse_block()?)
        } else {
            None
        };

        let end_span = else_block
            .as_ref()
            .map(|b| b.span)
            .or_else(|| else_if_branches.last().map(|b| b.span))
            .unwrap_or(then_block.span);
        let span = start_span.to(&end_span);

        Ok(Statement::If(IfStmt::new(
            condition,
            then_block,
            else_if_branches,
            else_block,
            span,
        )))
    }
}
