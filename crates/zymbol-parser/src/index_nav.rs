//! Parsers for multi-dimensional indexing and restructuring.
//!
//! Handles the new postfix index forms:
//! - Deep scalar access:    arr[i>j>k]
//! - Flat extraction:       arr[p ; q ; r]  or  arr[[i>j]]
//! - Structured extraction: arr[[group] ; [group] ; ...]
//! - Computed indices:      arr[(expr)>(expr)]

use zymbol_ast::{
    DeepIndexExpr, ExtractGroup, Expr, FlatExtractExpr, IdentifierExpr,
    IndexExpr, LiteralExpr, NavPath, NavStep, StructuredExtractExpr,
};
use zymbol_common::Literal;
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;
use crate::Parser;

impl Parser {
    /// Return `true` when the next `[` should be treated as a nav-index expression
    /// rather than a regular expression index.
    ///
    /// Called when `peek()` is `LBracket`. Uses one extra token of lookahead to decide.
    pub(crate) fn is_nav_index(&self) -> bool {
        // peek(0) is `[` — look at what follows
        let p1 = match self.peek_ahead(1) {
            Some(t) => &t.kind,
            None => return false,
        };

        match p1 {
            // arr[[...]] — double bracket → structured or flat-wrapped extract
            TokenKind::LBracket => true,

            // arr[n>...] or arr[n;...] or arr[n..end] — nav path with plain atom
            TokenKind::Integer(_) | TokenKind::Ident(_) => {
                match self.peek_ahead(2) {
                    Some(t) => matches!(
                        t.kind,
                        TokenKind::Gt | TokenKind::Semicolon | TokenKind::DotDot
                    ),
                    None => false,
                }
            }

            // arr[-n>...] — negative integer nav atom (Minus Integer Gt/;/..)
            TokenKind::Minus => {
                let p2 = match self.peek_ahead(2) {
                    Some(t) => &t.kind,
                    None => return false,
                };
                if matches!(p2, TokenKind::Integer(_)) {
                    match self.peek_ahead(3) {
                        Some(t) => matches!(
                            t.kind,
                            TokenKind::Gt | TokenKind::Semicolon | TokenKind::DotDot
                        ),
                        None => false,
                    }
                } else {
                    false
                }
            }

            // arr[(expr)>(expr)] — computed nav: scan for `>` outside parens before `]`
            TokenKind::LParen => self.scan_nav_gt_after_paren(),

            _ => false,
        }
    }

    /// Scan forward (past the opening `[`) to see if a `>` depth separator
    /// appears outside of parentheses before the closing `]`.
    ///
    /// This detects `arr[(a)>(b)]` as nav without false-positives on `arr[(a > b)]`.
    fn scan_nav_gt_after_paren(&self) -> bool {
        let mut offset = 1; // skip the `[` itself
        let mut depth = 0usize; // paren depth
        loop {
            let tok = match self.peek_ahead(offset) {
                Some(t) => t,
                None => return false,
            };
            match &tok.kind {
                TokenKind::LParen => { depth += 1; }
                TokenKind::RParen => {
                    if depth == 0 { return false; }
                    depth -= 1;
                    // After closing paren at depth 0, check for `>`
                    if depth == 0 {
                        let next = self.peek_ahead(offset + 1);
                        if let Some(nt) = next {
                            if matches!(nt.kind, TokenKind::Gt) {
                                return true;
                            }
                        }
                    }
                }
                TokenKind::RBracket => return false, // reached `]` without finding nav `>`
                TokenKind::Eof => return false,
                _ => {}
            }
            offset += 1;
        }
    }

    /// Entry point: the outer `[` has NOT been consumed yet.
    /// Decides which nav form to build and returns the appropriate `Expr`.
    pub(crate) fn parse_nav_index(&mut self, base_expr: Expr) -> Result<Expr, Diagnostic> {
        self.advance(); // consume `[`

        if matches!(self.peek().kind, TokenKind::LBracket) {
            return self.parse_double_bracket_extract(base_expr);
        }

        self.parse_single_bracket_nav(base_expr)
    }

    /// `arr[[...]]` — the outer `[` has been consumed; the inner `[` is next.
    ///
    /// After the first group's `]`:
    /// - `]`  → FlatExtract (single group, returns flat array)
    /// - `;`  → StructuredExtract (multiple groups, returns array-of-arrays)
    fn parse_double_bracket_extract(&mut self, base_expr: Expr) -> Result<Expr, Diagnostic> {
        let start_span = base_expr.span();

        let first_group = self.parse_extract_group()?;

        if matches!(self.peek().kind, TokenKind::RBracket) {
            // arr[[paths]] → FlatExtract
            let close_token = self.peek().clone();
            self.advance(); // consume outer `]`
            let span = start_span.to(&close_token.span);
            return Ok(Expr::FlatExtract(FlatExtractExpr {
                array: Box::new(base_expr),
                paths: first_group.paths,
                span,
            }));
        }

        // `;` → StructuredExtract
        let mut groups = vec![first_group];
        while matches!(self.peek().kind, TokenKind::Semicolon) {
            self.advance(); // consume `;`
            groups.push(self.parse_extract_group()?);
        }

        let close_token = self.peek().clone();
        if !matches!(close_token.kind, TokenKind::RBracket) {
            return Err(Diagnostic::error("expected ']' to close structured extraction")
                .with_span(close_token.span)
                .with_help("structured extraction: arr[[row>col] ; [row>col]]"));
        }
        self.advance(); // consume outer `]`
        let span = start_span.to(&close_token.span);

        Ok(Expr::StructuredExtract(StructuredExtractExpr {
            array: Box::new(base_expr),
            groups,
            span,
        }))
    }

    /// Parse a group: `[nav_path (, nav_path)*]`  (the opening `[` is peeked, not consumed).
    fn parse_extract_group(&mut self) -> Result<ExtractGroup, Diagnostic> {
        // Consume inner `[`
        self.advance();

        let mut paths = Vec::new();
        paths.push(self.parse_nav_path()?);

        while matches!(self.peek().kind, TokenKind::Comma) {
            self.advance(); // consume `,`
            paths.push(self.parse_nav_path()?);
        }

        let close = self.peek().clone();
        if !matches!(close.kind, TokenKind::RBracket) {
            return Err(Diagnostic::error("expected ']' to close extraction group")
                .with_span(close.span)
                .with_help("each group must be wrapped: [path] or [path1, path2]"));
        }
        self.advance(); // consume inner `]`

        Ok(ExtractGroup { paths })
    }

    /// Single-bracket form: outer `[` already consumed.
    ///
    /// Reads the first nav_path, then decides:
    /// - `]`  and path is one plain step → backward-compat `Expr::Index`
    /// - `]`  and path has `>` steps (no ranges) → `DeepIndex`
    /// - `]`  and path has ranges → `FlatExtract`
    /// - `;`  → `FlatExtract` (multiple paths)
    fn parse_single_bracket_nav(&mut self, base_expr: Expr) -> Result<Expr, Diagnostic> {
        let start_span = base_expr.span();

        let first_path = self.parse_nav_path()?;

        if matches!(self.peek().kind, TokenKind::Semicolon) {
            // Flat extraction: arr[p ; q ; ...]
            let mut paths = vec![first_path];
            while matches!(self.peek().kind, TokenKind::Semicolon) {
                self.advance(); // consume `;`
                paths.push(self.parse_nav_path()?);
            }
            let close_token = self.peek().clone();
            if !matches!(close_token.kind, TokenKind::RBracket) {
                return Err(Diagnostic::error("expected ']' after flat extraction")
                    .with_span(close_token.span));
            }
            self.advance(); // consume `]`
            let span = start_span.to(&close_token.span);
            return Ok(Expr::FlatExtract(FlatExtractExpr {
                array: Box::new(base_expr),
                paths,
                span,
            }));
        }

        // Expect `]`
        let close_token = self.peek().clone();
        if !matches!(close_token.kind, TokenKind::RBracket) {
            return Err(Diagnostic::error("expected ']' after index")
                .with_span(close_token.span)
                .with_help("array indexing must use brackets: arr[index] or arr[i>j]"));
        }
        self.advance(); // consume `]`
        let span = start_span.to(&close_token.span);

        let has_range = first_path.steps.iter().any(|s| s.range_end.is_some());
        let step_count = first_path.steps.len();

        if step_count == 1 && !has_range {
            // Backward-compat: single plain step → Expr::Index
            let single_step = first_path.steps.into_iter().next().unwrap();
            return Ok(Expr::Index(IndexExpr::new(
                Box::new(base_expr),
                single_step.index,
                span,
            )));
        }

        if has_range {
            // Any ranged step → FlatExtract (range expands to multiple values)
            return Ok(Expr::FlatExtract(FlatExtractExpr {
                array: Box::new(base_expr),
                paths: vec![first_path],
                span,
            }));
        }

        // Multiple plain steps, no ranges → DeepIndex
        Ok(Expr::DeepIndex(DeepIndexExpr {
            array: Box::new(base_expr),
            path: first_path,
            span,
        }))
    }

    /// Parse a `nav_path`: `nav_step (">" nav_step)*`
    ///
    /// Inside a postfix index context `[...]`, `>` is always a depth separator,
    /// not a comparison operator.
    pub(crate) fn parse_nav_path(&mut self) -> Result<NavPath, Diagnostic> {
        let mut steps = vec![self.parse_nav_step()?];

        while matches!(self.peek().kind, TokenKind::Gt) {
            self.advance(); // consume `>`
            steps.push(self.parse_nav_step()?);
        }

        Ok(NavPath { steps })
    }

    /// Parse a `nav_step`: `nav_atom (".." nav_atom)?`
    fn parse_nav_step(&mut self) -> Result<NavStep, Diagnostic> {
        let index = self.parse_nav_atom()?;

        let range_end = if matches!(self.peek().kind, TokenKind::DotDot) {
            self.advance(); // consume `..`
            Some(self.parse_nav_atom()?)
        } else {
            None
        };

        Ok(NavStep { index, range_end })
    }

    /// Parse a `nav_atom`: integer literal | -integer | identifier | `(` expr `)`
    fn parse_nav_atom(&mut self) -> Result<Box<Expr>, Diagnostic> {
        let token = self.peek().clone();
        match &token.kind {
            // Negative integer literal: -n
            TokenKind::Minus => {
                self.advance(); // consume `-`
                let int_token = self.peek().clone();
                if let TokenKind::Integer(n) = int_token.kind {
                    self.advance();
                    let span = token.span.to(&int_token.span);
                    return Ok(Box::new(Expr::Literal(LiteralExpr::new(
                        Literal::Int(-n),
                        span,
                    ))));
                }
                Err(Diagnostic::error("expected integer after '-' in nav index")
                    .with_span(int_token.span)
                    .with_help("negative indices: arr[-1], arr[-2>-1]"))
            }
            TokenKind::Integer(n) => {
                let n = *n;
                self.advance();
                Ok(Box::new(Expr::Literal(LiteralExpr::new(
                    Literal::Int(n),
                    token.span,
                ))))
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();
                Ok(Box::new(Expr::Identifier(IdentifierExpr::new(name, token.span))))
            }
            TokenKind::LParen => {
                self.advance(); // consume `(`
                let expr = self.parse_expr()?;
                let close = self.peek().clone();
                if !matches!(close.kind, TokenKind::RParen) {
                    return Err(Diagnostic::error("expected ')' after computed index")
                        .with_span(close.span)
                        .with_help("computed indices: arr[(expr)>(expr)]"));
                }
                self.advance(); // consume `)`
                Ok(Box::new(expr))
            }
            _ => Err(Diagnostic::error(
                "expected index: integer, variable, or (expression)",
            )
            .with_span(token.span)
            .with_help(
                "valid nav index: arr[1>2], arr[n>m], arr[(a)>(b)], arr[1>2..4]",
            )),
        }
    }
}
