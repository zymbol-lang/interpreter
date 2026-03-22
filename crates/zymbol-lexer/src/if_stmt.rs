//! IF statement token recognition for Zymbol-Lang
//!
//! Handles recognition of IF/ELSE-IF/ELSE related tokens:
//! - ? (if)
//! - _? (else-if)
//! - _ (else/wildcard)

use zymbol_span::Position;
use crate::{Lexer, Token, TokenKind};

impl Lexer {
    /// Try to parse IF-related tokens
    /// Returns Some(token) if an IF token is recognized, None otherwise
    pub(crate) fn try_parse_if_token(&mut self, ch: char, start: Position) -> Option<Token> {
        // Check for ?? (match) or ? (if)
        if ch == '?' {
            if self.peek() == Some('?') {
                // ?? is not an IF token, it's a MATCH token (GRUPO 5)
                return None;
            } else {
                self.advance();
                return Some(Token::new(TokenKind::Question, self.span(start)));
            }
        }

        // Check for _? (else-if)
        if ch == '_' && self.peek() == Some('?') {
            self.advance();
            self.advance();
            return Some(Token::new(TokenKind::ElseIf, self.span(start)));
        }

        // Check for _ (underscore - might be identifier or else)
        if ch == '_' {
            // Peek ahead to see if it's followed by valid ident char (identifier) or not (else)
            if let Some(next) = self.peek() {
                if Self::is_ident_continue(next) {
                    // This is an identifier like _variable, not an else/wildcard token
                    return None;
                }
            }
            // Standalone underscore (else/wildcard)
            self.advance();
            return Some(Token::new(TokenKind::Underscore, self.span(start)));
        }

        None
    }
}
