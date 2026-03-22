//! Function-related token recognition for Zymbol-Lang
//!
//! Handles recognition of function-related tokens:
//! - -> (arrow - lambda expression)
//! - <~ (return statement / output parameter)

use zymbol_span::Position;
use crate::{Lexer, Token, TokenKind};

impl Lexer {
    /// Try to parse function-related tokens
    /// Returns Some(token) if a function token is recognized, None otherwise
    pub(crate) fn try_parse_function_token(&mut self, ch: char, start: Position) -> Option<Token> {
        // Check for -> (arrow - lambda)
        if ch == '-' && self.peek() == Some('>') {
            self.advance();
            self.advance();
            return Some(Token::new(TokenKind::Arrow, self.span(start)));
        }

        // Check for <~ (return / output parameter)
        if ch == '<' && self.peek() == Some('~') {
            self.advance();
            self.advance();
            return Some(Token::new(TokenKind::Return, self.span(start)));
        }

        None
    }
}
