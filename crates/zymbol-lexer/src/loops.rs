//! Loop token recognition for Zymbol-Lang
//!
//! Handles recognition of loop-related tokens:
//! - @ (universal loop)
//! - @! (break)
//! - @> (continue)

use zymbol_span::Position;
use crate::{Lexer, Token, TokenKind};

impl Lexer {
    /// Try to parse loop-related tokens
    /// Returns Some(token) if a loop token is recognized, None otherwise
    pub(crate) fn try_parse_loop_token(&mut self, ch: char, start: Position) -> Option<Token> {
        // Check for @! (break)
        if ch == '@' && self.peek() == Some('!') {
            self.advance();
            self.advance();
            return Some(Token::new(TokenKind::AtBreak, self.span(start)));
        }

        // Check for @> (continue)
        if ch == '@' && self.peek() == Some('>') {
            self.advance();
            self.advance();
            return Some(Token::new(TokenKind::AtContinue, self.span(start)));
        }

        // Check for @ (loop)
        if ch == '@' {
            self.advance();
            return Some(Token::new(TokenKind::At, self.span(start)));
        }

        None
    }
}
