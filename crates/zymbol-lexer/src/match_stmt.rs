//! MATCH statement token recognition for Zymbol-Lang
//!
//! Handles recognition of MATCH/pattern matching related tokens:
//! - ?? (match expression)

use zymbol_span::Position;
use crate::{Lexer, Token, TokenKind};

impl Lexer {
    /// Try to parse MATCH-related tokens
    /// Returns Some(token) if a MATCH token is recognized, None otherwise
    pub(crate) fn try_parse_match_token(&mut self, ch: char, start: Position) -> Option<Token> {
        // Check for ?? (match)
        if ch == '?' && self.peek() == Some('?') {
            self.advance();
            self.advance();
            return Some(Token::new(TokenKind::DoubleQuestion, self.span(start)));
        }

        None
    }
}
