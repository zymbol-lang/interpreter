//! Script execution parsing for Zymbol-Lang
//!
//! Handles parsing of script execution expressions:
//! - Execute expressions: </ file.zy /> (runs Zymbol scripts)
//! - Bash exec expressions: <\ command \> (runs shell commands with interpolation)

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

    /// Parse bash execute expression: <\ command \>
    /// The lexer already consumed everything between <\ and \> as a raw string,
    /// so here we just extract the command and parse variable interpolation.
    pub(crate) fn parse_bash_exec_expr(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.advance(); // consume BashCommand token

        let command = match &token.kind {
            TokenKind::BashCommand(raw) => raw.clone(),
            _ => unreachable!("parse_bash_exec_expr called on non-BashCommand token"),
        };

        if command.is_empty() {
            return Err(Diagnostic::error("expected bash command after <\\")
                .with_span(token.span)
                .with_help("bash execute syntax: <\\ command \\>"));
        }

        let (parts, variables) = self.parse_bash_interpolation(&command)?;

        Ok(Expr::BashExec(BashExecExpr {
            parts,
            variables,
            span: token.span,
        }))
    }

    /// Parse variable interpolation in bash commands
    /// Splits "cat {file} | head -n {count}" into:
    /// - parts: ["cat ", " | head -n ", ""]
    /// - variables: ["file", "count"]
    fn parse_bash_interpolation(&self, command: &str) -> Result<(Vec<String>, Vec<String>), Diagnostic> {
        let mut parts = Vec::new();
        let mut variables = Vec::new();
        let mut current_part = String::new();
        let mut in_variable = false;
        let mut variable_name = String::new();

        let chars: Vec<char> = command.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];

            if ch == '{' && !in_variable {
                // Start of variable interpolation
                in_variable = true;
                variable_name.clear();
                i += 1;
                continue;
            }

            if ch == '}' && in_variable {
                // End of variable interpolation
                in_variable = false;
                if variable_name.is_empty() {
                    return Err(Diagnostic::error("empty variable name in bash interpolation")
                        .with_help("use {variable_name} for interpolation"));
                }
                variables.push(variable_name.clone());
                parts.push(current_part.clone());
                current_part.clear();
                variable_name.clear();
                i += 1;
                continue;
            }

            if in_variable {
                // Inside variable name
                if ch.is_alphanumeric() || ch == '_' {
                    variable_name.push(ch);
                } else {
                    return Err(Diagnostic::error(format!("invalid character '{}' in variable name", ch))
                        .with_help("variable names must contain only alphanumeric characters and underscores"));
                }
            } else {
                // Regular command text
                current_part.push(ch);
            }

            i += 1;
        }

        if in_variable {
            return Err(Diagnostic::error("unclosed variable interpolation")
                .with_help("make sure all { have matching }"));
        }

        // Add the final part (even if empty)
        parts.push(current_part);

        Ok((parts, variables))
    }
}
