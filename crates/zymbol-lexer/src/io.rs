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
        // Check for >> and >>! >>? >>| >>~ (output family).
        // ch = source[current] (first '>'), peek() = source[current+1], peek_ahead(2) = source[current+2].
        // Each advance() moves current forward by 1.
        if ch == '>' && self.peek() == Some('>') {
            match self.peek_ahead(2) {
                Some('!') => {
                    self.advance(); self.advance(); self.advance(); // consume > > !
                    return Some(Token::new(TokenKind::OutputClear, self.span(start)));
                }
                Some('?') => {
                    self.advance(); self.advance(); self.advance(); // consume > > ?
                    return Some(Token::new(TokenKind::OutputQuery, self.span(start)));
                }
                Some('|') => {
                    self.advance(); self.advance(); self.advance(); // consume > > |
                    return Some(Token::new(TokenKind::OutputGate,  self.span(start)));
                }
                Some('~') => {
                    self.advance(); self.advance(); self.advance(); // consume > > ~
                    return Some(Token::new(TokenKind::OutputPos,   self.span(start)));
                }
                _ => {}
            }
            self.advance(); self.advance(); // consume > >
            return Some(Token::new(TokenKind::Output, self.span(start)));
        }

        // Check for >< (CLI args capture)
        if ch == '>' && self.peek() == Some('<') {
            self.advance();
            self.advance();
            return Some(Token::new(TokenKind::CliArgsCapture, self.span(start)));
        }

        // Check for << and <<| <<|? (input family).
        // ch = source[current] (first '<'), peek_ahead(2) = third char, peek_ahead(3) = fourth char.
        if ch == '<' && self.peek() == Some('<') {
            if self.peek_ahead(2) == Some('|') {
                if self.peek_ahead(3) == Some('?') {
                    self.advance(); self.advance(); self.advance(); self.advance(); // < < | ?
                    return Some(Token::new(TokenKind::KeyNonBlock, self.span(start)));
                }
                self.advance(); self.advance(); self.advance(); // < < |
                return Some(Token::new(TokenKind::KeyBlock, self.span(start)));
            }
            self.advance(); self.advance(); // < <
            return Some(Token::new(TokenKind::Input, self.span(start)));
        }

        // Check for \> (bash execute close) — only when inside a bash exec block
        if ch == '\\' && self.peek() == Some('>') && self.bash_depth > 0 {
            self.advance(); // consume \
            self.advance(); // consume >
            self.bash_depth -= 1;
            return Some(Token::new(TokenKind::BashClose, self.span(start)));
        }

        // Check for \\ (double backslash - explicit newline)
        if ch == '\\' && self.peek() == Some('\\') {
            self.advance();
            self.advance();
            return Some(Token::new(TokenKind::Backslash2, self.span(start)));
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
