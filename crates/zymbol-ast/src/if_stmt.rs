//! IF statement AST nodes for Zymbol-Lang
//!
//! Contains AST structures for conditional statements:
//! - IF: ? condition { }
//! - ELSE-IF: _? condition { }
//! - ELSE: _ { }

use zymbol_span::Span;
use crate::{Block, Expr};

/// If statement: ? condition { } _? { } _ { }
#[derive(Debug, Clone)]
pub struct IfStmt {
    pub condition: Box<Expr>,
    pub then_block: Block,
    pub else_if_branches: Vec<ElseIfBranch>,
    pub else_block: Option<Block>,
    pub span: Span,
}

/// Else-if branch: _? condition { }
#[derive(Debug, Clone)]
pub struct ElseIfBranch {
    pub condition: Box<Expr>,
    pub block: Block,
    pub span: Span,
}

impl IfStmt {
    pub fn new(
        condition: Box<Expr>,
        then_block: Block,
        else_if_branches: Vec<ElseIfBranch>,
        else_block: Option<Block>,
        span: Span,
    ) -> Self {
        Self {
            condition,
            then_block,
            else_if_branches,
            else_block,
            span,
        }
    }
}

impl ElseIfBranch {
    pub fn new(condition: Box<Expr>, block: Block, span: Span) -> Self {
        Self {
            condition,
            block,
            span,
        }
    }
}
