//! Loop token recognition for Zymbol-Lang
//!
//! Handles recognition of loop-related tokens:
//! - @ (universal loop)
//! - @! (break)
//! - @> (continue)
//! - @label (labeled loop declaration, fused — no space between @ and identifier)

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

        // Check for @label (labeled loop declaration: @outer i:1..5 { })
        // Requires no space between @ and the identifier — distinguishes from
        // `@ bool_var { }` (while loop) vs `@label { }` (labeled infinite loop).
        if ch == '@' && self.peek().map(|c| Self::is_ident_start(c)).unwrap_or(false) {
            self.advance(); // consume @, now current_char is first letter of label
            let mut label = String::new();
            while !self.is_at_end() && Self::is_ident_continue(self.current_char()) {
                label.push(self.current_char());
                self.advance();
            }
            return Some(Token::new(TokenKind::AtLabel(label), self.span(start)));
        }

        // Check for @ (loop)
        if ch == '@' {
            self.advance();
            return Some(Token::new(TokenKind::At, self.span(start)));
        }

        None
    }
}
