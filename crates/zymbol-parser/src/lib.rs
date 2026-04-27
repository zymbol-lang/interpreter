//! Parser for Zymbol-Lang
//!
//! Phase 0: Only parses >> "string" statements
//! Phase 1: Parses assignments and identifiers

use zymbol_ast::{
    BasePrefix, Block, CastKind, CollectionLengthExpr, Expr, ExprStatement, FormatKind,
    FunctionCallExpr, IdentifierExpr, IndexExpr, LiteralExpr,
    NumericCastExpr, Program, RangeExpr, Statement, TypeMetadataExpr,
};
use zymbol_common::Literal;
use zymbol_error::Diagnostic;
use zymbol_lexer::{StringPart, Token, TokenKind};

mod literals;
mod io;
mod if_stmt;
mod loops;
mod match_stmt;
mod variables;
mod functions;
mod collections;
mod collection_ops;
mod string_ops;
mod expressions;
mod data_ops;
mod script_exec;
mod modules;
mod error_handling;
mod index_nav;

/// Parser for Zymbol source code
pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            current: 0,
            diagnostics: Vec::new(),
        }
    }

    /// Parse the token stream into an AST
    pub fn parse(mut self) -> Result<Program, Vec<Diagnostic>> {
        let mut module_decl = None;
        let mut imports = Vec::new();
        let mut statements = Vec::new();

        if matches!(self.peek().kind, TokenKind::Hash) {
            // Module file: parse the closed block # name { ... }
            match self.parse_module_block() {
                Ok((decl, mod_imports, mod_stmts)) => {
                    module_decl = Some(decl);
                    imports = mod_imports;
                    statements = mod_stmts;
                }
                Err(diag) => {
                    self.diagnostics.push(diag);
                }
            }
            // Nothing is allowed after the closing }
            if !self.is_at_end() && self.diagnostics.is_empty() {
                let span = self.peek().span;
                self.diagnostics.push(
                    Diagnostic::error("unexpected token after module block")
                        .with_span(span)
                        .with_help("a module file must contain only: # name { ... }"),
                );
            }
        } else {
            // Executable program: imports first, then statements
            while matches!(self.peek().kind, TokenKind::ModuleImport) {
                match self.parse_import_statement() {
                    Ok(import) => imports.push(import),
                    Err(diag) => {
                        self.diagnostics.push(diag);
                        self.advance();
                    }
                }
            }

            while !self.is_at_end() {
                match self.parse_statement() {
                    Ok(stmt) => {
                        statements.push(stmt);
                        if matches!(self.peek().kind, TokenKind::Semicolon) {
                            self.advance();
                        }
                    }
                    Err(diag) => {
                        self.diagnostics.push(diag);
                        self.advance();
                    }
                }
            }
        }

        if self.diagnostics.is_empty() {
            Ok(Program::new_with_module(module_decl, imports, statements))
        } else {
            Err(self.diagnostics)
        }
    }

    /// Parse a single statement
    fn parse_statement(&mut self) -> Result<Statement, Diagnostic> {
        let token = self.peek();

        match &token.kind {
            TokenKind::SetNumeralMode(base) => {
                let base = *base;
                let span = token.span;
                self.advance(); // consume the token
                Ok(Statement::SetNumeralMode { base, span })
            }
            TokenKind::Output => self.parse_output(),
            TokenKind::Input => self.parse_input(),
            TokenKind::CliArgsCapture => self.parse_cli_args_capture(),
            TokenKind::Question => self.parse_if(),
            TokenKind::DoubleQuestion => self.parse_match_statement(),
            TokenKind::At | TokenKind::AtLabel(_) | TokenKind::AtColonLabel(_) => self.parse_loop(),
            TokenKind::AtBreak | TokenKind::AtColonLabelBreak(_) => self.parse_break(),
            TokenKind::AtContinue | TokenKind::AtColonLabelContinue(_) => self.parse_continue(),
            TokenKind::TryBlock => self.parse_try_statement(),
            TokenKind::Newline | TokenKind::Backslash2 => self.parse_newline(),
            TokenKind::Backslash => self.parse_lifetime_end(),
            TokenKind::Return => self.parse_return(),
            TokenKind::Ident(_) => {
                // Look ahead to distinguish between function declaration, function call, assignment, and const decl
                // Function declaration: name(...) { }
                // Function call (as statement): name(...) (no block after)
                // Constant declaration: name := ...
                // Assignment: name = ...

                // Check for const declaration first (:=)
                if self.peek_ahead(1).map(|t| matches!(t.kind, TokenKind::ConstAssign)).unwrap_or(false) {
                    return self.parse_const_decl();
                }

                // Check for module void call: alias::fn(...) — GAP G11
                if self.peek_ahead(1).map(|t| matches!(t.kind, TokenKind::ScopeResolution)).unwrap_or(false) {
                    return self.parse_function_call_statement();
                }

                if self.peek_ahead(1).map(|t| matches!(t.kind, TokenKind::LParen)).unwrap_or(false) {
                    // It's either a function declaration or a function call statement
                    // We need to scan for the closing ) and check if there's a { after it

                    // Save current position
                    let saved_pos = self.current;

                    // Skip identifier
                    self.advance();
                    // Skip (
                    self.advance();

                    // Scan for matching )
                    let mut depth = 1;
                    while depth > 0 && !self.is_at_end() {
                        match self.peek().kind {
                            TokenKind::LParen => depth += 1,
                            TokenKind::RParen => depth -= 1,
                            _ => {}
                        }
                        self.advance();
                    }

                    // Check what's after the )
                    let has_block = matches!(self.peek().kind, TokenKind::LBrace);

                    // Restore position
                    self.current = saved_pos;

                    if has_block {
                        self.parse_function_decl()
                    } else {
                        // Function call as statement (discard return value)
                        self.parse_function_call_statement()
                    }
                } else {
                    // If next token is an assignment operator, route to parse_assignment.
                    // A bare identifier (or expression starting with one) gets parsed as
                    // an expression statement — this makes REPL inspection work naturally.
                    let is_assignment_op = self.peek_ahead(1)
                        .map(|t| matches!(t.kind,
                            TokenKind::Assign
                            | TokenKind::PlusAssign
                            | TokenKind::MinusAssign
                            | TokenKind::StarAssign
                            | TokenKind::SlashAssign
                            | TokenKind::PercentAssign
                            | TokenKind::CaretAssign
                            | TokenKind::PlusPlus
                            | TokenKind::MinusMinus
                            | TokenKind::LBracket
                            | TokenKind::DollarExclaimExclaim
                        ))
                        .unwrap_or(false);
                    if is_assignment_op {
                        self.parse_assignment()
                    } else {
                        let expr = self.parse_expr()?;
                        let span = expr.span();
                        Ok(Statement::Expr(ExprStatement::new(expr, span)))
                    }
                }
            }
            TokenKind::LBracket => {
                // Could be array destructure: [a, b] = expr
                // is_array_destructure() saves/restores state, so peek() still returns '[' after
                if self.is_array_destructure() {
                    self.parse_destructure_assign()
                } else {
                    let span = self.peek().span;
                    Err(Diagnostic::error("unexpected '[' at statement level")
                        .with_span(span)
                        .with_help("use '[a, b] = expr' for array destructuring"))
                }
            }
            TokenKind::LParen => {
                // Could be tuple destructure: (a, b) = expr or (name: n) = expr
                if self.is_tuple_destructure() {
                    self.parse_destructure_assign()
                } else {
                    let span = self.peek().span;
                    Err(Diagnostic::error("unexpected '(' at statement level")
                        .with_span(span)
                        .with_help("use '(a, b) = expr' for tuple destructuring"))
                }
            }
            TokenKind::BashOpen => {
                // BashExec as void statement: <\ expr... \> (side-effect only, result discarded)
                let expr = self.parse_expr()?;
                let span = expr.span();
                Ok(Statement::Expr(ExprStatement::new(expr, span)))
            }
            TokenKind::Eof => Err(Diagnostic::error("unexpected end of file")
                .with_span(token.span)),
            TokenKind::Error(msg) => Err(Diagnostic::error(msg.clone())
                .with_span(token.span)),
            _ => Err(Diagnostic::error(format!("unexpected token: {:?}", token.kind))
                .with_span(token.span)
                .with_help("expected statement (>>, <<, ?, ??, @, @!, @>, !?, <~, ¶, \\\\, or identifier)")),
        }
    }

    /// Parse a block: { statements }
    fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let start_token = self.peek().clone();
        if !matches!(start_token.kind, TokenKind::LBrace) {
            return Err(Diagnostic::error("expected '{' to start block")
                .with_span(start_token.span)
                .with_help("blocks must be enclosed in braces"));
        }
        self.advance(); // consume {

        let mut statements = Vec::new();

        while !matches!(self.peek().kind, TokenKind::RBrace) && !self.is_at_end() {
            match self.parse_statement() {
                Ok(stmt) => {
                    statements.push(stmt);
                    // Consume optional semicolon after statement
                    if matches!(self.peek().kind, TokenKind::Semicolon) {
                        self.advance();
                    }
                }
                Err(diag) => {
                    self.diagnostics.push(diag);
                    // Try to recover
                    self.advance();
                }
            }
        }

        let end_token = self.peek().clone();
        if !matches!(end_token.kind, TokenKind::RBrace) {
            return Err(Diagnostic::error("expected '}' to close block")
                .with_span(end_token.span)
                .with_help("blocks must be enclosed in braces"));
        }
        self.advance(); // consume }

        let span = start_token.span.to(&end_token.span);

        Ok(Block::new(statements, span))
    }

    // Parse an expression (supports comma for concatenation - lowest precedence)
    // Parse pipe expression: expr |> func(_)
    // Parse logical OR expression: ||
    // Parse logical AND expression: &&
    // Parse comparison expression: ==, <>, <, >, <=, >=
    // Parse addition/subtraction: +, -
    // Parse multiplication/division/modulo: *, /, %
    // Parse power/exponentiation: ^ (right-associative)

    /// Parse range expression: start..end (only literals and identifiers allowed)
    fn parse_range(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.parse_postfix()?;

        // Check for .. operator
        if matches!(self.peek().kind, TokenKind::DotDot) {
            self.advance(); // consume ..

            let end = self.parse_postfix()?;

            // Check for optional step: :step
            if matches!(self.peek().kind, TokenKind::Colon) {
                self.advance(); // consume :
                let step = self.parse_postfix()?;
                let span = start.span().to(&step.span());

                Ok(Expr::Range(RangeExpr::with_step(
                    Box::new(start),
                    Box::new(end),
                    Box::new(step),
                    span,
                )))
            } else {
                let span = start.span().to(&end.span());

                Ok(Expr::Range(RangeExpr::new(
                    Box::new(start),
                    Box::new(end),
                    span,
                )))
            }
        } else {
            Ok(start)
        }
    }

    /// Parse unary + structural postfix only: `[]`, `.`, `()`.
    /// Stops before any `$X` collection operator.
    /// Used as the argument parser for binary collection ops so that
    /// `arr$+ 4$+ 5` chains as `(arr$+ 4)$+ 5` instead of `arr$+(4$+5)`.
    pub(crate) fn parse_postfix_structural(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_unary()?;
        loop {
            match self.peek().kind {
                TokenKind::LBracket => {
                    if self.peek().span.start.line != expr.span().end.line { break; }
                    if self.is_nav_index() {
                        expr = self.parse_nav_index(expr)?;
                    } else {
                        self.advance();
                        let index = self.parse_expr()?;
                        let close = self.peek().clone();
                        if !matches!(close.kind, TokenKind::RBracket) {
                            return Err(Diagnostic::error("expected ']' after index")
                                .with_span(close.span));
                        }
                        self.advance();
                        let span = expr.span().to(&close.span);
                        expr = Expr::Index(IndexExpr::new(Box::new(expr), Box::new(index), span));
                    }
                }
                TokenKind::Dot => {
                    self.advance();
                    let field_token = self.peek().clone();
                    let field_name = if let TokenKind::Ident(ref name) = field_token.kind {
                        name.clone()
                    } else {
                        return Err(Diagnostic::error("expected field name after '.'")
                            .with_span(field_token.span));
                    };
                    self.advance();
                    let span = expr.span().to(&field_token.span);
                    expr = Expr::MemberAccess(zymbol_ast::MemberAccessExpr::new(
                        Box::new(expr), field_name, span,
                    ));
                }
                TokenKind::LParen => {
                    if matches!(expr, Expr::Literal(_)) { break; }
                    if self.peek().span.start.line != expr.span().end.line { break; }
                    self.advance();
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
                    let rparen = self.peek().clone();
                    if !matches!(rparen.kind, TokenKind::RParen) {
                        return Err(Diagnostic::error("expected ')' after arguments")
                            .with_span(rparen.span));
                    }
                    self.advance();
                    let span = expr.span().to(&rparen.span);
                    expr = Expr::FunctionCall(FunctionCallExpr::new(
                        Box::new(expr), arguments, span,
                    ));
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    /// Parse postfix expressions (indexing, member access, function calls)
    /// Also handles unary operators (-x, !x, +x)
    pub(crate) fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_unary()?;

        // Handle indexing, member access, and chained function calls: arr[index], obj.field, func(args)
        loop {
            match self.peek().kind {
                TokenKind::LBracket => {
                    // Only treat '[' as a postfix index if it's on the same line.
                    // A '[' on a new line is either a new statement or an array destructure.
                    if self.peek().span.start.line != expr.span().end.line {
                        break;
                    }
                    if self.is_nav_index() {
                        expr = self.parse_nav_index(expr)?;
                    } else {
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
                }
                TokenKind::Dot => {
                    self.advance(); // consume .

                    // Expect identifier (field name)
                    let field_token = self.peek().clone();
                    let field_name = if let TokenKind::Ident(ref name) = field_token.kind {
                        name.clone()
                    } else {
                        return Err(Diagnostic::error("expected field name after '.'")
                            .with_span(field_token.span)
                            .with_help("member access requires a field name: object.field"));
                    };
                    self.advance(); // consume identifier

                    let span = expr.span().to(&field_token.span);
                    expr = Expr::MemberAccess(zymbol_ast::MemberAccessExpr::new(
                        Box::new(expr),
                        field_name,
                        span,
                    ));
                }
                TokenKind::LParen => {
                    // Chained function call: expr(args) — only if on the same line.
                    // A '(' on a new line starts a new statement (e.g. destructure pattern),
                    // not a chained call.
                    // Literals (strings, numbers, bools, chars) are never callable.
                    if matches!(expr, Expr::Literal(_)) {
                        break;
                    }
                    if self.peek().span.start.line != expr.span().end.line {
                        break;
                    }
                    self.advance(); // consume (

                    // Parse arguments
                    let mut arguments = Vec::new();

                    if !matches!(self.peek().kind, TokenKind::RParen) {
                        loop {
                            arguments.push(self.parse_expr()?);

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
                        return Err(Diagnostic::error("expected ')' after function arguments")
                            .with_span(rparen_token.span)
                            .with_help("function call syntax: expr(arg1, arg2, ...)"));
                    }
                    self.advance(); // consume )

                    let span = expr.span().to(&rparen_token.span);
                    expr = Expr::FunctionCall(FunctionCallExpr::new(
                        Box::new(expr),
                        arguments,
                        span,
                    ));
                }
                _ => break,
            }
        }

        // Handle collection operators (postfix): $#, $+, $-, $?, $~, $[..], $>, $|, $<
        loop {
            let token = self.peek().clone();
            match token.kind {
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
                TokenKind::DollarSlash => {
                    expr = self.parse_string_split(expr)?;
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
                    // Type metadata operator: expr#?
                    let start_span = expr.span();
                    self.advance(); // consume #?
                    let span = start_span.to(&token.span);
                    expr = Expr::TypeMetadata(TypeMetadataExpr::new(Box::new(expr), span));
                }
                TokenKind::DollarExclaim => {
                    // Error check operator: expr$!
                    let start_span = expr.span();
                    self.advance(); // consume $!
                    let span = start_span.to(&token.span);
                    expr = Expr::ErrorCheck(zymbol_ast::ErrorCheckExpr::new(Box::new(expr), span));
                }
                TokenKind::DollarExclaimExclaim => {
                    // Error propagate operator: expr$!!
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

    /// Parse postfix expressions WITHOUT function calls (for pipe operator callable)
    /// Handles: indexing, member access, collection operators, but NOT function calls
    fn parse_postfix_without_calls(&mut self) -> Result<Expr, Diagnostic> {
        // Parse primary expression but skip function call syntax
        // We need to handle: identifiers, parenthesized expressions, lambdas, literals
        // But NOT: identifier(...) or expr(...) function calls
        let mut expr = match self.peek().kind {
            TokenKind::LParen => {
                // Could be lambda or grouped expression
                if self.is_lambda_start() {
                    self.parse_lambda()?
                } else {
                    // Grouped expression
                    self.advance(); // consume (
                    let inner = self.parse_expr()?;
                    if !matches!(self.peek().kind, TokenKind::RParen) {
                        return Err(Diagnostic::error("expected ')' after expression")
                            .with_span(self.peek().span));
                    }
                    self.advance(); // consume )
                    inner
                }
            }
            TokenKind::Ident(_) => {
                // Check for lambda: x -> expr
                if self.peek_ahead(1).is_some_and(|t| matches!(t.kind, TokenKind::Arrow)) {
                    self.parse_lambda()?
                } else {
                    // Simple identifier - DON'T parse function call
                    let token = self.peek().clone();
                    let name = if let TokenKind::Ident(name) = &token.kind {
                        name.clone()
                    } else {
                        unreachable!()
                    };
                    self.advance();
                    Expr::Identifier(IdentifierExpr::new(name, token.span))
                }
            }
            _ => {
                // For other cases, use normal unary parsing
                self.parse_unary()?
            }
        };

        // Handle indexing and member access (same line only), but NOT function calls
        loop {
            let token = self.peek().clone();
            match token.kind {
                TokenKind::LBracket => {
                    // Only treat '[' as a postfix index if it's on the same line.
                    if self.peek().span.start.line != expr.span().end.line {
                        break;
                    }
                    if self.is_nav_index() {
                        expr = self.parse_nav_index(expr)?;
                    } else {
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
                }
                TokenKind::Dot => {
                    self.advance(); // consume .

                    // Expect identifier (field name)
                    let field_token = self.peek().clone();
                    let field_name = if let TokenKind::Ident(ref name) = field_token.kind {
                        name.clone()
                    } else {
                        return Err(Diagnostic::error("expected field name after '.'")
                            .with_span(field_token.span)
                            .with_help("member access requires a field name: object.field"));
                    };
                    self.advance(); // consume identifier

                    let span = expr.span().to(&field_token.span);
                    expr = Expr::MemberAccess(zymbol_ast::MemberAccessExpr::new(
                        Box::new(expr),
                        field_name,
                        span,
                    ));
                }
                // Note: We intentionally skip TokenKind::LParen (function calls) here
                // Collection operators
                TokenKind::DollarHash => {
                    let start_span = expr.span();
                    self.advance(); // consume $#
                    let span = start_span.to(&token.span);
                    expr = Expr::CollectionLength(CollectionLengthExpr::new(Box::new(expr), span));
                }
                TokenKind::DollarPlus => {
                    expr = self.parse_collection_append(expr)?;
                }
                TokenKind::DollarMinus => {
                    expr = self.parse_collection_remove(expr)?;
                }
                TokenKind::DollarTilde => {
                    expr = self.parse_collection_update(expr)?;
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
                TokenKind::DollarSlash => {
                    expr = self.parse_string_split(expr)?;
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

    // Parse unary expression: !, -, +

    /// Parse a primary expression (literals, identifiers)
    fn parse_primary_expr(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();

        match &token.kind {
            TokenKind::String(s) => {
                let s = s.clone();
                self.advance(); // consume string
                Ok(Expr::Literal(LiteralExpr::new(
                    Literal::String(s),
                    token.span,
                )))
            }
            TokenKind::StringInterpolated(parts) => {
                // Reconstruct "{var}" form — stored as InterpolatedString so the
                // interpreter resolves variables at runtime without touching plain strings
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
                self.advance(); // consume interpolated string token
                Ok(Expr::Literal(LiteralExpr::new(
                    Literal::InterpolatedString(reconstructed),
                    token.span,
                )))
            }
            TokenKind::Integer(n) => {
                let n = *n;
                self.advance(); // consume integer
                Ok(Expr::Literal(LiteralExpr::new(Literal::Int(n), token.span)))
            }
            TokenKind::Float(f) => {
                let f = *f;
                self.advance(); // consume float
                Ok(Expr::Literal(LiteralExpr::new(Literal::Float(f), token.span)))
            }
            TokenKind::Char(c) => {
                let c = *c;
                self.advance(); // consume char
                Ok(Expr::Literal(LiteralExpr::new(Literal::Char(c), token.span)))
            }
            TokenKind::Boolean(b) => {
                let b = *b;
                self.advance(); // consume boolean
                Ok(Expr::Literal(LiteralExpr::new(
                    Literal::Bool(b),
                    token.span,
                )))
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                let span_start = token.span;

                // Check for single-param lambda BEFORE consuming: x -> expr
                if let Some(next_token) = self.peek_ahead(1) {
                    if matches!(next_token.kind, TokenKind::Arrow) {
                        // This is a lambda with single parameter
                        return self.parse_lambda();
                    }
                }

                self.advance(); // consume identifier

                // Check for module function call: module::function(...)
                if matches!(self.peek().kind, TokenKind::ScopeResolution) {
                    self.advance(); // consume ::

                    // Parse function name
                    let func_name = match &self.peek().kind {
                        TokenKind::Ident(func_name) => {
                            let func_name = func_name.clone();
                            self.advance();
                            func_name
                        }
                        _ => {
                            return Err(Diagnostic::error("expected function name after '::'")
                                .with_span(self.peek().span))
                        }
                    };

                    // Expect (
                    if !matches!(self.peek().kind, TokenKind::LParen) {
                        return Err(Diagnostic::error("expected '(' for module function call")
                            .with_span(self.peek().span)
                            .with_help("module function call syntax: module::function(args)"));
                    }
                    self.advance(); // consume (

                    // Parse arguments
                    let mut arguments = Vec::new();
                    if !matches!(self.peek().kind, TokenKind::RParen) {
                        loop {
                            arguments.push(self.parse_expr()?);

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
                        return Err(Diagnostic::error("expected ')' after function arguments")
                            .with_span(rparen_token.span)
                            .with_help("module function call syntax: module::function(arg1, arg2, ...)"));
                    }
                    self.advance(); // consume )

                    let span = span_start.to(&rparen_token.span);

                    // Create a module member access as the callable: module::func
                    let module_ident = Expr::Identifier(IdentifierExpr::new(name, span_start));
                    let member_access = Expr::MemberAccess(zymbol_ast::MemberAccessExpr::new_module(
                        Box::new(module_ident),
                        func_name,
                        span_start.to(&self.tokens[self.current - 2].span), // Up to func_name
                    ));

                    Ok(Expr::FunctionCall(FunctionCallExpr::new(
                        Box::new(member_access),
                        arguments,
                        span,
                    )))
                }
                // Check for regular function call: name(...) — only if ( is on the same line.
                // A '(' on a new line starts a new statement (e.g. tuple destructure),
                // not a function call. Same guard as in parse_postfix.
                else if matches!(self.peek().kind, TokenKind::LParen)
                    && self.peek().span.start.line == span_start.start.line
                {
                    self.advance(); // consume (

                    // Parse arguments
                    let mut arguments = Vec::new();

                    if !matches!(self.peek().kind, TokenKind::RParen) {
                        loop {
                            arguments.push(self.parse_expr()?);

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
                        return Err(Diagnostic::error("expected ')' after function arguments")
                            .with_span(rparen_token.span)
                            .with_help("function call syntax: name(arg1, arg2, ...)"));
                    }
                    self.advance(); // consume )

                    let span = span_start.to(&rparen_token.span);

                    // Create identifier as callable
                    let callable = Expr::Identifier(IdentifierExpr::new(name, span_start));

                    Ok(Expr::FunctionCall(FunctionCallExpr::new(
                        Box::new(callable),
                        arguments,
                        span,
                    )))
                } else {
                    // Just an identifier
                    Ok(Expr::Identifier(IdentifierExpr::new(name, span_start)))
                }
            }
            TokenKind::LParen => {
                // Parse grouped expression (expr), tuple (expr, expr, ...), named tuple (name: expr, ...), or lambda (a, b) -> expr

                // Check if it's a lambda with multiple params: (a, b, c) -> expr
                // We need to lookahead to detect the pattern
                if self.is_lambda_start() {
                    return self.parse_lambda();
                }

                // Delegate to collections module for tuple/grouped parsing
                self.parse_tuple_or_grouped()
            }
            TokenKind::LBracket => {
                // Parse array literal: [expr1, expr2, ...]
                self.parse_array_literal()
            }
            TokenKind::DoubleQuestion => {
                // Parse match expression: ?? expr { cases }
                self.parse_match_expr()
            }
            TokenKind::HashPipe => {
                // Parse numeric evaluation: #|expr|
                self.parse_numeric_eval()
            }
            TokenKind::HashHashDot => {
                // Cast to Float: ##.expr
                let start = self.peek().span;
                self.advance(); // consume ##.
                let expr = self.parse_postfix()?;
                let span = start.to(&expr.span());
                Ok(Expr::NumericCast(NumericCastExpr::new(CastKind::ToFloat, Box::new(expr), span)))
            }
            TokenKind::HashHashHash => {
                // Cast to Int rounding: ###expr
                let start = self.peek().span;
                self.advance(); // consume ###
                let expr = self.parse_postfix()?;
                let span = start.to(&expr.span());
                Ok(Expr::NumericCast(NumericCastExpr::new(CastKind::ToIntRound, Box::new(expr), span)))
            }
            TokenKind::HashHashBang => {
                // Cast to Int truncating: ##!expr
                let start = self.peek().span;
                self.advance(); // consume ##!
                let expr = self.parse_postfix()?;
                let span = start.to(&expr.span());
                Ok(Expr::NumericCast(NumericCastExpr::new(CastKind::ToIntTrunc, Box::new(expr), span)))
            }
            TokenKind::HashDot => {
                // Parse round expression: #.N|expr|
                self.parse_round_expr()
            }
            TokenKind::HashExclaim => {
                // Parse truncate expression: #!N|expr|
                self.parse_trunc_expr()
            }
            TokenKind::HashComma => {
                // Parse thousands format: #,|expr| or #,.N|expr| or #,!N|expr|
                self.parse_format_expr(FormatKind::Thousands)
            }
            TokenKind::HashCaret => {
                // Parse scientific notation format: #^|expr| or #^.N|expr| or #^!N|expr|
                self.parse_format_expr(FormatKind::Scientific)
            }
            TokenKind::BaseBinary => {
                // Parse binary base conversion: 0b|expr|
                self.parse_base_conversion(BasePrefix::Binary)
            }
            TokenKind::BaseOctal => {
                // Parse octal base conversion: 0o|expr|
                self.parse_base_conversion(BasePrefix::Octal)
            }
            TokenKind::BaseDecimal => {
                // Parse decimal base conversion: 0d|expr|
                self.parse_base_conversion(BasePrefix::Decimal)
            }
            TokenKind::BaseHex => {
                // Parse hexadecimal base conversion: 0x|expr|
                self.parse_base_conversion(BasePrefix::Hex)
            }
            TokenKind::ExecuteCommand(_) => {
                // Parse execute expression: </ file.zy />
                self.parse_execute_expr()
            }
            TokenKind::BashOpen => {
                // Parse bash execute expression: <\ expr... \>
                self.parse_bash_exec_expr()
            }
            TokenKind::Eof => Err(Diagnostic::error("expected expression, found end of file")
                .with_span(token.span)),
            _ => Err(Diagnostic::error(format!(
                "expected expression, found {:?}",
                token.kind
            ))
            .with_span(token.span)),
        }
    }

    // Parse array literal: [expr1, expr2, ...]


    /// Check if a token is a comment
    fn is_comment(token: &Token) -> bool {
        matches!(token.kind, TokenKind::LineComment(_) | TokenKind::BlockComment(_))
    }

    /// Skip over comment tokens from current position
    fn skip_comments(&mut self) {
        while self.current < self.tokens.len() && Self::is_comment(&self.tokens[self.current]) {
            self.current += 1;
        }
    }

    /// Peek at current token (skipping comments)
    fn peek(&self) -> &Token {
        let mut idx = self.current;
        while idx < self.tokens.len() && Self::is_comment(&self.tokens[idx]) {
            idx += 1;
        }
        if idx < self.tokens.len() {
            &self.tokens[idx]
        } else {
            &self.tokens[self.tokens.len() - 1] // EOF
        }
    }

    /// Peek ahead at token at offset from current position (skipping comments)
    fn peek_ahead(&self, offset: usize) -> Option<&Token> {
        let mut idx = self.current;
        let mut non_comment_count = 0;

        while idx < self.tokens.len() {
            if !Self::is_comment(&self.tokens[idx]) {
                if non_comment_count == offset {
                    return Some(&self.tokens[idx]);
                }
                non_comment_count += 1;
            }
            idx += 1;
        }
        None
    }

    /// Advance to next token and return the current one (skipping comments)
    fn advance(&mut self) -> Token {
        self.skip_comments();
        let token = self.tokens[self.current].clone();
        if !self.is_at_end() {
            self.current += 1;
            self.skip_comments();
        }
        token
    }

    /// Check if at end of tokens
    fn is_at_end(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    /// Check if current position is at start of a lambda with multiple params
    /// Pattern: ( identifier [, identifier]* ) ->
    fn is_lambda_start(&mut self) -> bool {
        // Must start with (
        if !matches!(self.peek().kind, TokenKind::LParen) {
            return false;
        }

        // Save current position for backtracking
        let checkpoint = self.current;

        self.advance(); // consume (

        // Check if we have identifier(s) followed by ->
        let mut is_lambda = false;

        // Must have at least one identifier
        if matches!(self.peek().kind, TokenKind::Ident(_)) {
            loop {
                if !matches!(self.peek().kind, TokenKind::Ident(_)) {
                    break;
                }
                self.advance(); // consume identifier

                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.advance(); // consume comma
                    // Continue to next parameter
                } else if matches!(self.peek().kind, TokenKind::RParen) {
                    self.advance(); // consume )
                    // Check for ->
                    if matches!(self.peek().kind, TokenKind::Arrow) {
                        is_lambda = true;
                    }
                    break;
                } else if matches!(self.peek().kind, TokenKind::Arrow) {
                    // Single param lambda in parens: (x -> expr)
                    is_lambda = true;
                    break;
                } else {
                    // Neither comma, ), nor ->, not a lambda
                    break;
                }
            }
        }

        // Restore position
        self.current = checkpoint;

        is_lambda
    }

}

#[cfg(test)]
mod tests {
    use zymbol_lexer::Lexer;
    use zymbol_span::FileId;
    use zymbol_ast::Statement;
    use crate::Parser;

    fn parse(src: &str) -> Vec<Statement> {
        let (tokens, lex_diags) = Lexer::new(src, FileId(0)).tokenize();
        assert!(lex_diags.is_empty(), "lex errors: {:?}", lex_diags);
        let parser = Parser::new(tokens);
        let program = parser.parse().expect("parse error");
        program.statements
    }

    #[test]
    fn parse_set_numeral_mode_ascii() {
        let stmts = parse("#09#");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Statement::SetNumeralMode { base, .. } => assert_eq!(*base, 0x0030),
            other => panic!("expected SetNumeralMode, got {:?}", other),
        }
    }

    #[test]
    fn parse_set_numeral_mode_devanagari() {
        let stmts = parse("#०९#");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Statement::SetNumeralMode { base, .. } => assert_eq!(*base, 0x0966),
            other => panic!("expected SetNumeralMode, got {:?}", other),
        }
    }

    #[test]
    fn parse_set_numeral_mode_thai() {
        let stmts = parse("#๐๙#");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Statement::SetNumeralMode { base, .. } => assert_eq!(*base, 0x0E50),
            other => panic!("expected SetNumeralMode, got {:?}", other),
        }
    }

    #[test]
    fn parse_mode_switch_followed_by_output() {
        // Mode switch + output statement both parse cleanly in sequence
        let stmts = parse("#09#\n>> 42 ¶");
        assert_eq!(stmts.len(), 3); // SetNumeralMode + Output + Newline
        assert!(matches!(stmts[0], Statement::SetNumeralMode { base: 0x0030, .. }));
        assert!(matches!(stmts[1], Statement::Output(_)));
    }
}
