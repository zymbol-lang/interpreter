//! IF statement execution for Zymbol-Lang
//!
//! Handles runtime execution of conditional statements:
//! - IF: ? condition { }
//! - ELSE-IF: _? condition { }
//! - ELSE: _ { }

use zymbol_ast::{Block, IfStmt, Statement};
use crate::{Interpreter, Result, RuntimeError, Value};
use std::io::Write;

/// QW7: returns true if the block declares any variable (Assignment or ConstDecl).
/// Blocks that only contain control flow, expressions, and returns do NOT need
/// an isolated scope — saving push_scope + pop_scope (~130ns) per execution.
#[inline(always)]
fn needs_own_scope(block: &Block) -> bool {
    block.statements.iter().any(|s| {
        matches!(s, Statement::Assignment(_) | Statement::ConstDecl(_) | Statement::DestructureAssign(_))
    })
}

impl<W: Write> Interpreter<W> {
    /// Execute IF statement: ? condition { } _? condition { } _ { }
    pub(crate) fn execute_if(&mut self, if_stmt: &IfStmt) -> Result<()> {
        let condition = self.eval_expr(&if_stmt.condition)?;

        if Self::condition(&condition, &if_stmt.condition)? {
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
                if Self::condition(&else_if_condition, &else_if_branch.condition)? {
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

    /// A `?` / `_?` condition is a Bool and nothing else (GLB-121).
    ///
    /// This read an Int as `!= 0`, a String as "not empty" and everything else
    /// as false, while the VM read everything but 0 and Unit as true and zyjs
    /// used its host's rules: nine of thirteen types took a different branch
    /// depending on the engine, under one identical warning. There is no
    /// truthiness, and a wrong TYPE is a `##Type` (D1).
    ///
    /// The expression is taken, not its span: asking an `Expr` where it is costs
    /// a match on every `?` that runs, and it is only needed on the way out
    /// (measured 2026-10-11: +2.2 % of the instructions of a recursive program).
    #[inline(always)]
    fn condition(value: &Value, written: &zymbol_ast::Expr) -> Result<bool> {
        match value {
            Value::Bool(b) => Ok(*b),
            other => Err(Self::not_a_condition(other, written)),
        }
    }

    #[cold]
    #[inline(never)]
    fn not_a_condition(value: &Value, written: &zymbol_ast::Expr) -> RuntimeError {
        RuntimeError::kinded(
            "Type",
            format!("a condition must be a Bool, got {}", value.type_ident()),
            written.span(),
        )
    }
}
