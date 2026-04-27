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
                    // Check for $++ (retired string insert — kept for migration error in parser)
                    if self.peek_ahead(2) == Some('+') {
                        self.advance(); // consume $
                        self.advance(); // consume first +
                        self.advance(); // consume second +
                        return Some(Token::new(TokenKind::DollarPlusPlus, self.span(start)));
                    }
                    // Check for $+[ (insert at position — arrays, tuples, strings)
                    // Note: space between $+ and [ produces DollarPlus + LBracket (append array literal)
                    if self.peek_ahead(2) == Some('[') {
                        self.advance(); // consume $
                        self.advance(); // consume +
                        self.advance(); // consume [
                        return Some(Token::new(TokenKind::DollarPlusLBracket, self.span(start)));
                    }
                    // Otherwise, it's $+ (append value)
                    self.advance(); // consume $
                    self.advance(); // consume +
                    return Some(Token::new(TokenKind::DollarPlus, self.span(start)));
                }
                '-' => {
                    // Check for $-- (remove all occurrences of value — arrays, tuples, strings)
                    if self.peek_ahead(2) == Some('-') {
                        self.advance(); // consume $
                        self.advance(); // consume first -
                        self.advance(); // consume second -
                        return Some(Token::new(TokenKind::DollarMinusMinus, self.span(start)));
                    }
                    // Check for $-[ (remove at position or range — arrays, tuples, strings)
                    // Note: space between $- and [ produces DollarMinus + LBracket (remove value [..])
                    if self.peek_ahead(2) == Some('[') {
                        self.advance(); // consume $
                        self.advance(); // consume -
                        self.advance(); // consume [
                        return Some(Token::new(TokenKind::DollarMinusLBracket, self.span(start)));
                    }
                    // Otherwise, it's $- (remove first occurrence of value)
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
                '^' => {
                    // $^+ (natural ascending), $^- (natural descending), $^ (custom comparator)
                    if self.peek_ahead(2) == Some('+') {
                        self.advance(); // consume $
                        self.advance(); // consume ^
                        self.advance(); // consume +
                        return Some(Token::new(TokenKind::DollarCaretPlus, self.span(start)));
                    }
                    if self.peek_ahead(2) == Some('-') {
                        self.advance(); // consume $
                        self.advance(); // consume ^
                        self.advance(); // consume -
                        return Some(Token::new(TokenKind::DollarCaretMinus, self.span(start)));
                    }
                    // $^ — custom comparator sort (direction encoded in lambda)
                    self.advance(); // consume $
                    self.advance(); // consume ^
                    return Some(Token::new(TokenKind::DollarCaret, self.span(start)));
                }
                '/' => {
                    self.advance(); // consume $
                    self.advance(); // consume /
                    return Some(Token::new(TokenKind::DollarSlash, self.span(start)));
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
