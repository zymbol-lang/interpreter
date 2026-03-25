//! Expression parsing for Zymbol-Lang
//!
//! Handles parsing of all expression types following operator precedence:
//! 1. Pipe operator (|>)
//! 2. Logical OR (||)
//! 3. Logical AND (&&)
//! 4. Comparison (==, <>, <, >, <=, >=)
//! 5. Addition/Subtraction (+, -)
//! 6. Multiplication/Division/Modulo (*, /, %)
//! 7. Power (^) - right-associative
//! 8. Unary operators (!, -, +)

use zymbol_ast::{BinaryExpr, Expr, UnaryExpr};
use zymbol_common::BinaryOp;
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;
use crate::Parser;

impl Parser {
    /// Parse expression (entry point)
    pub(crate) fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        // No longer parses comma expressions - use multiple expressions in Output instead
        // Parse pipe expressions which have higher precedence than logic_or
        self.parse_pipe()
    }

    /// Parse pipe expression: expr |> func(_)
    pub(crate) fn parse_pipe(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_logic_or()?;

        // Handle pipe operator: left |> callable(args)
        while matches!(self.peek().kind, TokenKind::PipeOp) {
            self.advance(); // consume |>

            // Parse the callable (identifier, lambda, member access, but NOT function calls)
            // We need to parse postfix operations like indexing and member access, but NOT function calls
            let callable = self.parse_postfix_without_calls()?;

            // Expect function call syntax: callable(args)
            if !matches!(self.peek().kind, TokenKind::LParen) {
                return Err(Diagnostic::error("expected '(' after pipe operator")
                    .with_span(self.peek().span)
                    .with_help("pipe syntax: value |> func(_) or value |> (x -> x * 2)(_)"));
            }
            self.advance(); // consume (

            // Parse arguments with _ placeholders
            let mut arguments = Vec::new();

            if !matches!(self.peek().kind, TokenKind::RParen) {
                loop {
                    // Check for _ placeholder
                    if matches!(self.peek().kind, TokenKind::Underscore) {
                        self.advance(); // consume _
                        arguments.push(zymbol_ast::PipeArg::Placeholder);
                    } else {
                        // Regular expression argument
                        let arg_expr = self.parse_logic_or()?;
                        arguments.push(zymbol_ast::PipeArg::Expr(arg_expr));
                    }

                    if matches!(self.peek().kind, TokenKind::Comma) {
                        self.advance(); // consume ,
                    } else {
                        break;
                    }
                }
            }

            // Expect )
            let rparen_token = self.peek().clone();
            if !matches!(rparen_token.kind, TokenKind::RParen) {
                return Err(Diagnostic::error("expected ')' after pipe arguments")
                    .with_span(rparen_token.span)
                    .with_help("pipe syntax: value |> func(_)"));
            }
            self.advance(); // consume )

            let span = left.span().to(&rparen_token.span);

            // Create pipe expression
            left = Expr::Pipe(zymbol_ast::PipeExpr {
                left: Box::new(left),
                callable: Box::new(callable),
                arguments,
                span,
            });
        }

        Ok(left)
    }

    /// Parse logical OR expression: ||
    pub(crate) fn parse_logic_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_logic_and()?;

        while matches!(self.peek().kind, TokenKind::Or) {
            let _op_token = self.advance();
            let right = self.parse_logic_and()?;
            let span = left.span().to(&right.span());

            left = Expr::Binary(BinaryExpr::new(
                BinaryOp::Or,
                Box::new(left),
                Box::new(right),
                span,
            ));
        }

        Ok(left)
    }

    /// Parse logical AND expression: &&
    pub(crate) fn parse_logic_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_comparison()?;

        while matches!(self.peek().kind, TokenKind::And) {
            let _op_token = self.advance();
            let right = self.parse_comparison()?;
            let span = left.span().to(&right.span());

            left = Expr::Binary(BinaryExpr::new(
                BinaryOp::And,
                Box::new(left),
                Box::new(right),
                span,
            ));
        }

        Ok(left)
    }

    /// Parse comparison expression: ==, <>, <, >, <=, >=
    pub(crate) fn parse_comparison(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_addition()?;

        while matches!(
            self.peek().kind,
            TokenKind::Eq | TokenKind::Neq | TokenKind::Lt | TokenKind::Gt | TokenKind::Le | TokenKind::Ge
        ) {
            let op_token = self.advance();
            let op = match op_token.kind {
                TokenKind::Eq => BinaryOp::Eq,
                TokenKind::Neq => BinaryOp::Neq,
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::Le => BinaryOp::Le,
                TokenKind::Ge => BinaryOp::Ge,
                _ => unreachable!(),
            };

            let right = self.parse_addition()?;
            let span = left.span().to(&right.span());

            left = Expr::Binary(BinaryExpr::new(op, Box::new(left), Box::new(right), span));
        }

        Ok(left)
    }

    /// Parse addition/subtraction: +, -
    pub(crate) fn parse_addition(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_multiplication()?;

        while matches!(self.peek().kind, TokenKind::Plus | TokenKind::Minus) {
            let op_token = self.advance();
            let op = match op_token.kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => unreachable!(),
            };

            let right = self.parse_multiplication()?;
            let span = left.span().to(&right.span());

            left = Expr::Binary(BinaryExpr::new(op, Box::new(left), Box::new(right), span));
        }

        Ok(left)
    }

    /// Parse multiplication/division/modulo: *, /, %
    pub(crate) fn parse_multiplication(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_power()?;

        while matches!(
            self.peek().kind,
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent
        ) {
            let op_token = self.advance();
            let op = match op_token.kind {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                _ => unreachable!(),
            };

            let right = self.parse_power()?;
            let span = left.span().to(&right.span());

            left = Expr::Binary(BinaryExpr::new(op, Box::new(left), Box::new(right), span));
        }

        Ok(left)
    }

    /// Parse power/exponentiation: ^ (right-associative)
    pub(crate) fn parse_power(&mut self) -> Result<Expr, Diagnostic> {
        let left = self.parse_range()?;

        // Power is right-associative: 2^3^4 = 2^(3^4) not (2^3)^4
        if matches!(self.peek().kind, TokenKind::Caret) {
            self.advance(); // consume ^
            let right = self.parse_power()?; // recursive call for right-associativity
            let span = left.span().to(&right.span());

            Ok(Expr::Binary(BinaryExpr::new(
                BinaryOp::Pow,
                Box::new(left),
                Box::new(right),
                span,
            )))
        } else {
            Ok(left)
        }
    }

    /// Parse unary operators: !, -, +
    pub(crate) fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();

        match token.kind {
            TokenKind::Not => {
                self.advance(); // consume !
                let operand = self.parse_unary()?; // recursive for chained unary: !!x
                let span = token.span.to(&operand.span());

                Ok(Expr::Unary(UnaryExpr::new(
                    zymbol_common::UnaryOp::Not,
                    Box::new(operand),
                    span,
                )))
            }
            TokenKind::Minus => {
                self.advance(); // consume -
                let operand = self.parse_unary()?; // recursive for chained unary: --x
                let span = token.span.to(&operand.span());

                Ok(Expr::Unary(UnaryExpr::new(
                    zymbol_common::UnaryOp::Neg,
                    Box::new(operand),
                    span,
                )))
            }
            TokenKind::Plus => {
                self.advance(); // consume +
                let operand = self.parse_unary()?; // recursive for chained unary: ++x
                let span = token.span.to(&operand.span());

                Ok(Expr::Unary(UnaryExpr::new(
                    zymbol_common::UnaryOp::Pos,
                    Box::new(operand),
                    span,
                )))
            }
            _ => self.parse_primary_expr(),
        }
    }

    /// Parse an output item for Haskell-style output: >> expr1 expr2 ...
    ///
    /// This method handles:
    /// - Unary operators: -95, !flag
    /// - Primary expressions: literals, identifiers, arrays, tuples, (expr)
    /// - Postfix for identifiers: arr[0], obj.field, func()
    /// - String concatenation with +: "a" + 5 works
    /// - But NOT other binary ops: "Score: " -95 is two items
    ///
    /// This allows: >> "Score: " -95 ¶     (two separate items)
    /// And also:    >> "i=" + i ¶           (concatenation)
    /// And also:    >> arr[0] ¶             (indexed access)
    pub(crate) fn parse_output_item(&mut self) -> Result<Expr, Diagnostic> {
        // Addition/subtraction level — lowest binary precedence in output context.
        // Supports numeric arithmetic: >> "Suma: " 10 + 5 ¶  (single item = 15)
        //                              >> a - b ¶             (single item = a-b)
        // Note: + with strings is a type error; use juxtaposition for multi-value output.
        let mut expr = self.parse_output_item_mul()?;

        while matches!(self.peek().kind, TokenKind::Plus | TokenKind::Minus) {
            let op_token = self.advance(); // consume + or -
            let op = match op_token.kind {
                TokenKind::Plus  => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => unreachable!(),
            };
            let right = self.parse_output_item_mul()?;
            let span = expr.span().to(&right.span());
            expr = Expr::Binary(BinaryExpr::new(op, Box::new(expr), Box::new(right), span));
        }

        Ok(expr)
    }

    /// Multiplication/division level for output items (higher precedence than +/-)
    /// Supports: >> "Result: " 10 * 5 ¶  → outputs "Result: 50"
    fn parse_output_item_mul(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();

        // Handle unary operators (- and !) at the start of an item: >> -5 ¶  >> !flag ¶
        if matches!(token.kind, TokenKind::Not | TokenKind::Minus) {
            return self.parse_unary();
        }

        // Use parse_output_item_term as base so ^ (power) is handled with correct
        // precedence on both sides of * / %
        let mut expr = self.parse_output_item_term()?;

        // Allow * / % with proper precedence
        while matches!(self.peek().kind, TokenKind::Star | TokenKind::Slash | TokenKind::Percent) {
            let op_token = self.advance();
            let op = match op_token.kind {
                TokenKind::Star    => BinaryOp::Mul,
                TokenKind::Slash   => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                _ => unreachable!(),
            };
            let right = self.parse_output_item_term()?;
            let span = expr.span().to(&right.span());
            expr = Expr::Binary(BinaryExpr::new(op, Box::new(expr), Box::new(right), span));
        }

        Ok(expr)
    }

    /// Parse a power-level term for output items (highest binary precedence).
    /// Handles primary expressions, postfix ops, and ^ (right-associative).
    fn parse_output_item_term(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();

        // Handle unary - for negative numbers at this level: a * -b
        if matches!(token.kind, TokenKind::Minus) {
            return self.parse_unary();
        }

        // Parse primary
        let mut expr = self.parse_primary_expr()?;

        // Handle postfix operations (collection ops, indexing, member access, calls)
        expr = self.parse_output_item_postfix(expr)?;

        // Handle ^ (power) — right-associative, highest binary precedence.
        // Right-recursive call so 2^3^4 = 2^(3^4).
        if matches!(self.peek().kind, TokenKind::Caret) {
            self.advance(); // consume ^
            let right = self.parse_output_item_term()?;
            let span = expr.span().to(&right.span());
            expr = Expr::Binary(BinaryExpr::new(
                BinaryOp::Pow,
                Box::new(expr),
                Box::new(right),
                span,
            ));
        }

        Ok(expr)
    }

    /// Parse postfix operations for output items.
    /// Handles structural ops ([], ., ()) and all collection operators ($#, $+, $-, etc.)
    /// so they can be used directly in >> without extra parentheses.
    fn parse_output_item_postfix(&mut self, mut expr: Expr) -> Result<Expr, Diagnostic> {
        use zymbol_ast::{IndexExpr, MemberAccessExpr, FunctionCallExpr, TypeMetadataExpr};

        loop {
            let token = self.peek().clone();
            match token.kind {
                // ── Structural postfix ────────────────────────────────────────
                TokenKind::LBracket => {
                    self.advance(); // consume [
                    let index = self.parse_expr()?;

                    let close_token = self.peek().clone();
                    if !matches!(close_token.kind, TokenKind::RBracket) {
                        return Err(Diagnostic::error("expected ']' after index")
                            .with_span(close_token.span)
                            .with_help("array indexing must use brackets: arr[index]"));
                    }
                    self.advance(); // consume ]

                    let span = expr.span().to(&close_token.span);
                    expr = Expr::Index(IndexExpr::new(Box::new(expr), Box::new(index), span));
                }
                TokenKind::Dot => {
                    self.advance(); // consume .

                    let field_token = self.peek().clone();
                    let field_name = if let TokenKind::Ident(ref name) = field_token.kind {
                        name.clone()
                    } else {
                        return Err(Diagnostic::error("expected field name after '.'")
                            .with_span(field_token.span));
                    };
                    self.advance(); // consume identifier

                    let span = expr.span().to(&field_token.span);
                    expr = Expr::MemberAccess(MemberAccessExpr::new(
                        Box::new(expr),
                        field_name,
                        span,
                    ));
                }
                TokenKind::LParen => {
                    self.advance(); // consume (

                    let mut arguments = Vec::new();
                    if !matches!(self.peek().kind, TokenKind::RParen) {
                        loop {
                            arguments.push(self.parse_expr()?);
                            if matches!(self.peek().kind, TokenKind::Comma) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }

                    let rparen_token = self.peek().clone();
                    if !matches!(rparen_token.kind, TokenKind::RParen) {
                        return Err(Diagnostic::error("expected ')' after arguments")
                            .with_span(rparen_token.span));
                    }
                    self.advance(); // consume )

                    let span = expr.span().to(&rparen_token.span);
                    expr = Expr::FunctionCall(FunctionCallExpr::new(
                        Box::new(expr),
                        arguments,
                        span,
                    ));
                }
                // ── Collection operators ──────────────────────────────────────
                TokenKind::DollarHash => {
                    expr = self.parse_collection_length(expr)?;
                }
                TokenKind::DollarPlus => {
                    expr = self.parse_collection_append(expr)?;
                }
                TokenKind::DollarPlusLBracket => {
                    expr = self.parse_collection_insert(expr)?;
                }
                TokenKind::DollarMinus => {
                    expr = self.parse_collection_remove(expr)?;
                }
                TokenKind::DollarMinusLBracket => {
                    expr = self.parse_collection_remove_positional(expr)?;
                }
                TokenKind::DollarQuestion => {
                    expr = self.parse_collection_contains(expr)?;
                }
                TokenKind::DollarQuestionQuestion => {
                    expr = self.parse_string_find_positions(expr)?;
                }
                TokenKind::DollarPlusPlus => {
                    expr = self.parse_string_insert(expr)?;
                }
                TokenKind::DollarMinusMinus => {
                    expr = self.parse_collection_remove_all(expr)?;
                }
                TokenKind::DollarTildeTilde => {
                    expr = self.parse_string_replace(expr)?;
                }
                TokenKind::DollarTilde => {
                    expr = self.parse_collection_update(expr)?;
                }
                TokenKind::DollarLBracket => {
                    expr = self.parse_collection_slice(expr)?;
                }
                TokenKind::DollarGt => {
                    expr = self.parse_collection_map(expr)?;
                }
                TokenKind::DollarPipe => {
                    expr = self.parse_collection_filter(expr)?;
                }
                TokenKind::DollarLt => {
                    expr = self.parse_collection_reduce(expr)?;
                }
                TokenKind::DollarCaretPlus => {
                    expr = self.parse_collection_sort(expr, true)?;
                }
                TokenKind::DollarCaretMinus => {
                    expr = self.parse_collection_sort(expr, false)?;
                }
                TokenKind::DollarCaret => {
                    expr = self.parse_collection_sort_custom(expr)?;
                }
                TokenKind::HashQuestion => {
                    let start_span = expr.span();
                    self.advance(); // consume #?
                    let span = start_span.to(&token.span);
                    expr = Expr::TypeMetadata(TypeMetadataExpr::new(Box::new(expr), span));
                }
                TokenKind::DollarExclaim => {
                    let start_span = expr.span();
                    self.advance(); // consume $!
                    let span = start_span.to(&token.span);
                    expr = Expr::ErrorCheck(zymbol_ast::ErrorCheckExpr::new(Box::new(expr), span));
                }
                TokenKind::DollarExclaimExclaim => {
                    let start_span = expr.span();
                    self.advance(); // consume $!!
                    let span = start_span.to(&token.span);
                    expr = Expr::ErrorPropagate(zymbol_ast::ErrorPropagateExpr::new(Box::new(expr), span));
                }
                _ => break,
            }
        }

        Ok(expr)
    }
}
