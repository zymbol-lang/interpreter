//! Arithmetic and comparison operations for Zymbol-Lang
//!
//! Handles runtime evaluation of:
//! - Arithmetic operations: +, -, *, /, %, ** (pow)
//! - Comparison operations: ==, !=, <, >, <=, >=
//! - String concatenation: String + Any type
//! - String split: String / Char
//! - Type promotions: Int ↔ Float

use zymbol_common::BinaryOp;
use zymbol_span::Span;
use crate::{Interpreter, Result, RuntimeError, Value};
use std::io::Write;

impl<W: Write> Interpreter<W> {
    /// Evaluate numeric addition (+)
    /// Note: + is arithmetic only. Use juxtaposition for string concatenation.
    pub(crate) fn eval_add(&self, left: &Value, right: &Value, span: &Span) -> Result<Value> {
        match (left, right) {
            // Integer addition
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            // Float addition
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            // Type promotion: Int + Float → Float
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f64)),

            _ => Err(RuntimeError::Generic {
                message: format!(
                    "+ is arithmetic only — use juxtaposition to concatenate strings: \"a\" b \"c\""
                ),
                span: *span,
            }),
        }
    }

    /// Evaluate juxtaposition concatenation (implicit, no explicit operator)
    /// Converts all values to their string representation and concatenates.
    pub(crate) fn eval_concat(&self, left: &Value, right: &Value, span: &Span) -> Result<Value> {
        let l = self.value_to_concat_str(left, span)?;
        let r = self.value_to_concat_str(right, span)?;
        Ok(Value::String(format!("{}{}", l, r)))
    }

    pub(crate) fn value_to_concat_str(&self, v: &Value, span: &Span) -> Result<String> {
        match v {
            Value::String(s) => Ok(s.clone()),
            Value::Char(c) => Ok(c.to_string()),
            Value::Int(n) => Ok(n.to_string()),
            Value::Float(f) => Ok(f.to_string()),
            Value::Bool(b) => Ok(if *b { "#1" } else { "#0" }.to_string()),
            _ => Err(RuntimeError::Generic {
                message: format!("cannot juxtapose value of type {:?} in string context", v),
                span: *span,
            }),
        }
    }

    /// Evaluate arithmetic operations (sub, mul, mod)
    pub(crate) fn eval_arithmetic<F, G>(&self, left: &Value, right: &Value, int_op: F, float_op: G, span: &Span) -> Result<Value>
    where
        F: Fn(i64, i64) -> i64,
        G: Fn(f64, f64) -> f64,
    {
        match (left, right) {
            // Integer operations
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(int_op(*a, *b))),
            // Float operations
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(float_op(*a, *b))),
            // Type promotion: Int op Float → Float
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(float_op(*a as f64, *b))),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(float_op(*a, *b as f64))),
            _ => Err(RuntimeError::Generic {
                message: format!("arithmetic requires numeric operands: {:?}, {:?}", left, right),
                span: *span,
            }),
        }
    }

    /// Evaluate division (with zero check and string split)
    pub(crate) fn eval_div(&self, left: &Value, right: &Value, span: &Span) -> Result<Value> {
        match (left, right) {
            // Integer division
            (Value::Int(a), Value::Int(b)) => {
                if *b == 0 {
                    Err(RuntimeError::Generic {
                        message: "division by zero".to_string(),
                        span: *span,
                    })
                } else {
                    Ok(Value::Int(a / b))
                }
            }
            // Float division
            (Value::Float(a), Value::Float(b)) => {
                if *b == 0.0 {
                    Err(RuntimeError::Generic {
                        message: "division by zero".to_string(),
                        span: *span,
                    })
                } else {
                    Ok(Value::Float(a / b))
                }
            }
            // Type promotion: Int / Float → Float
            (Value::Int(a), Value::Float(b)) => {
                if *b == 0.0 {
                    Err(RuntimeError::Generic {
                        message: "division by zero".to_string(),
                        span: *span,
                    })
                } else {
                    Ok(Value::Float(*a as f64 / b))
                }
            }
            (Value::Float(a), Value::Int(b)) => {
                if *b == 0 {
                    Err(RuntimeError::Generic {
                        message: "division by zero".to_string(),
                        span: *span,
                    })
                } else {
                    Ok(Value::Float(a / *b as f64))
                }
            }
            _ => Err(RuntimeError::Generic {
                message: format!("/ requires numeric operands — use $/ to split strings"),
                span: *span,
            }),
        }
    }

    /// Evaluate power/exponentiation (with overflow check)
    pub(crate) fn eval_pow(&self, left: &Value, right: &Value, span: &Span) -> Result<Value> {
        match (left, right) {
            // Integer exponentiation
            (Value::Int(base), Value::Int(exp)) => {
                if *exp < 0 {
                    // Negative exponents produce floats
                    Ok(Value::Float((*base as f64).powf(*exp as f64)))
                } else {
                    // Convert exponent to u32 for pow() method
                    let exp_u32 = *exp as u32;

                    // Use checked_pow to detect overflow
                    match base.checked_pow(exp_u32) {
                        Some(result) => Ok(Value::Int(result)),
                        None => Err(RuntimeError::Generic {
                            message: format!("power operation overflow: {}^{}", base, exp),
                            span: *span,
                        }),
                    }
                }
            }
            // Float exponentiation
            (Value::Float(base), Value::Float(exp)) => Ok(Value::Float(base.powf(*exp))),
            // Type promotion: Int ^ Float → Float
            (Value::Int(base), Value::Float(exp)) => Ok(Value::Float((*base as f64).powf(*exp))),
            (Value::Float(base), Value::Int(exp)) => Ok(Value::Float(base.powf(*exp as f64))),
            _ => Err(RuntimeError::Generic {
                message: format!("power operator requires numeric operands: {:?}, {:?}", left, right),
                span: *span,
            }),
        }
    }

    /// Check if two values are equal
    pub(crate) fn values_equal(&self, left: &Value, right: &Value) -> bool {
        Self::values_equal_static(left, right)
    }

    fn values_equal_static(left: &Value, right: &Value) -> bool {
        match (left, right) {
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => (a - b).abs() < f64::EPSILON,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| Self::values_equal_static(x, y))
            }
            (Value::Tuple(a), Value::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| Self::values_equal_static(x, y))
            }
            (Value::Unit, Value::Unit) => true,
            _ => false,
        }
    }

    /// Compare two values with a comparison function
    pub(crate) fn compare_values<F, G>(
        &self,
        left: &Value,
        right: &Value,
        int_compare: F,
        float_compare: G,
        op: &BinaryOp,
    ) -> Result<Value>
    where
        F: Fn(i64, i64) -> bool,
        G: Fn(f64, f64) -> bool,
    {
        match (left, right) {
            // Integer comparison
            (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(int_compare(*a, *b))),
            // Float comparison
            (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(float_compare(*a, *b))),
            // Type promotion for comparison
            (Value::Int(a), Value::Float(b)) => Ok(Value::Bool(float_compare(*a as f64, *b))),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Bool(float_compare(*a, *b as f64))),
            // String comparison: numeric if both parse as integers, else lexicographic
            (Value::String(a), Value::String(b)) => {
                if let (Ok(a_int), Ok(b_int)) = (a.parse::<i64>(), b.parse::<i64>()) {
                    Ok(Value::Bool(int_compare(a_int, b_int)))
                } else {
                    Ok(Value::Bool(int_compare(
                        0,
                        match a.as_str().cmp(b.as_str()) {
                            std::cmp::Ordering::Less    => 1,
                            std::cmp::Ordering::Equal   => 0,
                            std::cmp::Ordering::Greater => -1,
                        },
                    )))
                }
            }
            // Handle String-Int comparisons (parse string to int)
            (Value::String(s), Value::Int(i)) => {
                if let Ok(s_int) = s.parse::<i64>() {
                    Ok(Value::Bool(int_compare(s_int, *i)))
                } else {
                    Err(RuntimeError::Generic {
                        message: format!(
                            "cannot compare string '{}' with integer {} using operator '{:?}'",
                            s, i, op
                        ),
                        span: Span::new(
                            zymbol_span::Position::start(),
                            zymbol_span::Position::start(),
                            zymbol_span::FileId(0),
                        ),
                    })
                }
            }
            (Value::Int(i), Value::String(s)) => {
                if let Ok(s_int) = s.parse::<i64>() {
                    Ok(Value::Bool(int_compare(*i, s_int)))
                } else {
                    Err(RuntimeError::Generic {
                        message: format!(
                            "cannot compare integer {} with string '{}' using operator '{:?}'",
                            i, s, op
                        ),
                        span: Span::new(
                            zymbol_span::Position::start(),
                            zymbol_span::Position::start(),
                            zymbol_span::FileId(0),
                        ),
                    })
                }
            }
            _ => Err(RuntimeError::Generic {
                message: format!(
                    "cannot compare values with operator '{:?}': {:?} and {:?}",
                    op, left, right
                ),
                span: Span::new(
                    zymbol_span::Position::start(),
                    zymbol_span::Position::start(),
                    zymbol_span::FileId(0),
                ),
            }),
        }
    }
}
