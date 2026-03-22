//! MATCH expression and pattern matching execution for Zymbol-Lang
//!
//! Handles runtime execution of:
//! - MATCH expression: ?? expr { cases }
//! - MATCH statement: ?? expr { cases } (discards value)
//! - Pattern matching: All pattern types (literals, ranges, lists, guards)

use zymbol_ast::{MatchExpr, Pattern};
use zymbol_common::Literal;
use crate::{Interpreter, Result, RuntimeError, Value};
use std::io::Write;

/// B7: Convert a literal directly to a Value without going through eval_expr.
/// Used for fast path in Pattern::Range when bounds are compile-time constants.
#[inline(always)]
fn literal_to_value(lit: &Literal) -> Value {
    match lit {
        Literal::String(s) => Value::String(s.clone()),
        Literal::Int(n)    => Value::Int(*n),
        Literal::Float(f)  => Value::Float(*f),
        Literal::Char(c)   => Value::Char(*c),
        Literal::Bool(b)   => Value::Bool(*b),
    }
}

impl<W: Write> Interpreter<W> {
    /// Execute match statement: ?? expr { cases } (discards return value)
    pub(crate) fn execute_match_statement(&mut self, match_expr: &MatchExpr) -> Result<()> {
        // Check if match returns values (warn if unused)
        let has_values = match_expr.cases.iter().any(|case| case.value.is_some());

        if has_values {
            // Warning: match returns values but they're being discarded
            eprintln!("warning: match expression returns values but result is unused");
            eprintln!("  --> consider assigning to a variable: `result = ?? expr {{ ... }}`");
            eprintln!("  --> or use execution-only form: `?? expr {{ pattern : {{ block }} }}`");
        }

        // Execute match as statement (discard return value)
        self.eval_match(match_expr)?;
        Ok(())
    }

    /// Evaluate a match expression
    pub(crate) fn eval_match(&mut self, match_expr: &MatchExpr) -> Result<Value> {
        // Evaluate the scrutinee (the value being matched)
        let scrutinee_value = self.eval_expr(&match_expr.scrutinee)?;

        // Try each case in order
        for case in &match_expr.cases {
            // Check if pattern matches
            if let Some(matched) = self.pattern_matches(&case.pattern, &scrutinee_value)? {
                if matched {
                    // Pattern matched
                    // Determine return value: either from value expr or Unit
                    let result = if let Some(ref value_expr) = case.value {
                        self.eval_expr(value_expr)?
                    } else {
                        Value::Unit
                    };

                    // Execute optional block (for side effects)
                    if let Some(ref block) = case.block {
                        self.execute_block(block)?;
                    }

                    return Ok(result);
                }
            }
        }

        // No pattern matched
        Err(RuntimeError::Generic {
            message: "no pattern matched in match expression".to_string(),
            span: match_expr.span,
        })
    }

    /// Check if a pattern matches a value
    /// Returns Some(true) if matched, Some(false) if not matched (but guard failed), None if pattern doesn't match
    pub(crate) fn pattern_matches(&mut self, pattern: &Pattern, value: &Value) -> Result<Option<bool>> {
        match pattern {
            Pattern::Wildcard(_) => {
                // Wildcard matches everything
                Ok(Some(true))
            }
            Pattern::Literal(lit, _) => {
                // Check if literal equals value
                let pattern_value = match lit {
                    Literal::String(s) => Value::String(s.clone()),
                    Literal::Int(n) => Value::Int(*n),
                    Literal::Float(f) => Value::Float(*f),
                    Literal::Char(c) => Value::Char(*c),
                    Literal::Bool(b) => Value::Bool(*b),
                };

                if self.values_equal(&pattern_value, value) {
                    Ok(Some(true))
                } else {
                    Ok(None)
                }
            }
            Pattern::Range(start_expr, end_expr, span) => {
                // B7: fast path for constant literal bounds (most common case).
                // Avoids 37-arm eval_expr dispatch when bounds are compile-time constants.
                use zymbol_ast::Expr;
                let start_val = match start_expr.as_ref() {
                    Expr::Literal(lit) => literal_to_value(&lit.value),
                    other => self.eval_expr(other)?,
                };
                let end_val = match end_expr.as_ref() {
                    Expr::Literal(lit) => literal_to_value(&lit.value),
                    other => self.eval_expr(other)?,
                };

                // Check if value is within range [start, end] inclusive
                let in_range = match (value, &start_val, &end_val) {
                    (Value::Int(v), Value::Int(s), Value::Int(e)) => v >= s && v <= e,
                    (Value::Char(v), Value::Char(s), Value::Char(e)) => v >= s && v <= e,
                    _ => {
                        return Err(RuntimeError::Generic {
                            message: "range pattern type mismatch".to_string(),
                            span: *span,
                        });
                    }
                };

                if in_range {
                    Ok(Some(true))
                } else {
                    Ok(None)
                }
            }
            Pattern::List(patterns, _span) => {
                // Match against list/array
                match value {
                    Value::Array(arr) => {
                        // Check if lengths match
                        if patterns.len() != arr.len() {
                            return Ok(None);
                        }

                        // Check each element
                        for (pattern, val) in patterns.iter().zip(arr.iter()) {
                            if let Some(matched) = self.pattern_matches(pattern, val)? {
                                if !matched {
                                    return Ok(None);
                                }
                            } else {
                                return Ok(None);
                            }
                        }

                        Ok(Some(true))
                    }
                    _ => Ok(None),
                }
            }
            Pattern::Guard(inner_pattern, condition, _) => {
                // First check if inner pattern matches
                if let Some(matched) = self.pattern_matches(inner_pattern, value)? {
                    if matched {
                        // Pattern matched, now check guard condition
                        let guard_result = self.eval_expr(condition)?;
                        match guard_result {
                            Value::Bool(true) => Ok(Some(true)),
                            Value::Bool(false) => Ok(Some(false)), // Pattern matched but guard failed
                            _ => Err(RuntimeError::Generic {
                                message: "guard condition must evaluate to boolean".to_string(),
                                span: condition.span(),
                            }),
                        }
                    } else {
                        Ok(Some(false))
                    }
                } else {
                    Ok(None)
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
    fn test_execution_only_match() {
        let code = r#"
score = 95
?? score {
    90..100 : { >> "A" ¶ }
    80..89 : { >> "B" ¶ }
    _ : { >> "F" ¶ }
}
"#;
        let output = run(code);
        assert_eq!(output, "A\n");
    }

    #[test]
    fn test_execution_only_match_wildcard() {
        let code = r#"
score = 50
?? score {
    90..100 : { >> "Excellent" ¶ }
    _ : { >> "Need improvement" ¶ }
}
"#;
        let output = run(code);
        assert_eq!(output, "Need improvement\n");
    }

    #[test]
    fn test_execution_only_match_multiple_statements() {
        let code = r#"
status = "PROCESSING"
?? status {
    "PENDING" : { >> "Waiting" ¶ }
    "PROCESSING" : {
        >> "Processing order" ¶
        >> "Updating inventory" ¶
    }
    _ : { >> "Unknown" ¶ }
}
"#;
        let output = run(code);
        assert_eq!(output, "Processing order\nUpdating inventory\n");
    }

    #[test]
    fn test_mixed_match_value_and_execution() {
        let code = r#"
score = 95
grade = ?? score {
    90..100 : 'A' { >> "Excellent!" ¶ }
    80..89 : 'B'
    _ : 'F'
}
>> grade ¶
"#;
        let output = run(code);
        assert_eq!(output, "Excellent!\nA\n");
    }

    #[test]
    fn test_match_as_statement_discards_return() {
        let code = r#"
x = 10
?? x {
    10 : "ten"
    20 : "twenty"
    _ : "other"
}
>> "done" ¶
"#;
        let output = run(code);
        assert_eq!(output, "done\n");
    }
}
