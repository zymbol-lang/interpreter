//! Expression evaluation for Zymbol-Lang
//!
//! Handles runtime evaluation of all expression types:
//! - Binary expressions (arithmetic, comparison, logical)
//! - Unary expressions (negation, logical NOT, positive)
//! - Pipe expressions (function composition with placeholder syntax)

use zymbol_ast::{BinaryExpr, Expr, PipeExpr, UnaryExpr};
use zymbol_common::num;
use zymbol_common::BinaryOp;
use crate::arithmetic_ops::int_result;
use crate::{Interpreter, Result, RuntimeError, Value};
use std::io::Write;

impl<W: Write> Interpreter<W> {
    /// Evaluate pipe expression: value |> func(_) or value |> (x -> x * 2)(_)
    pub(crate) fn eval_pipe(&mut self, pipe: &PipeExpr) -> Result<Value> {
        // Evaluate the left side (value being piped)
        let piped_value = self.eval_expr(&pipe.left)?;

        // Evaluate the callable
        let callable_value = self.eval_expr(&pipe.callable)?;

        // Build arguments, replacing _ with piped_value
        let mut arg_values = Vec::new();
        for arg in &pipe.arguments {
            match arg {
                zymbol_ast::PipeArg::Placeholder => {
                    // Replace _ with the piped value
                    arg_values.push(piped_value.clone());
                }
                zymbol_ast::PipeArg::Expr(expr) => {
                    // Evaluate the expression normally
                    arg_values.push(self.eval_expr(expr)?);
                }
            }
        }

        // Call the function/lambda with the arguments
        match callable_value {
            Value::Function(func) => {
                // Lambda call
                self.eval_lambda_call(func, arg_values, &pipe.span)
            }
            _ => {
                Err(RuntimeError::Generic {
                    message: "pipe operator requires a callable function or lambda".to_string(),
                    span: pipe.span,
                })
            }
        }
    }

    /// Evaluate a binary expression (arithmetic and comparison operators)
    pub(crate) fn eval_binary(&mut self, binary: &BinaryExpr) -> Result<Value> {
        use zymbol_common::Literal;

        // Short-circuit && and ||, before anything evaluates both sides.
        //
        // This has to come first. Every path below — the fast paths and the
        // slow one — evaluates `binary.right` before dispatching on the
        // operator, which made `#0 && f()` call `f()` and `arr$# > 0 &&
        // arr[1] > 5` index an empty array. The answer was still right, so the
        // corpus could not see it: only a right-hand side with an observable
        // effect tells the two apart. The VM and the browser engine have always
        // short-circuited; this is the tree-walker catching up (DM-19).
        //
        // Guarding the *left* operand's type before deciding is deliberate:
        // `1 && x` must stay the same error it has always been, not become an
        // error about `x`.
        if matches!(binary.op, BinaryOp::And | BinaryOp::Or) {
            let is_and = binary.op == BinaryOp::And;
            let name = if is_and { "AND" } else { "OR" };
            let left = self.eval_expr(&binary.left)?;
            let left_bool = match &left {
                Value::Bool(b) => *b,
                _ => return Err(RuntimeError::Generic {
                    message: format!("logical {name} requires boolean operands, got {}", left.type_ident()),
                    span: binary.span,
                }),
            };
            // `#0 && _` is #0 and `#1 || _` is #1 whatever the right side says,
            // so the right side is not evaluated at all — not even to type-check
            // it. That is the whole point: the left operand guards the right.
            if left_bool != is_and {
                return Ok(Value::Bool(left_bool));
            }
            let right = self.eval_expr(&binary.right)?;
            let right_bool = match &right {
                Value::Bool(b) => *b,
                _ => return Err(RuntimeError::Generic {
                    message: format!("logical {name} requires boolean operands, got {}", right.type_ident()),
                    span: binary.span,
                }),
            };
            return Ok(Value::Bool(right_bool));
        }
        // QW15a: Identifier OP IntLiteral — most common in loops/conditions
        // Saves 2× eval_expr dispatch (~80ns) per binary expression
        if let Expr::Identifier(lhs) = binary.left.unwrap_group() {
            if let Expr::Literal(rlit) = binary.right.unwrap_group() {
                if let Literal::Int(rval) = &rlit.value {
                    if let Some(Value::Int(lval)) = self.get_variable(&lhs.name) {
                        let (l, r) = (*lval, *rval);
                        match binary.op {
                            BinaryOp::Lt  => return Ok(Value::Bool(l < r)),
                            BinaryOp::Le  => return Ok(Value::Bool(l <= r)),
                            BinaryOp::Gt  => return Ok(Value::Bool(l > r)),
                            BinaryOp::Ge  => return Ok(Value::Bool(l >= r)),
                            BinaryOp::Eq  => return Ok(Value::Bool(l == r)),
                            BinaryOp::Neq => return Ok(Value::Bool(l != r)),
                            // The i53 range is checked here, not only on the
                            // slow path. These arms used to wrap, so
                            // `>>(9007199254740991 + 1)` raised ##Range on a
                            // literal — constant-folded — and answered
                            // 9007199254740992 the moment the same value came
                            // from a variable, which is the shape every real
                            // program has (DM-01).
                            BinaryOp::Add => return int_result(num::add(l, r), l, "+", r, &binary.span),
                            BinaryOp::Sub => return int_result(num::sub(l, r), l, "-", r, &binary.span),
                            BinaryOp::Mul => return int_result(num::mul(l, r), l, "*", r, &binary.span),
                            BinaryOp::Mod if r != 0 => return Ok(Value::Int(l % r)),
                            BinaryOp::Div if r != 0 => return Ok(Value::Int(l / r)),
                            _ => {}
                        }
                    }
                }
            }
            // QW15b: Identifier OP Identifier — both Int
            if let Expr::Identifier(rhs) = binary.right.unwrap_group() {
                let lv = self.get_variable(&lhs.name).and_then(|v| if let Value::Int(n) = v { Some(*n) } else { None });
                let rv = self.get_variable(&rhs.name).and_then(|v| if let Value::Int(n) = v { Some(*n) } else { None });
                if let (Some(l), Some(r)) = (lv, rv) {
                    match binary.op {
                        BinaryOp::Lt  => return Ok(Value::Bool(l < r)),
                        BinaryOp::Le  => return Ok(Value::Bool(l <= r)),
                        BinaryOp::Gt  => return Ok(Value::Bool(l > r)),
                        BinaryOp::Ge  => return Ok(Value::Bool(l >= r)),
                        BinaryOp::Eq  => return Ok(Value::Bool(l == r)),
                        BinaryOp::Neq => return Ok(Value::Bool(l != r)),
                        // Same range check as the arm above: identifier OP
                        // identifier is the other half of DM-01.
                        BinaryOp::Add => return int_result(num::add(l, r), l, "+", r, &binary.span),
                        BinaryOp::Sub => return int_result(num::sub(l, r), l, "-", r, &binary.span),
                        BinaryOp::Mul => return int_result(num::mul(l, r), l, "*", r, &binary.span),
                        BinaryOp::Mod if r != 0 => return Ok(Value::Int(l % r)),
                        BinaryOp::Div if r != 0 => return Ok(Value::Int(l / r)),
                        _ => {}
                    }
                }
            }
        }
        // Hot/pre_hot RHS: c = c°/°c + a — eval right first to infer neutral type, then init left
        //
        // `Concat` is here for the same reason `Add` is: `s = °s "x"` is the
        // string accumulator GUIDE.md documents, and without this branch the
        // `°s` was evaluated as a variable that does not exist yet — so the
        // tree-walker refused the program while the VM answered `0xxx` and the
        // browser engine answered `0`. Three engines, three answers, on a form
        // the guide gives as an example (GLB-002).
        //
        // The neutral follows the OPERATOR, not the operand: juxtaposition
        // joins text, so its neutral is the empty string whatever is on the
        // right. `+` keeps inferring from the right-hand value, because there
        // it is the operand that decides between Int and Float.
        if matches!(binary.op, BinaryOp::Add | BinaryOp::Concat) {
            if let Expr::Identifier(ident) = binary.left.unwrap_group() {
                if (ident.hot || ident.pre_hot) && self.get_variable(&ident.name).is_none() {
                    let right_val = self.eval_expr(&binary.right)?;
                    let neutral = if binary.op == BinaryOp::Concat {
                        Value::String(String::new())
                    } else {
                        match &right_val {
                            Value::String(_) => Value::String(String::new()),
                            Value::Float(_)  => Value::Float(0.0),
                            _                => Value::Int(0),
                        }
                    };
                    if ident.pre_hot {
                        self.set_above_nearest_loop(&ident.name, neutral);
                    } else {
                        self.set_variable(&ident.name, neutral);
                    }
                    let left_val = self.eval_expr(&binary.left)?;
                    return if binary.op == BinaryOp::Concat {
                        self.eval_concat(&left_val, &right_val, &binary.span)
                    } else {
                        self.eval_add(&left_val, &right_val, &binary.span)
                    };
                }
            }
        }

        // Slow path: full eval
        let left = self.eval_expr(&binary.left)?;
        let right = self.eval_expr(&binary.right)?;

        match binary.op {
            // Juxtaposition concatenation (implicit, no explicit operator)
            BinaryOp::Concat => self.eval_concat(&left, &right, &binary.span),

            // Arithmetic operators
            BinaryOp::Add => self.eval_add(&left, &right, &binary.span),
            BinaryOp::Sub => self.eval_arithmetic(&left, &right, num::sub, |a, b| a - b, "-", &binary.span),
            BinaryOp::Mul => self.eval_arithmetic(&left, &right, num::mul, |a, b| a * b, "*", &binary.span),
            BinaryOp::Div => self.eval_div(&left, &right, &binary.span),
            BinaryOp::Mod => self.eval_mod(&left, &right, &binary.span),
            BinaryOp::Pow => self.eval_pow(&left, &right, &binary.span),

            // Comparison operators
            BinaryOp::Eq => Ok(Value::Bool(self.values_equal(&left, &right))),
            BinaryOp::Neq => Ok(Value::Bool(!self.values_equal(&left, &right))),
            BinaryOp::Lt => self.compare_values(&left, &right, |a, b| a < b, |a, b| a < b, &binary.op),
            BinaryOp::Gt => self.compare_values(&left, &right, |a, b| a > b, |a, b| a > b, &binary.op),
            BinaryOp::Le => self.compare_values(&left, &right, |a, b| a <= b, |a, b| a <= b, &binary.op),
            BinaryOp::Ge => self.compare_values(&left, &right, |a, b| a >= b, |a, b| a >= b, &binary.op),

            // Logical operators
            BinaryOp::And => {
                let left_bool = match &left {
                    Value::Bool(b) => *b,
                    _ => return Err(RuntimeError::Generic {
                        message: format!("logical AND requires boolean operands, got {}", left.type_ident()),
                        span: binary.span,
                    }),
                };
                let right_bool = match &right {
                    Value::Bool(b) => *b,
                    _ => return Err(RuntimeError::Generic {
                        message: format!("logical AND requires boolean operands, got {}", right.type_ident()),
                        span: binary.span,
                    }),
                };
                Ok(Value::Bool(left_bool && right_bool))
            }
            BinaryOp::Or => {
                let left_bool = match &left {
                    Value::Bool(b) => *b,
                    _ => return Err(RuntimeError::Generic {
                        message: format!("logical OR requires boolean operands, got {}", left.type_ident()),
                        span: binary.span,
                    }),
                };
                let right_bool = match &right {
                    Value::Bool(b) => *b,
                    _ => return Err(RuntimeError::Generic {
                        message: format!("logical OR requires boolean operands, got {}", right.type_ident()),
                        span: binary.span,
                    }),
                };
                Ok(Value::Bool(left_bool || right_bool))
            }

            _ => Err(RuntimeError::Generic {
                message: format!("unsupported binary operator: {:?}", binary.op),
                span: binary.span,
            }),
        }
    }

    /// Evaluate unary expression (!, -, +)
    pub(crate) fn eval_unary(&mut self, unary: &UnaryExpr) -> Result<Value> {
        let operand = self.eval_expr(&unary.operand)?;

        match unary.op {
            zymbol_common::UnaryOp::Not => {
                match operand {
                    Value::Bool(b) => Ok(Value::Bool(!b)),
                    _ => Err(RuntimeError::Generic {
                        message: format!("logical NOT requires boolean operand, got {}", operand.type_ident()),
                        span: unary.span,
                    }),
                }
            }
            zymbol_common::UnaryOp::Neg => {
                match operand {
                    Value::Int(n) => Ok(Value::Int(-n)),
                    Value::Float(f) => Ok(Value::Float(-f)),
                    _ => Err(RuntimeError::Generic {
                        message: format!("negation requires numeric operand, got {}", operand.type_ident()),
                        span: unary.span,
                    }),
                }
            }
            zymbol_common::UnaryOp::Pos => {
                match operand {
                    Value::Int(n) => Ok(Value::Int(n)),
                    Value::Float(f) => Ok(Value::Float(f)),
                    _ => Err(RuntimeError::Generic {
                        message: format!("unary plus requires numeric operand, got {:?}", operand),
                        span: unary.span,
                    }),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Interpreter;
    use zymbol_lexer::Lexer;
    use zymbol_parser::Parser;
    use zymbol_span::FileId;

    fn run(source: &str) -> String {
        let mut output = Vec::new();
        let lexer = Lexer::new(source, FileId(0));
        let (tokens, lex_diagnostics) = lexer.tokenize();
        assert!(lex_diagnostics.is_empty(), "Lexer errors: {:?}", lex_diagnostics);
        let parser = Parser::new(tokens);
        let program = parser.parse().expect("Parse error");
        let mut interpreter = Interpreter::with_output(&mut output);
        interpreter.execute(&program).expect("Runtime error");
        String::from_utf8(output).expect("Invalid UTF-8")
    }

    #[test]
    fn test_function_call_statement_basic() {
        let code = r#"
greet(name) {
    >> "Hello " name "!" ¶
    <~ #1
}

greet("Alice")
greet("Bob")
"#;
        let output = run(code);
        assert_eq!(output, "Hello Alice!\nHello Bob!\n");
    }

    #[test]
    fn test_function_call_statement_with_return() {
        let code = r#"
factorial(n) {
    ? n <= 1 {
        <~ 1
    }
    <~ n * factorial(n - 1)
}

factorial(5)
factorial(3)
>> "Done" ¶
"#;
        let output = run(code);
        assert_eq!(output, "Done\n");
    }

    #[test]
    fn test_expression_statement_inside_block() {
        let code = r#"
log(msg) {
    >> "[LOG] " msg ¶
}

x = 10
? x > 5 {
    log("x is greater than 5")
    log("Continuing...")
}
"#;
        let output = run(code);
        assert_eq!(output, "[LOG] x is greater than 5\n[LOG] Continuing...\n");
    }

    #[test]
    fn test_multiple_expression_statements() {
        let code = r#"
print_num(n) {
    >> n ¶
}

print_num(1)
print_num(2)
print_num(3)
print_num(4)
print_num(5)
"#;
        let output = run(code);
        assert_eq!(output, "1\n2\n3\n4\n5\n");
    }

    #[test]
    fn test_expression_statement_in_loop() {
        let code = r#"
log(msg) {
    >> msg ¶
}

@ i:1..3 {
    log("Iteration")
}
"#;
        let output = run(code);
        assert_eq!(output, "Iteration\nIteration\nIteration\n");
    }

    #[test]
    fn test_expression_statement_with_output_params() {
        let code = r#"
swap(a, b, x<~, y<~) {
    x = b
    y = a
}

first = 10
second = 20
swap(first, second, first, second)
>> first " " second ¶
"#;
        let output = run(code);
        assert_eq!(output, "20 10\n");
    }
}
