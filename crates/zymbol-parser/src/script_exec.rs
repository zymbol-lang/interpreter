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
    pub(crate) fn parse_execute_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_token = self.advance(); // consume </

        // Parse the path (should be a string literal for now, or identifier for relative path)
        let path_token = self.peek().clone();
        let path = match &path_token.kind {
            TokenKind::String(s) => {
                self.advance(); // consume string
                s.clone()
            }
            TokenKind::Ident(name) => {
                // Support simple identifiers as paths (e.g., </ app.zy />)
                // We'll collect all path components until />
                let mut path_parts = vec![name.clone()];
                self.advance(); // consume identifier

                // Collect remaining path parts (dots, slashes, identifiers)
                while !matches!(self.peek().kind, TokenKind::ExecuteEnd | TokenKind::Eof) {
                    match &self.peek().kind {
                        TokenKind::Dot => {
                            path_parts.push(".".to_string());
                            self.advance();
                        }
                        TokenKind::Slash => {
                            path_parts.push("/".to_string());
                            self.advance();
                        }
                        TokenKind::Ident(part) => {
                            path_parts.push(part.clone());
                            self.advance();
                        }
                        _ => break,
                    }
                }

                path_parts.join("")
            }
            _ => {
                return Err(Diagnostic::error("expected file path after </")
                    .with_span(path_token.span)
                    .with_help("execute syntax: </ path.zy /> or </ \"path/to/file.zy\" />"));
            }
        };

        // Expect closing />
        let end_token = self.peek().clone();
        if !matches!(end_token.kind, TokenKind::ExecuteEnd) {
            return Err(Diagnostic::error("expected '/>' to close execute expression")
                .with_span(end_token.span)
                .with_help("execute syntax: </ path.zy />"));
        }
        self.advance(); // consume />

        let span = start_token.span.to(&end_token.span);
        Ok(Expr::Execute(ExecuteExpr {
            path,
            span,
        }))
    }

    /// Parse bash execute expression: <\ command \>
    pub(crate) fn parse_bash_exec_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_token = self.advance(); // consume <\

        // Collect all tokens until we find \>
        // Build the command string, being smart about spacing
        let mut command = String::new();
        let mut last_was_sticky_start = false; // Track if last was +, %, or : (sticky to NEXT)
        let mut inside_braces = false; // Track if we're inside {variable}

        while !matches!(self.peek().kind, TokenKind::BashEnd | TokenKind::Eof) {
            let token = self.advance();

            // Format start characters (+, %) stick to NEXT content
            let is_sticky_start = matches!(
                token.kind,
                TokenKind::Plus | TokenKind::Percent
            );

            // Format continuation characters (%, :, -) stick to PREVIOUS when after sticky content
            let is_continuation = matches!(
                token.kind,
                TokenKind::Percent | TokenKind::Colon | TokenKind::Minus
            ) && last_was_sticky_start;

            // % and . always glue to the previous token regardless of sticky state.
            // Fixes: date +%s%N → was "date +%s %N" (space before second %)
            // Fixes: date +%s.%N → was "date +%s .%N" (space before dot)
            // In bash exec context, % and . are almost always part of format strings or paths.
            let percent_always_glues = matches!(token.kind, TokenKind::Percent | TokenKind::Dot);

            // Content inside braces sticks together (no spaces)
            // But LBrace itself gets a space before it (unless it's the first token)
            let is_inside_braces = inside_braces && !matches!(token.kind, TokenKind::LBrace | TokenKind::RBrace);

            // Add space before token unless:
            // - It's the first token
            // - Last was a sticky start (for +%Y, %m, etc.)
            // - Current token is a continuation
            // - Current token is % (always glues to previous for date format strings)
            // - Current token is a special operator (pipe, and, or)
            // - We're inside braces (content between { and })
            // - Previous token was RBrace (no space after })
            let needs_space = !command.is_empty()
                && !last_was_sticky_start
                && !is_continuation
                && !percent_always_glues
                && !is_inside_braces
                && !matches!(token.kind, TokenKind::Pipe | TokenKind::And | TokenKind::Or | TokenKind::RBrace);

            if needs_space {
                command.push(' ');
            }

            // Update brace tracking AFTER spacing decision
            if matches!(token.kind, TokenKind::LBrace) {
                inside_braces = true;
            }
            if matches!(token.kind, TokenKind::RBrace) {
                inside_braces = false;
            }

            // Update state: sticky starts and continuations keep the chain going
            last_was_sticky_start = is_sticky_start || is_continuation;

            match &token.kind {
                TokenKind::String(s) => command.push_str(&format!("\"{}\"", s)),
                TokenKind::Ident(name) => command.push_str(name),
                TokenKind::Integer(n) => command.push_str(&n.to_string()),
                TokenKind::Minus => command.push('-'),
                TokenKind::Dot => command.push('.'),
                TokenKind::Slash => command.push('/'),
                TokenKind::Star => command.push('*'),
                TokenKind::Percent => command.push('%'),
                TokenKind::Plus => command.push('+'),
                TokenKind::Colon => command.push(':'),
                TokenKind::Pipe => {
                    command.push_str(" | ");
                    last_was_sticky_start = false;
                }
                TokenKind::Gt => command.push('>'),
                TokenKind::Lt => command.push('<'),
                TokenKind::Eq => command.push_str("=="),
                TokenKind::And => {
                    command.push_str(" && ");
                    last_was_sticky_start = false;
                }
                TokenKind::Or => {
                    command.push_str(" || ");
                    last_was_sticky_start = false;
                }
                TokenKind::LBrace => command.push('{'),
                TokenKind::RBrace => command.push('}'),
                _ => {
                    // For other tokens, convert to string representation
                    command.push_str(&format!("{:?}", token.kind));
                }
            }
        }

        if command.is_empty() {
            return Err(Diagnostic::error("expected bash command after <\\")
                .with_span(start_token.span)
                .with_help("bash execute syntax: <\\ command \\>"));
        }

        // Expect closing \>
        let end_token = self.peek().clone();
        if !matches!(end_token.kind, TokenKind::BashEnd) {
            return Err(Diagnostic::error("expected '\\>' to close bash execute expression")
                .with_span(end_token.span)
                .with_help("bash execute syntax: <\\ command \\>"));
        }
        self.advance(); // consume \>

        // Parse variable interpolation: {variable}
        let (parts, variables) = self.parse_bash_interpolation(&command)?;

        let span = start_token.span.to(&end_token.span);
        Ok(Expr::BashExec(BashExecExpr {
            parts,
            variables,
            span,
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
