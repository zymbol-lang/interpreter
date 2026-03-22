//! IF statement execution for Zymbol-Lang
//!
//! Handles runtime execution of conditional statements:
//! - IF: ? condition { }
//! - ELSE-IF: _? condition { }
//! - ELSE: _ { }

use zymbol_ast::{Block, IfStmt, Statement};
use crate::{Interpreter, Result, Value};
use std::io::Write;

/// QW7: returns true if the block declares any variable (Assignment or ConstDecl).
/// Blocks that only contain control flow, expressions, and returns do NOT need
/// an isolated scope — saving push_scope + pop_scope (~130ns) per execution.
#[inline(always)]
fn needs_own_scope(block: &Block) -> bool {
    block.statements.iter().any(|s| {
        matches!(s, Statement::Assignment(_) | Statement::ConstDecl(_))
    })
}

impl<W: Write> Interpreter<W> {
    /// Execute IF statement: ? condition { } _? condition { } _ { }
    pub(crate) fn execute_if(&mut self, if_stmt: &IfStmt) -> Result<()> {
        let condition = self.eval_expr(&if_stmt.condition)?;

        if self.is_truthy(&condition) {
            // QW7: skip scope creation when block has no variable declarations
            if needs_own_scope(&if_stmt.then_block) {
                self.execute_block(&if_stmt.then_block)?;
            } else {
                self.execute_block_no_scope(&if_stmt.then_block)?;
            }
        } else {
            let mut executed = false;
            for else_if_branch in &if_stmt.else_if_branches {
                let else_if_condition = self.eval_expr(&else_if_branch.condition)?;
                if self.is_truthy(&else_if_condition) {
                    if needs_own_scope(&else_if_branch.block) {
                        self.execute_block(&else_if_branch.block)?;
                    } else {
                        self.execute_block_no_scope(&else_if_branch.block)?;
                    }
                    executed = true;
                    break;
                }
            }

            if !executed {
                if let Some(else_block) = &if_stmt.else_block {
                    if needs_own_scope(else_block) {
                        self.execute_block(else_block)?;
                    } else {
                        self.execute_block_no_scope(else_block)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Helper: Check if a value is truthy
    ///
    /// NOTE: This is shared between IF and WHILE loops.
    /// Temporarily pub(crate) until final module structure is decided.
    pub(crate) fn is_truthy(&self, value: &Value) -> bool {
        match value {
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            Value::String(s) => !s.is_empty(),
            _ => false,
        }
    }
}
