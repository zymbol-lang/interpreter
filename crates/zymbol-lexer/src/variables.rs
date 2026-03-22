//! Variable and constant token recognition for Zymbol-Lang
//!
//! Handles recognition of variable and constant related tokens:
//! - = (assignment)
//! - := (constant declaration)
//! - += (add assign)
//! - -= (subtract assign)
//! - *= (multiply assign)
//! - /= (divide assign)
//! - %= (modulo assign)
//! - ++ (increment)
//! - -- (decrement)

use zymbol_span::Position;
use crate::{Lexer, Token, TokenKind};

impl Lexer {
    /// Try to parse variable/constant assignment tokens
    /// Returns Some(token) if a variable-related token is recognized, None otherwise
    pub(crate) fn try_parse_variable_token(&mut self, ch: char, start: Position) -> Option<Token> {
        // Check for == (equal comparison)
        if ch == '=' && self.peek() == Some('=') {
            self.advance();
            self.advance();
            return Some(Token::new(TokenKind::Eq, self.span(start)));
        }

        // Check for = (assign)
        if ch == '=' {
            self.advance();
            return Some(Token::new(TokenKind::Assign, self.span(start)));
        }

        // Check for := (const assignment), :! (catch), :> (finally), :: (scope resolution), or : (colon)
        if ch == ':' {
            if self.peek() == Some('=') {
                self.advance();
                self.advance();
                return Some(Token::new(TokenKind::ConstAssign, self.span(start)));
            }
            if self.peek() == Some('!') {
                self.advance(); // consume :
                self.advance(); // consume !
                return Some(Token::new(TokenKind::CatchBlock, self.span(start)));
            }
            if self.peek() == Some('>') {
                self.advance(); // consume :
                self.advance(); // consume >
                return Some(Token::new(TokenKind::FinallyBlock, self.span(start)));
            }
            // Note: :: and : are not variable tokens, return None to let main lexer handle them
            return None;
        }

        // Check for + operators (++, +=, +)
        if ch == '+' {
            if self.peek() == Some('+') {
                self.advance();
                self.advance();
                return Some(Token::new(TokenKind::PlusPlus, self.span(start)));
            } else if self.peek() == Some('=') {
                self.advance();
                self.advance();
                return Some(Token::new(TokenKind::PlusAssign, self.span(start)));
            }
            // Plain + is not a variable token, return None
            return None;
        }

        // Check for - operators (->, --, -=, -)
        if ch == '-' {
            if self.peek() == Some('>') {
                // -> is not a variable token (it's for lambdas), return None
                return None;
            } else if self.peek() == Some('-') {
                self.advance();
                self.advance();
                return Some(Token::new(TokenKind::MinusMinus, self.span(start)));
            } else if self.peek() == Some('=') {
                self.advance();
                self.advance();
                return Some(Token::new(TokenKind::MinusAssign, self.span(start)));
            }
            // Plain - is not a variable token, return None
            return None;
        }

        // Check for * operators (*=, *)
        if ch == '*' {
            if self.peek() == Some('=') {
                self.advance();
                self.advance();
                return Some(Token::new(TokenKind::StarAssign, self.span(start)));
            }
            // Plain * is not a variable token, return None
            return None;
        }

        // Check for / operators (/=, /)
        if ch == '/' {
            if self.peek() == Some('=') {
                self.advance();
                self.advance();
                return Some(Token::new(TokenKind::SlashAssign, self.span(start)));
            }
            // Other / tokens are not variable tokens, return None
            return None;
        }

        // Check for % operators (%=, %)
        if ch == '%' {
            if self.peek() == Some('=') {
                self.advance();
                self.advance();
                return Some(Token::new(TokenKind::PercentAssign, self.span(start)));
            }
            // Plain % is not a variable token, return None
            return None;
        }

        None
    }
}
