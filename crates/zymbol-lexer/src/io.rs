//! IO token recognition for Zymbol-Lang
//!
//! Handles recognition of IO-related tokens:
//! - >> (output)
//! - << (input)
//! - >< (CLI args capture)
//! - ¶ (pilcrow newline)
//! - \\ (double backslash newline)

use zymbol_span::Position;
use crate::{Lexer, Token, TokenKind};

impl Lexer {
    /// Try to parse IO-related tokens
    /// Returns Some(token) if an IO token is recognized, None otherwise
    pub(crate) fn try_parse_io_token(&mut self, ch: char, start: Position) -> Option<Token> {
        // Check for >> (output)
        if ch == '>' && self.peek() == Some('>') {
            self.advance();
            self.advance();
            return Some(Token::new(TokenKind::Output, self.span(start)));
        }

        // Check for >< (CLI args capture)
        if ch == '>' && self.peek() == Some('<') {
            self.advance();
            self.advance();
            return Some(Token::new(TokenKind::CliArgsCapture, self.span(start)));
        }

        // Check for << (input)
        if ch == '<' && self.peek() == Some('<') {
            self.advance();
            self.advance();
            return Some(Token::new(TokenKind::Input, self.span(start)));
        }

        // Check for \\ (double backslash - explicit newline)
        if ch == '\\' && self.peek() == Some('\\') {
            self.advance();
            self.advance();
            return Some(Token::new(TokenKind::Backslash2, self.span(start)));
        }

        // Check for \> (bash execute end) — MUST come before single-backslash check
        // Bug fix: try_parse_io_token runs before the BashEnd check in next_token(),
        // so \> was being consumed as Backslash + Gt instead of BashEnd.
        if ch == '\\' && self.peek() == Some('>') {
            self.advance();
            self.advance();
            return Some(Token::new(TokenKind::BashEnd, self.span(start)));
        }

        // Check for \ (single backslash - lifetime end)
        if ch == '\\' {
            self.advance();
            return Some(Token::new(TokenKind::Backslash, self.span(start)));
        }

        // Check for ¶ (pilcrow - explicit newline)
        if ch == '¶' {
            self.advance();
            return Some(Token::new(TokenKind::Newline, self.span(start)));
        }

        None
    }
}
