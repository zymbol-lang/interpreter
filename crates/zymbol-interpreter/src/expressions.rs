//! Expression evaluation for Zymbol-Lang
//!
//! Handles runtime evaluation of all expression types:
//! - Binary expressions (arithmetic, comparison, logical)
//! - Unary expressions (negation, logical NOT, positive)
//! - Pipe expressions (function composition with placeholder syntax)

use zymbol_ast::{BinaryExpr, Expr, PipeExpr, UnaryExpr};
use zymbol_common::BinaryOp;
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
        // QW15a: Identifier OP IntLiteral — most common in loops/conditions
        // Saves 2× eval_expr dispatch (~80ns) per binary expression
        if let Expr::Identifier(lhs) = binary.left.as_ref() {
            if let Expr::Literal(rlit) = binary.right.as_ref() {
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
                            BinaryOp::Add => return Ok(Value::Int(l.wrapping_add(r))),
                            BinaryOp::Sub => return Ok(Value::Int(l.wrapping_sub(r))),
                            BinaryOp::Mul => return Ok(Value::Int(l.wrapping_mul(r))),
                            BinaryOp::Mod if r != 0 => return Ok(Value::Int(l % r)),
                            BinaryOp::Div if r != 0 => return Ok(Value::Int(l / r)),
                            _ => {}
                        }
                    }
                }
            }
            // QW15b: Identifier OP Identifier — both Int
            if let Expr::Identifier(rhs) = binary.right.as_ref() {
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
                        BinaryOp::Add => return Ok(Value::Int(l.wrapping_add(r))),
                        BinaryOp::Sub => return Ok(Value::Int(l.wrapping_sub(r))),
                        BinaryOp::Mul => return Ok(Value::Int(l.wrapping_mul(r))),
                        BinaryOp::Mod if r != 0 => return Ok(Value::Int(l % r)),
                        BinaryOp::Div if r != 0 => return Ok(Value::Int(l / r)),
                        _ => {}
                    }
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
            BinaryOp::Sub => self.eval_arithmetic(&left, &right, |a, b| a - b, |a, b| a - b, &binary.span),
            BinaryOp::Mul => self.eval_arithmetic(&left, &right, |a, b| a * b, |a, b| a * b, &binary.span),
            BinaryOp::Div => self.eval_div(&left, &right, &binary.span),
            BinaryOp::Mod => self.eval_arithmetic(&left, &right, |a, b| a % b, |a, b| a % b, &binary.span),
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
                        message: format!("logical AND requires boolean operands, got {:?}", left),
                        span: binary.span,
                    }),
                };
                let right_bool = match &right {
                    Value::Bool(b) => *b,
                    _ => return Err(RuntimeError::Generic {
                        message: format!("logical AND requires boolean operands, got {:?}", right),
                        span: binary.span,
                    }),
                };
                Ok(Value::Bool(left_bool && right_bool))
            }
            BinaryOp::Or => {
                let left_bool = match &left {
                    Value::Bool(b) => *b,
                    _ => return Err(RuntimeError::Generic {
                        message: format!("logical OR requires boolean operands, got {:?}", left),
                        span: binary.span,
                    }),
                };
                let right_bool = match &right {
                    Value::Bool(b) => *b,
                    _ => return Err(RuntimeError::Generic {
                        message: format!("logical OR requires boolean operands, got {:?}", right),
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
                        message: format!("logical NOT requires boolean operand, got {:?}", operand),
                        span: unary.span,
                    }),
                }
            }
            zymbol_common::UnaryOp::Neg => {
                match operand {
                    Value::Int(n) => Ok(Value::Int(-n)),
                    Value::Float(f) => Ok(Value::Float(-f)),
                    _ => Err(RuntimeError::Generic {
                        message: format!("negation requires numeric operand, got {:?}", operand),
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
