//! Collection token recognition for Zymbol-Lang
//!
//! Handles recognition of collection-related tokens:
//! - [ (left bracket - array start)
//! - ] (right bracket - array end)
//! - ( (left paren - tuple start)
//! - ) (right paren - tuple end)
//! - , (comma - separator)

use zymbol_span::Position;
use crate::{Lexer, Token, TokenKind};

impl Lexer {
    /// Try to parse collection-related tokens
    /// Returns Some(token) if a collection token is recognized, None otherwise
    pub(crate) fn try_parse_collection_token(&mut self, ch: char, start: Position) -> Option<Token> {
        // Check for [ (left bracket)
        if ch == '[' {
            self.advance();
            return Some(Token::new(TokenKind::LBracket, self.span(start)));
        }

        // Check for ] (right bracket)
        if ch == ']' {
            self.advance();
            return Some(Token::new(TokenKind::RBracket, self.span(start)));
        }

        // Check for ( (left paren)
        if ch == '(' {
            self.advance();
            return Some(Token::new(TokenKind::LParen, self.span(start)));
        }

        // Check for ) (right paren)
        if ch == ')' {
            self.advance();
            return Some(Token::new(TokenKind::RParen, self.span(start)));
        }

        // Check for , (comma)
        if ch == ',' {
            self.advance();
            return Some(Token::new(TokenKind::Comma, self.span(start)));
        }

        None
    }
}
