//! Loop execution for Zymbol-Lang (GRUPO 6: LOOPS)
//!
//! Handles runtime execution of loops:
//! - Universal loop: @ [condition] { }
//! - For-each loop: @ var:iterable { }
//! - Loop control: BREAK (@!), CONTINUE (@>)
//! - Labeled loops: @ @label { }

use std::io::Write;
use zymbol_ast::{Break, Continue, Expr, Loop, Sleep};
use crate::{ControlFlow, Interpreter, Result, RuntimeError, Value};

/// The `@ <expr>` specifier is either a count (`Int`) or a condition (`Bool`).
/// Anything else is refused rather than coerced — the same message and the same
/// type names all four engines use, so the form fails identically everywhere.
fn loop_specifier_error(value: &Value, span: zymbol_span::Span) -> RuntimeError {
    RuntimeError::Generic {
        message: format!("loop expects a count or a condition, got {}", value.type_word()),
        span,
    }
}

/// Returns true if an assignment's RHS contains a hot self-reference to the same variable.
/// Covers `arr = arr°$+ i` (CollectionAppend) and `s = s° + ch` (Binary Add).
fn rhs_has_hot_self_ref(expr: &zymbol_ast::Expr, name: &str) -> bool {
    use zymbol_ast::Expr;
    match expr {
        Expr::CollectionAppend(op) => {
            if let Expr::Identifier(id) = op.collection.unwrap_group() {
                (id.hot || id.pre_hot) && id.name == name
            } else { false }
        }
        Expr::Binary(bin) => {
            if let Expr::Identifier(id) = bin.left.unwrap_group() {
                (id.hot || id.pre_hot) && id.name == name
            } else { false }
        }
        _ => false,
    }
}

/// QW16: Returns true if the block introduces any variable NOT already in scope.
/// Hot (`x°`) and pre-hot (`°x`) assignments always anchor to a loop scope, so they
/// never need a fresh body scope. If false, execute_block_no_scope is safe.
/// Checked ONCE before the loop starts (not per iteration).
fn body_needs_own_scope<W: std::io::Write>(block: &zymbol_ast::Block, interp: &Interpreter<W>) -> bool {
    use zymbol_ast::Statement;
    block.statements.iter().any(|s| match s {
        Statement::Assignment(a) => {
            let is_hot = a.hot || a.pre_hot || rhs_has_hot_self_ref(&a.value, &a.name);
            !is_hot && interp.get_variable(&a.name).is_none()
        }
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

    /// Execute sleep statement: @~ N (milliseconds)
    pub(crate) fn execute_sleep(&mut self, sleep: &Sleep) -> Result<()> {
        let ms = match self.eval_expr(&sleep.duration)? {
            Value::Int(n) if n >= 0 => n as u64,
            Value::Int(n) => return Err(RuntimeError::Generic {
                message: format!("@~ requires non-negative duration, got {}", n),
                span: sleep.span,
            }),
            other => return Err(RuntimeError::Generic {
                message: format!("@~ requires integer milliseconds, got {}", self.value_type_name(&other)),
                span: sleep.span,
            }),
        };
        std::thread::sleep(std::time::Duration::from_millis(ms));
        Ok(())
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
    /// Always creates a persistent loop-anchor scope so that x° and °x anchoring works.
    pub(crate) fn execute_loop(&mut self, loop_stmt: &Loop) -> Result<()> {
        self.push_loop_scope();
        let result = self.run_loop(loop_stmt);
        self.pop_loop_scope();
        result
    }

    fn run_loop(&mut self, loop_stmt: &Loop) -> Result<()> {
        // Check if this is a for-each loop
        if let (Some(iterator_var), Some(iterable_expr)) = (&loop_stmt.iterator_var, &loop_stmt.iterable) {
            // B5: Fast path for integer ranges — avoid Vec allocation
            if let Expr::Range(range_expr) = iterable_expr.unwrap_group() {
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
                // QW16: check once whether loop body needs a fresh scope per iteration
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

            // QW16: check once whether loop body needs a fresh scope per iteration
            let needs_scope = body_needs_own_scope(&loop_stmt.body, self);
            for value in values {
                self.set_variable(iterator_var, value);

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

            if let Some(condition_expr) = &loop_stmt.condition {
                let initial_value = self.eval_expr(condition_expr)?;

                // QW16: check once whether loop body needs a fresh scope per iteration
                let needs_scope = body_needs_own_scope(&loop_stmt.body, self);
                match initial_value {
                    Value::Int(n) => {
                        // TIMES loop: repeat N times (evaluated once). A count of
                        // zero or less runs the body zero times — it is a count,
                        // not a condition, so it never falls through to WHILE.
                        for _ in 0..n.max(0) {
                            if needs_scope {
                                self.execute_block(&loop_stmt.body)?;
                            } else {
                                self.execute_block_no_scope(&loop_stmt.body)?;
                            }
                            if self.handle_loop_control(&loop_stmt.label) { break; }
                        }
                    }
                    Value::Bool(_) => {
                        // WHILE loop: re-evaluate condition each iteration
                        loop {
                            let condition = self.eval_expr(condition_expr)?;
                            let Value::Bool(keep_going) = condition else {
                                return Err(loop_specifier_error(&condition, condition_expr.span()));
                            };

                            if !keep_going { break; }

                            if needs_scope {
                                self.execute_block(&loop_stmt.body)?;
                            } else {
                                self.execute_block_no_scope(&loop_stmt.body)?;
                            }
                            if self.handle_loop_control(&loop_stmt.label) { break; }
                        }
                    }
                    other => {
                        // Neither a count nor a condition. Truthiness would have
                        // to invent an answer here, and each engine invented a
                        // different one — an array was false in the tree-walker,
                        // true in the VM and a hard error in zyml.
                        return Err(loop_specifier_error(&other, condition_expr.span()));
                    }
                }
            } else {
                // Infinite loop: no condition
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
