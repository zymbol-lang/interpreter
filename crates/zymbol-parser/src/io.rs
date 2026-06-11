//! IO statement parsing for Zymbol-Lang
//!
//! Handles parsing of all IO-related statements:
//! - Output statements: >> expr1 expr2 ...
//! - Input statements: << variable OR << "prompt" variable
//! - Newline statements: ¶ OR \\
//! - CLI args capture: >< variable

use zymbol_ast::{ClearScreen, CliArgsCaptureStmt, Expr, Input, InputCast, InputPrompt, KeyInput, LiteralExpr, Newline, Output, OutputPos, TuiBlock};
use zymbol_common::Literal;
use zymbol_error::Diagnostic;
use zymbol_lexer::{StringPart, TokenKind};
use crate::{Parser, Statement};

impl Parser {
    /// Parse newline statement: ¶ or \\
    pub(crate) fn parse_newline(&mut self) -> Result<Statement, Diagnostic> {
        let token = self.advance(); // consume ¶ or \\
        let stmt = if matches!(token.kind, TokenKind::Backslash2) {
            Newline::new_backslash(token.span)
        } else {
            Newline::new(token.span)
        };
        Ok(Statement::Newline(stmt))
    }

    /// Parse input statement:
    ///   << variable                — store raw string
    ///   << #|variable|             — store as numeric (int/float)
    ///   << "prompt" variable
    ///   << "prompt" #|variable|
    ///   << <typespec> "prompt" var — typed/validated input (re-prompts until valid)
    ///       where <typespec> ∈ { ##. , ##.(t,d) , ### , ###(n) , ##"(n) , ##' }
    pub(crate) fn parse_input(&mut self) -> Result<Statement, Diagnostic> {
        let start_span = self.advance().span; // consume <<

        // Optional leading typespec cast: ##. / ##.(t,d) / ### / ###(n) / ##"(n) / ##'
        let typespec = self.parse_input_typespec()?;

        // Optional string prompt: << [typespec] "prompt" ...
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

        // Legacy `#|variable|` numeric cast — only when no typespec was given.
        let legacy_numeric = typespec.is_none() && matches!(self.peek().kind, TokenKind::HashPipe);
        if legacy_numeric {
            self.advance(); // consume #|
        }

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
                    .with_help("input syntax: << var  |  << #|var|  |  << \"prompt\" var  |  << ##.(5,2) \"prompt\" var"));
            }
        };

        // If legacy numeric cast, consume closing `|`
        if legacy_numeric {
            let pipe_tok = self.peek().clone();
            if !matches!(pipe_tok.kind, TokenKind::Pipe) {
                return Err(Diagnostic::error("expected '|' to close #|variable|")
                    .with_span(pipe_tok.span)
                    .with_help("numeric input syntax: << #|variable|"));
            }
            self.advance(); // consume |
        }

        let cast = match typespec {
            Some(c) => c,
            None if legacy_numeric => InputCast::Numeric,
            None => InputCast::String,
        };

        let span = start_span.to(&var_token.span);
        Ok(Statement::Input(Input::new(variable, prompt, cast, span)))
    }

    /// Parse an optional input typespec immediately after `<<`:
    ///   ##.        → Float
    ///   ##.(t,d)   → Decimal { total: t, decimals: d }
    ///   ###  / ###(n)  → Int { max_digits }
    ///   ##!  / ##!(n)  → Int { max_digits }   (truncate alias, same parse)
    ///   ##"  / ##"(n)  → Text { max }
    ///   ##'        → Char
    /// Returns `Ok(None)` when the next token is not a typespec.
    fn parse_input_typespec(&mut self) -> Result<Option<InputCast>, Diagnostic> {
        let cast = match self.peek().kind {
            TokenKind::HashHashDot => {
                self.advance(); // consume ##.
                if matches!(self.peek().kind, TokenKind::LParen) {
                    let (total, decimals) = self.parse_two_uint_args()?;
                    InputCast::Decimal { total, decimals }
                } else {
                    InputCast::Float
                }
            }
            TokenKind::HashHashHash | TokenKind::HashHashBang => {
                self.advance(); // consume ### / ##!
                InputCast::Int { max_digits: self.parse_opt_one_uint_arg()? }
            }
            TokenKind::HashHashQuote => {
                self.advance(); // consume ##"
                InputCast::Text { max: self.parse_opt_one_uint_arg()? }
            }
            TokenKind::HashHashApos => {
                self.advance(); // consume ##'
                InputCast::Char
            }
            _ => return Ok(None),
        };
        Ok(Some(cast))
    }

    /// Parse an optional single unsigned-int argument in parentheses: `(N)`.
    /// Returns `None` when no `(` follows.
    fn parse_opt_one_uint_arg(&mut self) -> Result<Option<u32>, Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::LParen) {
            return Ok(None);
        }
        self.advance(); // consume (
        let n = self.expect_uint_arg()?;
        self.expect_rparen()?;
        Ok(Some(n))
    }

    /// Parse a required two unsigned-int arguments in parentheses: `(A, B)`.
    fn parse_two_uint_args(&mut self) -> Result<(u32, u32), Diagnostic> {
        self.advance(); // consume (  (caller already checked it)
        let a = self.expect_uint_arg()?;
        let comma = self.peek().clone();
        if !matches!(comma.kind, TokenKind::Comma) {
            return Err(Diagnostic::error("expected ',' between the two size arguments")
                .with_span(comma.span)
                .with_help("decimal typespec syntax: ##.(total, decimals)"));
        }
        self.advance(); // consume ,
        let b = self.expect_uint_arg()?;
        self.expect_rparen()?;
        Ok((a, b))
    }

    /// Consume one non-negative integer literal as `u32`.
    fn expect_uint_arg(&mut self) -> Result<u32, Diagnostic> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Integer(n) if n >= 0 => {
                self.advance();
                Ok(n as u32)
            }
            _ => Err(Diagnostic::error("expected a non-negative integer size argument")
                .with_span(tok.span)
                .with_help("input typespec sizes are non-negative integers, e.g. ##.(5,2) or ###(4)")),
        }
    }

    /// Consume a closing `)`.
    fn expect_rparen(&mut self) -> Result<(), Diagnostic> {
        let tok = self.peek().clone();
        if !matches!(tok.kind, TokenKind::RParen) {
            return Err(Diagnostic::error("expected ')' to close the size argument list")
                .with_span(tok.span));
        }
        self.advance(); // consume )
        Ok(())
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
                    // Keep the interpolated string as ONE literal (same as in
                    // expression context, lib.rs primary parsing) so the
                    // formatter can reprint exactly what the user wrote. The
                    // interpreter/VM resolve `{var}` at runtime and produce
                    // the same concatenation the old per-part expansion did.
                    parser.advance(); // consume interpolated string
                    let mut reconstructed = String::new();
                    for part in parts {
                        match part {
                            StringPart::Text(t) => reconstructed.push_str(t),
                            StringPart::Variable(v) => {
                                reconstructed.push('{');
                                reconstructed.push_str(v);
                                reconstructed.push('}');
                            }
                        }
                    }
                    Ok(vec![Expr::Literal(LiteralExpr::new(
                        Literal::InterpolatedString(reconstructed),
                        token.span,
                    ))])
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
                | TokenKind::AtLabel(_)       // labeled loop (legacy)
                | TokenKind::AtColonLabel(_) // labeled loop
                | TokenKind::AtTilde       // sleep statement
                | TokenKind::Input         // input statement
                | TokenKind::KeyBlock      // blocking key input
                | TokenKind::KeyNonBlock   // non-blocking key input
                | TokenKind::OutputClear   // clear screen
                | TokenKind::OutputPos     // positioned output
                | TokenKind::OutputGate    // TUI block
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
                            | TokenKind::CaretAssign
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
    /// Parse clear screen: >>!
    pub(crate) fn parse_clear_screen(&mut self) -> Result<Statement, Diagnostic> {
        let span = self.advance().span; // consume >>!
        Ok(Statement::ClearScreen(ClearScreen::new(span)))
    }

    /// Parse key input: <<| var (blocking) or <<|? var (non-blocking)
    pub(crate) fn parse_key_input(&mut self, blocking: bool) -> Result<Statement, Diagnostic> {
        let start_span = self.advance().span; // consume <<| or <<|?
        let var_token = self.peek().clone();
        let variable = match &var_token.kind {
            TokenKind::Ident(name) => { self.advance(); name.clone() }
            _ => return Err(Diagnostic::error("expected variable name after key input operator")
                .with_span(var_token.span)
                .with_help(if blocking { "syntax: <<| var" } else { "syntax: <<|? var" })),
        };
        let span = start_span.to(&var_token.span);
        Ok(Statement::KeyInput(KeyInput::new(variable, blocking, span)))
    }

    /// Parse positioned output: >>~ (fila, col, BKS, fg, bg) > items
    /// Sparse inline: >>~(,,,15,0)> — commas as position markers, empty slot = absent
    pub(crate) fn parse_output_pos(&mut self) -> Result<Statement, Diagnostic> {
        let start_span = self.advance().span; // consume >>~

        let parenthesized = matches!(self.peek().kind, TokenKind::LParen);
        let slots: Vec<Option<Expr>> = if parenthesized {
            self.parse_sparse_pos_tuple()?
        } else if matches!(self.peek().kind, TokenKind::Ident(_)) {
            // Variable evaluated at runtime as dense tuple
            let expr = self.parse_postfix()?;
            vec![Some(expr)]
        } else {
            let t = self.peek().clone();
            return Err(Diagnostic::error("expected '(' or variable after >>~")
                .with_span(t.span)
                .with_help("syntax: >>~ (fila, col [, BKS [, fg [, bg]]]) > items"));
        };

        let gt = self.peek().clone();
        if !matches!(gt.kind, TokenKind::Gt) {
            return Err(Diagnostic::error("expected '>' after >>~ position")
                .with_span(gt.span)
                .with_help("syntax: >>~ (fila, col) > items"));
        }
        let gt_span = self.advance().span; // consume >
        let items = self.parse_output_items_same_line(gt_span.start.line)?;
        let end_span = items.last().map(|e| e.span()).unwrap_or(gt_span);
        let ctor = if parenthesized { OutputPos::new } else { OutputPos::new_bare };
        Ok(Statement::OutputPos(ctor(slots, items, start_span.to(&end_span))))
    }

    /// Parse sparse position tuple: (slot0, slot1, slot2, slot3, slot4)
    /// Each slot may be empty (absent). Max 5 slots: [fila, col, BKS, fg, bg].
    fn parse_sparse_pos_tuple(&mut self) -> Result<Vec<Option<Expr>>, Diagnostic> {
        self.advance(); // consume (
        let mut slots: Vec<Option<Expr>> = Vec::new();

        loop {
            match self.peek().kind.clone() {
                TokenKind::RParen => {
                    self.advance(); // consume )
                    break;
                }
                TokenKind::Comma => {
                    slots.push(None); // absent slot
                    self.advance();   // consume ,
                }
                _ => {
                    let expr = self.parse_expr()?;
                    slots.push(Some(expr));
                    if matches!(self.peek().kind, TokenKind::Comma) {
                        self.advance(); // consume ,
                    }
                }
            }
            if slots.len() > 5 {
                let t = self.peek().clone();
                return Err(Diagnostic::error(
                    ">>~ position tuple has at most 5 slots: (fila, col, BKS, fg, bg)",
                )
                .with_span(t.span));
            }
        }
        Ok(slots)
    }

    /// Parse TUI block: >>| { statements }
    pub(crate) fn parse_tui_block(&mut self) -> Result<Statement, Diagnostic> {
        let start_span = self.advance().span; // consume >>|
        if !matches!(self.peek().kind, TokenKind::LBrace) {
            let t = self.peek().clone();
            return Err(Diagnostic::error("expected '{' after >>|")
                .with_span(t.span)
                .with_help("TUI block syntax: >>| { statements }"));
        }
        let body = self.parse_block()?;
        let span = start_span.to(&body.span);
        Ok(Statement::TuiBlock(TuiBlock::new(body, span)))
    }

    /// Parse items for >>~ positioned output; stops at end of source line, ¶, \\, } or EOF.
    /// Uses the source line of the >>~ token to detect line boundaries (the lexer discards \n).
    /// Used by >>~ so that consecutive positioned-output statements don't merge into one.
    pub(crate) fn parse_output_items_same_line(&mut self, line: u32) -> Result<Vec<Expr>, Diagnostic> {
        let mut items = Vec::new();
        loop {
            if matches!(
                self.peek().kind,
                TokenKind::Newline | TokenKind::Backslash2 | TokenKind::RBrace | TokenKind::Eof
            ) { break; }
            if self.peek().span.start.line != line {
                break;
            }
            items.push(self.parse_output_item()?);
        }
        Ok(items)
    }

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
    use zymbol_common::{BinaryOp, Literal};
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

    #[test]
    fn test_parse_output_subtraction() {
        // >> a - b ¶ must parse as a single Binary(Sub) expression, not two items
        let program = parse(">> a - b").expect("should parse");
        match &program.statements[0] {
            Statement::Output(output) => {
                assert_eq!(output.exprs.len(), 1, "a - b must be one item");
                assert!(
                    matches!(&output.exprs[0], Expr::Binary(b) if b.op == BinaryOp::Sub),
                    "expected Binary(Sub)"
                );
            }
            _ => panic!("Expected output"),
        }
    }

    #[test]
    fn test_parse_output_power() {
        // >> a ^ b ¶ must parse as a single Binary(Pow) expression
        let program = parse(">> a ^ b").expect("should parse");
        match &program.statements[0] {
            Statement::Output(output) => {
                assert_eq!(output.exprs.len(), 1, "a ^ b must be one item");
                assert!(
                    matches!(&output.exprs[0], Expr::Binary(b) if b.op == BinaryOp::Pow),
                    "expected Binary(Pow)"
                );
            }
            _ => panic!("Expected output"),
        }
    }

    #[test]
    fn test_parse_output_precedence() {
        // >> a - b * c  must parse as a - (b*c), i.e. Sub(a, Mul(b,c))
        let program = parse(">> a - b * c").expect("should parse");
        match &program.statements[0] {
            Statement::Output(output) => {
                assert_eq!(output.exprs.len(), 1);
                match &output.exprs[0] {
                    Expr::Binary(sub) => {
                        assert_eq!(sub.op, BinaryOp::Sub);
                        assert!(matches!(*sub.right, Expr::Binary(ref m) if m.op == BinaryOp::Mul));
                    }
                    _ => panic!("expected Sub at top level"),
                }
            }
            _ => panic!("Expected output"),
        }
    }

    #[test]
    fn test_parse_output_unary_still_works() {
        // >> -5  must still parse as a single unary-minus item (not broken by sub fix)
        let program = parse(">> -5").expect("should parse");
        match &program.statements[0] {
            Statement::Output(output) => {
                assert_eq!(output.exprs.len(), 1, "-5 must be one item");
                assert!(matches!(&output.exprs[0], Expr::Unary(_)), "expected Unary");
            }
            _ => panic!("Expected output"),
        }
    }

    #[test]
    fn test_parse_output_juxtaposition_unaffected() {
        // >> "label" value  must still produce two separate items (Haskell-style)
        let program = parse(">> \"label\" value").expect("should parse");
        match &program.statements[0] {
            Statement::Output(output) => {
                assert_eq!(output.exprs.len(), 2, "juxtaposition must still produce two items");
            }
            _ => panic!("Expected output"),
        }
    }
}
