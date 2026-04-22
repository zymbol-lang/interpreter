//! Loop token recognition for Zymbol-Lang
//!
//! Handles recognition of loop-related tokens:
//! - @ (universal loop)
//! - @! (break)
//! - @> (continue)
//! - @:label  (labeled loop declaration)
//! - @:label! (labeled break)
//! - @:label> (labeled continue)

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

        // Check for @:label, @:label!, @:label>
        if ch == '@' && self.peek() == Some(':') {
            self.advance(); // consume @
            self.advance(); // consume :
            // Must have an identifier immediately after @:
            if self.is_at_end() || !Self::is_ident_start(self.current_char()) {
                // Malformed — emit As plain @: which will cause a parse error
                return Some(Token::new(TokenKind::At, self.span(start)));
            }
            let mut label = String::new();
            while !self.is_at_end() && Self::is_ident_continue(self.current_char()) {
                label.push(self.current_char());
                self.advance();
            }
            // Check for trailing ! or > to distinguish break/continue from declaration
            if !self.is_at_end() && self.current_char() == '!' {
                self.advance(); // consume !
                return Some(Token::new(TokenKind::AtColonLabelBreak(label), self.span(start)));
            }
            if !self.is_at_end() && self.current_char() == '>' {
                self.advance(); // consume >
                return Some(Token::new(TokenKind::AtColonLabelContinue(label), self.span(start)));
            }
            return Some(Token::new(TokenKind::AtColonLabel(label), self.span(start)));
        }

        // Check for @ (loop)
        if ch == '@' {
            self.advance();
            return Some(Token::new(TokenKind::At, self.span(start)));
        }

        None
    }
}
