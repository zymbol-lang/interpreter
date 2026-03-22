//! Collection operator token recognition for Zymbol-Lang
//!
//! Handles tokenization of all collection operators:
//! - $# (length/size)
//! - $+ (append element)
//! - $- (remove by index)
//! - $? (contains/search)
//! - $~ (update element)
//! - $[ (slice start)
//! - $> (map - transform collection)
//! - $| (filter - select elements)
//! - $< (reduce - accumulate)
//!
//! String operators:
//! - $?? (find all positions of pattern in string)
//! - $++ (insert text at position)
//! - $-- (remove text by count)
//! - $~~ (replace pattern with replacement text)
//!
//! Error handling operators:
//! - $! (is_error - check if value is an error)
//! - $!! (error propagate - rethrow error to caller)

use zymbol_span::Position;
use crate::{Lexer, Token, TokenKind};

impl Lexer {
    /// Try to parse a collection operator token starting with '$'
    /// Returns Some(Token) if a collection operator is recognized, None otherwise
    pub(crate) fn try_parse_collection_op(&mut self, start: Position) -> Option<Token> {
        // We know ch == '$' at this point
        if let Some(next) = self.peek() {
            match next {
                '#' => {
                    self.advance(); // consume $
                    self.advance(); // consume #
                    return Some(Token::new(TokenKind::DollarHash, self.span(start)));
                }
                '+' => {
                    // Check for $++ (string insert operator)
                    if self.peek_ahead(2) == Some('+') {
                        self.advance(); // consume $
                        self.advance(); // consume first +
                        self.advance(); // consume second +
                        return Some(Token::new(TokenKind::DollarPlusPlus, self.span(start)));
                    }
                    // Otherwise, it's $+ (array append)
                    self.advance(); // consume $
                    self.advance(); // consume +
                    return Some(Token::new(TokenKind::DollarPlus, self.span(start)));
                }
                '-' => {
                    // Check for $-- (string remove operator)
                    if self.peek_ahead(2) == Some('-') {
                        self.advance(); // consume $
                        self.advance(); // consume first -
                        self.advance(); // consume second -
                        return Some(Token::new(TokenKind::DollarMinusMinus, self.span(start)));
                    }
                    // Otherwise, it's $- (array remove by index)
                    self.advance(); // consume $
                    self.advance(); // consume -
                    return Some(Token::new(TokenKind::DollarMinus, self.span(start)));
                }
                '?' => {
                    // Check for $?? (string find positions operator)
                    if self.peek_ahead(2) == Some('?') {
                        self.advance(); // consume $
                        self.advance(); // consume first ?
                        self.advance(); // consume second ?
                        return Some(Token::new(TokenKind::DollarQuestionQuestion, self.span(start)));
                    }
                    // Otherwise, it's $? (contains/search)
                    self.advance(); // consume $
                    self.advance(); // consume ?
                    return Some(Token::new(TokenKind::DollarQuestion, self.span(start)));
                }
                '~' => {
                    // Check for $~~ (string replace operator)
                    if self.peek_ahead(2) == Some('~') {
                        self.advance(); // consume $
                        self.advance(); // consume first ~
                        self.advance(); // consume second ~
                        return Some(Token::new(TokenKind::DollarTildeTilde, self.span(start)));
                    }
                    // Otherwise, it's $~ (collection update)
                    self.advance(); // consume $
                    self.advance(); // consume ~
                    return Some(Token::new(TokenKind::DollarTilde, self.span(start)));
                }
                '[' => {
                    self.advance(); // consume $
                    self.advance(); // consume [
                    return Some(Token::new(TokenKind::DollarLBracket, self.span(start)));
                }
                '>' => {
                    self.advance(); // consume $
                    self.advance(); // consume >
                    return Some(Token::new(TokenKind::DollarGt, self.span(start)));
                }
                '|' => {
                    self.advance(); // consume $
                    self.advance(); // consume |
                    return Some(Token::new(TokenKind::DollarPipe, self.span(start)));
                }
                '<' => {
                    self.advance(); // consume $
                    self.advance(); // consume <
                    return Some(Token::new(TokenKind::DollarLt, self.span(start)));
                }
                '!' => {
                    // Check for $!! (error propagate operator)
                    if self.peek_ahead(2) == Some('!') {
                        self.advance(); // consume $
                        self.advance(); // consume first !
                        self.advance(); // consume second !
                        return Some(Token::new(TokenKind::DollarExclaimExclaim, self.span(start)));
                    }
                    // Otherwise, it's $! (is_error check)
                    self.advance(); // consume $
                    self.advance(); // consume !
                    return Some(Token::new(TokenKind::DollarExclaim, self.span(start)));
                }
                _ => {}
            }
        }
        None
    }
}
