//! Span-ordered comment stream for faithful comment re-emission.
//!
//! Replaces the old `merge_comments` line-matching pass: instead of trying to
//! match formatted lines back to original lines (which corrupted output when
//! the matching failed), the visitor walks the AST in source order and asks
//! this stream which comments precede / trail each statement, using nothing
//! but spans. Indentation comes for free from the `OutputBuilder`.

use zymbol_lexer::{Token, TokenKind};

/// One comment extracted from the token stream, markers excluded.
#[derive(Debug, Clone)]
pub struct Comment {
    /// Raw content: text after `//`, or everything between `/*` and `*/`
    /// (may contain newlines for block comments).
    pub text: String,
    pub is_block: bool,
    pub start_line: u32,
    pub end_line: u32,
    /// 1-indexed column of the opening `/` — used to re-indent block-comment
    /// continuation lines relative to their opening line (spec §9.3).
    pub start_col: u32,
}

/// Comments in source order with a consuming cursor.
pub struct CommentStream {
    items: Vec<Comment>,
    idx: usize,
}

impl CommentStream {
    pub fn from_tokens(tokens: &[Token]) -> Self {
        let mut items: Vec<Comment> = tokens
            .iter()
            .filter_map(|t| match &t.kind {
                TokenKind::LineComment(text) => Some(Comment {
                    text: text.clone(),
                    is_block: false,
                    start_line: t.span.start.line,
                    end_line: t.span.end.line,
                    start_col: t.span.start.column,
                }),
                TokenKind::BlockComment(text) => Some(Comment {
                    text: text.clone(),
                    is_block: true,
                    start_line: t.span.start.line,
                    end_line: t.span.end.line,
                    start_col: t.span.start.column,
                }),
                _ => None,
            })
            .collect();
        items.sort_by_key(|c| (c.start_line, c.start_col));
        Self { items, idx: 0 }
    }

    pub fn peek(&self) -> Option<&Comment> {
        self.items.get(self.idx)
    }

    /// Take the next comment that starts strictly before `line`.
    pub fn next_before_line(&mut self, line: u32) -> Option<Comment> {
        match self.peek() {
            Some(c) if c.start_line < line => {
                let c = self.items[self.idx].clone();
                self.idx += 1;
                Some(c)
            }
            _ => None,
        }
    }

    /// Take the next comment that starts exactly on `line` (a trailing comment).
    pub fn next_on_line(&mut self, line: u32) -> Option<Comment> {
        match self.peek() {
            Some(c) if c.start_line == line => {
                let c = self.items[self.idx].clone();
                self.idx += 1;
                Some(c)
            }
            _ => None,
        }
    }

    /// True when an unconsumed comment starts within `[start_line, end_line]`
    /// (used to refuse inlining a block that contains a comment).
    pub fn has_within(&self, start_line: u32, end_line: u32) -> bool {
        self.items[self.idx..]
            .iter()
            .take_while(|c| c.start_line <= end_line)
            .any(|c| c.start_line >= start_line)
    }

    /// Take everything that is left (end-of-scope / end-of-file flush).
    pub fn drain_rest(&mut self) -> Vec<Comment> {
        let rest = self.items[self.idx..].to_vec();
        self.idx = self.items.len();
        rest
    }
}
