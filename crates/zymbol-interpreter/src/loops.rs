//! Loop execution for Zymbol-Lang (GRUPO 6: LOOPS)
//!
//! Handles runtime execution of loops:
//! - Universal loop: @ [condition] { }
//! - For-each loop: @ var:iterable { }
//! - Loop control: BREAK (@!), CONTINUE (@>)
//! - Labeled loops: @ @label { }

use std::io::Write;
use zymbol_ast::{Break, Continue, Expr, Loop};
use crate::{ControlFlow, Interpreter, Result, RuntimeError, Value};

/// QW16: Returns true if the block introduces any variable NOT already in scope.
/// If false, execute_block_no_scope is safe — no new scope is needed.
/// Checked ONCE before the loop starts (not per iteration).
fn body_needs_own_scope<W: std::io::Write>(block: &zymbol_ast::Block, interp: &Interpreter<W>) -> bool {
    use zymbol_ast::Statement;
    block.statements.iter().any(|s| match s {
        Statement::Assignment(a) => interp.get_variable(&a.name).is_none(),
        Statement::ConstDecl(_) => true,
        Statement::DestructureAssign(_) => true,
        _ => false,
    })
}

impl<W: Write> Interpreter<W> {
    /// Handle loop control flow after executing a loop body.
    /// Returns `true` if the loop should `break` (Break, Return, or labeled Continue for outer loop).
    /// Resets Break/Continue control flow when the label matches this loop.
    #[inline(always)]
    fn handle_loop_control(&mut self, loop_label: &Option<String>) -> bool {
        if !self.is_control_flow_pending() { return false; }
        match &self.control_flow {
            ControlFlow::Break(label) => {
                if label.is_none() || label == loop_label {
                    self.clear_control_flow();
                }
                true
            }
            ControlFlow::Continue(label) => {
                let ours = label.is_none() || label == loop_label;
                if ours { self.clear_control_flow(); }
                !ours
            }
            ControlFlow::Return(_) => true,
            ControlFlow::None => false,
        }
    }

    /// Execute break statement: @! [label]
    pub(crate) fn execute_break(&mut self, break_stmt: &Break) -> Result<()> {
        self.set_control_flow(ControlFlow::Break(break_stmt.label.clone()));
        Ok(())
    }

    /// Execute continue statement: @> [label]
    pub(crate) fn execute_continue(&mut self, continue_stmt: &Continue) -> Result<()> {
        self.set_control_flow(ControlFlow::Continue(continue_stmt.label.clone()));
        Ok(())
    }

    /// Execute loop statement: @ condition { } or @ var:iterable { }
    pub(crate) fn execute_loop(&mut self, loop_stmt: &Loop) -> Result<()> {
        // Check if this is a for-each loop
        if let (Some(iterator_var), Some(iterable_expr)) = (&loop_stmt.iterator_var, &loop_stmt.iterable) {
            // B5: Fast path for integer ranges — avoid Vec allocation
            if let Expr::Range(range_expr) = &**iterable_expr {
                let start_val = self.eval_expr(&range_expr.start)?;
                let end_val = self.eval_expr(&range_expr.end)?;
                let step = if let Some(step_expr) = &range_expr.step {
                    match self.eval_expr(step_expr)? {
                        Value::Int(n) if n > 0 => n,
                        Value::Int(n) => return Err(RuntimeError::Generic {
                            message: format!("step must be positive, got {}", n),
                            span: step_expr.span(),
                        }),
                        other => return Err(RuntimeError::Generic {
                            message: format!("step must be an integer, got {:?}", other),
                            span: step_expr.span(),
                        }),
                    }
                } else { 1i64 };

                let (start, end) = match (start_val, end_val) {
                    (Value::Int(s), Value::Int(e)) => (s, e),
                    (sv, ev) => return Err(RuntimeError::Generic {
                        message: format!("range bounds must be integers, got {:?} and {:?}", sv, ev),
                        span: range_expr.start.span(),
                    }),
                };

                let forward = start <= end;
                let mut current = start;
                // QW16: check once whether loop body needs a fresh scope
                let needs_scope = body_needs_own_scope(&loop_stmt.body, self);
                loop {
                    if (forward && current > end) || (!forward && current < end) { break; }

                    self.set_variable(iterator_var, Value::Int(current));
                    if needs_scope {
                        self.execute_block(&loop_stmt.body)?;
                    } else {
                        self.execute_block_no_scope(&loop_stmt.body)?;
                    }

                    if self.handle_loop_control(&loop_stmt.label) { break; }

                    if forward { current += step; } else { current -= step; }
                }
                return Ok(());
            }

            // Slow path: non-range iterables (arrays, strings)
            let values = self.eval_iterable(iterable_expr)?;

            // QW16: check once whether loop body needs a fresh scope
            let needs_scope = body_needs_own_scope(&loop_stmt.body, self);
            for value in values {
                // Set iterator variable
                self.set_variable(iterator_var, value);

                // Execute loop body
                if needs_scope {
                    self.execute_block(&loop_stmt.body)?;
                } else {
                    self.execute_block_no_scope(&loop_stmt.body)?;
                }

                if self.handle_loop_control(&loop_stmt.label) { break; }
            }

            Ok(())
        } else {
            // While loop, TIMES loop, or infinite loop

            // Check if we have a condition
            if let Some(condition_expr) = &loop_stmt.condition {
                // Evaluate condition ONCE to determine loop type
                let initial_value = self.eval_expr(condition_expr)?;

                // INFERENCIA: Detect TIMES vs WHILE based on initial value type
                // QW16: check once whether loop body needs a fresh scope
                let needs_scope = body_needs_own_scope(&loop_stmt.body, self);
                match initial_value {
                    Value::Int(n) if n > 0 => {
                        // TIMES loop: repeat N times (evaluated once)
                        for _ in 0..n {
                            if needs_scope {
                                self.execute_block(&loop_stmt.body)?;
                            } else {
                                self.execute_block_no_scope(&loop_stmt.body)?;
                            }
                            if self.handle_loop_control(&loop_stmt.label) { break; }
                        }
                    }
                    _ => {
                        // WHILE loop: re-evaluate condition each iteration
                        loop {
                            let condition = self.eval_expr(condition_expr)?;

                            if !self.is_truthy(&condition) {
                                break; // Exit loop if condition is false
                            }

                            // Execute loop body
                            if needs_scope {
                                self.execute_block(&loop_stmt.body)?;
                            } else {
                                self.execute_block_no_scope(&loop_stmt.body)?;
                            }
                            if self.handle_loop_control(&loop_stmt.label) { break; }
                        }
                    }
                }
            } else {
                // Infinite loop: no condition
                // QW16: check once whether loop body needs a fresh scope
                let needs_scope = body_needs_own_scope(&loop_stmt.body, self);
                loop {
                    if needs_scope {
                        self.execute_block(&loop_stmt.body)?;
                    } else {
                        self.execute_block_no_scope(&loop_stmt.body)?;
                    }
                    if self.handle_loop_control(&loop_stmt.label) { break; }
                }
            }

            Ok(())
        }
    }
}
