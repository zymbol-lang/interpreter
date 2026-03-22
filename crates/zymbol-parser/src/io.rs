//! IO statement parsing for Zymbol-Lang
//!
//! Handles parsing of all IO-related statements:
//! - Output statements: >> expr1 expr2 ...
//! - Input statements: << variable OR << "prompt" variable
//! - Newline statements: ¶ OR \\
//! - CLI args capture: >< variable

use zymbol_ast::{CliArgsCaptureStmt, Expr, IdentifierExpr, Input, InputPrompt, LiteralExpr, Newline, Output};
use zymbol_common::Literal;
use zymbol_error::Diagnostic;
use zymbol_lexer::{StringPart, TokenKind};
use crate::{Parser, Statement};

impl Parser {
    /// Parse newline statement: ¶ or \\
    pub(crate) fn parse_newline(&mut self) -> Result<Statement, Diagnostic> {
        let span = self.advance().span; // consume ¶ or \\
        Ok(Statement::Newline(Newline::new(span)))
    }

    /// Parse input statement: << variable or << "prompt" variable
    pub(crate) fn parse_input(&mut self) -> Result<Statement, Diagnostic> {
        let start_span = self.advance().span; // consume <<

        // Check for optional string prompt: << "prompt" variable
        // The prompt can be a simple string or an interpolated string
        let prompt = if matches!(self.peek().kind, TokenKind::String(_) | TokenKind::StringInterpolated(_)) {
            let token = self.advance();
            match &token.kind {
                TokenKind::String(s) => Some(InputPrompt::Simple(s.clone())),
                TokenKind::StringInterpolated(parts) => {
                    Some(InputPrompt::Interpolated(parts.clone()))
                }
                _ => unreachable!(),
            }
        } else {
            None
        };

        // Parse variable name
        let var_token = self.peek().clone();
        let variable = match &var_token.kind {
            TokenKind::Ident(name) => {
                self.advance(); // consume identifier
                name.clone()
            }
            _ => {
                return Err(Diagnostic::error("expected variable name in input statement")
                    .with_span(var_token.span)
                    .with_help("input syntax: << variable or << \"prompt\" variable"));
            }
        };

        let span = start_span.to(&var_token.span);

        Ok(Statement::Input(Input::new(variable, prompt, span)))
    }

    /// Parse output statement: >> expr
    pub(crate) fn parse_output(&mut self) -> Result<Statement, Diagnostic> {
        let start_span = self.advance().span; // consume >>

        // Parse multiple expressions until delimiter (Haskell-style)
        let mut exprs = Vec::new();

        // Check if immediately followed by delimiter (allows >> ¶ or >> \\)
        if matches!(
            self.peek().kind,
            TokenKind::Newline | TokenKind::Backslash2 | TokenKind::RBrace | TokenKind::Eof
        ) {
            // Empty output (just >> with delimiter)
            return Ok(Statement::Output(Output::new(exprs, start_span)));
        }

        // Helper to expand interpolated strings or parse expression
        // Uses parse_postfix() instead of parse_expr() to support Haskell-style:
        // >> "Score: " -95 ¶  -> outputs "Score: -95" (two items)
        // For binary operations, use parentheses: >> "Result: " (1 + 2) ¶
        let parse_expr_with_interpolation = |parser: &mut Parser| -> Result<Vec<Expr>, Diagnostic> {
            let token = parser.peek().clone();

            match &token.kind {
                TokenKind::StringInterpolated(parts) => {
                    // Expand interpolated string to multiple expressions
                    parser.advance(); // consume interpolated string
                    let mut expanded = Vec::new();

                    for part in parts {
                        match part {
                            StringPart::Text(text) => {
                                expanded.push(Expr::Literal(LiteralExpr::new(
                                    Literal::String(text.clone()),
                                    token.span,
                                )));
                            }
                            StringPart::Variable(var_name) => {
                                expanded.push(Expr::Identifier(IdentifierExpr::new(
                                    var_name.clone(),
                                    token.span,
                                )));
                            }
                        }
                    }

                    Ok(expanded)
                }
                _ => {
                    // Use parse_output_item() to handle Haskell-style output:
                    // - Unary: -95, !flag, +x
                    // - Primary: literals, identifiers, arrays, tuples, (expr)
                    // - Postfix for identifiers: arr[0], obj.field, func()
                    // - But NOT postfix for literals: "text" [1,2,3] is two items
                    Ok(vec![parser.parse_output_item()?])
                }
            }
        };

        // Parse at least one expression (or expand interpolation)
        exprs.extend(parse_expr_with_interpolation(self)?);

        // Continue parsing expressions while not at delimiter or statement initiator
        loop {
            match &self.peek().kind {
                TokenKind::Newline | TokenKind::Backslash2 | TokenKind::RBrace | TokenKind::Eof | TokenKind::Semicolon => {
                    // Stop at delimiters
                    break;
                }
                TokenKind::Output => {
                    // Allow chaining: >> "a" >> "b" on same line
                    break;
                }
                // Statement-starting tokens - stop parsing output expressions
                TokenKind::Question        // if statement
                | TokenKind::DoubleQuestion // match statement
                | TokenKind::At            // loop/break/continue
                | TokenKind::Input         // input statement
                | TokenKind::Return        // return statement
                => {
                    break;
                }
                // For identifiers, check if followed by assignment operators (new statement)
                // NOTE: LParen (function call) is NOT a break — fn(args) is a valid output item.
                // Newlines already delimit statements, so fn(x) after a newline stops correctly.
                TokenKind::Ident(_) => {
                    if let Some(next) = self.peek_ahead(1) {
                        match next.kind {
                            TokenKind::Assign
                            | TokenKind::PlusAssign
                            | TokenKind::MinusAssign
                            | TokenKind::StarAssign
                            | TokenKind::SlashAssign
                            | TokenKind::PercentAssign
                            | TokenKind::PlusPlus
                            | TokenKind::MinusMinus
                            => {
                                // This starts a new statement, stop parsing output
                                break;
                            }
                            _ => {
                                // Otherwise, parse as expression (including function calls)
                                exprs.extend(parse_expr_with_interpolation(self)?);
                            }
                        }
                    } else {
                        // No next token, parse as expression
                        exprs.extend(parse_expr_with_interpolation(self)?);
                    }
                }
                _ => {
                    // Parse next expression (or expand interpolation)
                    exprs.extend(parse_expr_with_interpolation(self)?);
                }
            }
        }

        let end_span = exprs.last().unwrap().span();
        let span = start_span.to(&end_span);

        Ok(Statement::Output(Output::new(exprs, span)))
    }

    /// Parse CLI args capture statement: >< variable
    pub(crate) fn parse_cli_args_capture(&mut self) -> Result<Statement, Diagnostic> {
        let start_span = self.advance().span; // consume ><

        // Parse variable name
        let var_token = self.peek().clone();
        let variable_name = match &var_token.kind {
            TokenKind::Ident(name) => {
                self.advance(); // consume identifier
                name.clone()
            }
            _ => {
                return Err(Diagnostic::error("expected variable name after ><")
                    .with_span(var_token.span)
                    .with_help("CLI args capture syntax: ><variable_name"));
            }
        };

        let span = start_span.to(&var_token.span);

        Ok(Statement::CliArgsCapture(CliArgsCaptureStmt {
            variable_name,
            span,
        }))
    }
}

#[cfg(test)]
mod tests {
    use zymbol_ast::{Expr, InputPrompt, Program, Statement};
    use zymbol_common::Literal;
    use zymbol_error::Diagnostic;
    use zymbol_lexer::{Lexer, StringPart};
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
    fn test_parse_output() {
        let program = parse(">> \"Hello\"").expect("should parse");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Output(output) => {
                assert_eq!(output.exprs.len(), 1);
                match &output.exprs[0] {
                    Expr::Literal(lit) => match &lit.value {
                        Literal::String(s) => assert_eq!(s, "Hello"),
                        _ => panic!("Expected string literal"),
                    },
                    _ => panic!("Expected literal in output"),
                }
            }
            _ => panic!("Expected output statement"),
        }
    }

    #[test]
    fn test_parse_multiple_outputs() {
        let program = parse(">> \"Line 1\"\n>> \"Line 2\"").expect("should parse");
        assert_eq!(program.statements.len(), 2);
    }

    #[test]
    fn test_parse_empty_output() {
        // >> followed by EOF is now valid (empty output)
        let program = parse(">>").expect("should parse empty output");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Output(output) => {
                assert_eq!(output.exprs.len(), 0); // Empty output
            }
            _ => panic!("Expected output statement"),
        }
    }

    #[test]
    fn test_parse_identifier_in_output() {
        let program = parse(">> x").expect("should parse");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Output(output) => {
                assert_eq!(output.exprs.len(), 1);
                match &output.exprs[0] {
                    Expr::Identifier(ident) => assert_eq!(ident.name, "x"),
                    _ => panic!("Expected identifier"),
                }
            }
            _ => panic!("Expected output"),
        }
    }

    #[test]
    fn test_parse_haskell_style_concatenation() {
        // Test Haskell-style concatenation without commas
        let program = parse(">> \"Hello\" \" \" \"World\"").expect("should parse");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Output(output) => {
                assert_eq!(output.exprs.len(), 3); // Three separate expressions
            }
            _ => panic!("Expected output"),
        }
    }

    #[test]
    fn test_parse_mixed_expression() {
        // Test Haskell-style with literal and identifier
        let program = parse(">> \"Greeting: \" mensaje").expect("should parse");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Output(output) => {
                assert_eq!(output.exprs.len(), 2);
                // First should be literal
                assert!(matches!(output.exprs[0], Expr::Literal(_)));
                // Second should be identifier
                assert!(matches!(output.exprs[1], Expr::Identifier(_)));
            }
            _ => panic!("Expected output"),
        }
    }

    #[test]
    fn test_parse_input_simple() {
        // Test simple input without prompt
        let program = parse("<< edad").expect("should parse");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Input(input) => {
                assert_eq!(input.variable, "edad");
                assert!(input.prompt.is_none());
            }
            _ => panic!("Expected input"),
        }
    }

    #[test]
    fn test_parse_input_with_prompt() {
        // Test input with simple string prompt
        let program = parse("<< \"Enter age: \" edad").expect("should parse");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Input(input) => {
                assert_eq!(input.variable, "edad");
                match &input.prompt {
                    Some(InputPrompt::Simple(s)) => assert_eq!(s, "Enter age: "),
                    _ => panic!("Expected simple prompt"),
                }
            }
            _ => panic!("Expected input"),
        }
    }

    #[test]
    fn test_parse_input_with_interpolated_prompt() {
        // Test input with interpolated string prompt
        let program = parse("<< \"Enter hobby {name}: \" hobby").expect("should parse");
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Input(input) => {
                assert_eq!(input.variable, "hobby");
                match &input.prompt {
                    Some(InputPrompt::Interpolated(parts)) => {
                        assert_eq!(parts.len(), 3); // "Enter hobby " + {name} + ": "
                        assert!(matches!(&parts[0], StringPart::Text(s) if s == "Enter hobby "));
                        assert!(matches!(&parts[1], StringPart::Variable(v) if v == "name"));
                        assert!(matches!(&parts[2], StringPart::Text(s) if s == ": "));
                    }
                    _ => panic!("Expected interpolated prompt"),
                }
            }
            _ => panic!("Expected input"),
        }
    }
}
