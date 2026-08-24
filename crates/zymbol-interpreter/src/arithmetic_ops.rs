//! Arithmetic and comparison operations for Zymbol-Lang
//!
//! Handles runtime evaluation of:
//! - Arithmetic operations: +, -, *, /, %, ** (pow)
//! - Comparison operations: ==, !=, <, >, <=, >=
//! - String concatenation: String + Any type
//! - String split: String / Char
//! - Type promotions: Int ↔ Float

use zymbol_common::num;
use zymbol_common::BinaryOp;
use zymbol_span::Span;
use crate::data_ops::ascii_digits;
use crate::numeral_mode::{to_numeral_int, to_numeral_float, to_numeral_bool};
use crate::{Interpreter, Result, RuntimeError, Value};
use std::io::Write;

/// The integer a string holds, with digits from any of the 69 supported scripts
/// (`"४२"` → 42). `None` when the string is not an integer.
pub(crate) fn str_as_int(s: &str) -> Option<i64> {
    ascii_digits(s.trim()).parse::<i64>().ok().filter(|n| num::in_int_range(*n))
}

/// `str_as_int` for numbers with a fractional part or an exponent.
pub(crate) fn str_as_float(s: &str) -> Option<f64> {
    ascii_digits(s.trim()).parse::<f64>().ok()
}

/// Lift an in-range integer into a `Value`, or report the overflow the way all
/// four engines report it. The operands are echoed because `integer overflow`
/// on its own tells a reader nothing about which of the operations on the line
/// produced it.
/// Turn a checked integer result into a value or a `##Range` error.
///
/// `pub(crate)` because the fast paths in `expressions.rs` and
/// `variables.rs` have to raise the same error in the same words: they used
/// to wrap instead, which is `DM-01`.
pub(crate) fn int_result(v: Option<i64>, a: i64, op: &str, b: i64, span: &Span) -> Result<Value> {
    match v {
        Some(n) => Ok(Value::Int(n)),
        None => Err(RuntimeError::Generic {
            message: num::overflow_msg(a, op, b),
            span: *span,
        }),
    }
}

impl<W: Write> Interpreter<W> {
    /// Evaluate numeric addition (+)
    /// Note: + is arithmetic only. Use juxtaposition for string concatenation.
    pub(crate) fn eval_add(&self, left: &Value, right: &Value, span: &Span) -> Result<Value> {
        match (left, right) {
            // Integer addition
            (Value::Int(a), Value::Int(b)) => int_result(num::add(*a, *b), *a, "+", *b, span),
            // Float addition
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            // Type promotion: Int + Float → Float
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f64)),

            _ => Err(RuntimeError::Generic {
                message: "+ is arithmetic only — use juxtaposition to concatenate strings: \"a\" b \"c\"".to_string(),
                span: *span,
            }),
        }
    }

    /// Evaluate juxtaposition concatenation (implicit, no explicit operator)
    /// Converts all values to their string representation and concatenates.
    pub(crate) fn eval_concat(&self, left: &Value, right: &Value, span: &Span) -> Result<Value> {
        let _ = span;
        let l = self.value_to_concat_str(left);
        let r = self.value_to_concat_str(right);
        Ok(Value::String(format!("{}{}", l, r)))
    }

    /// The string a value contributes to a juxtaposition. Cannot fail: every
    /// value has one, and it is the one `>>` prints.
    pub(crate) fn value_to_concat_str(&self, v: &Value) -> String {
        match v {
            Value::String(s) => s.clone(),
            Value::Char(c) => c.to_string(),
            Value::Int(n) => to_numeral_int(*n, self.numeral_mode),
            Value::Float(f) => to_numeral_float(*f, self.numeral_mode),
            Value::Bool(b) => to_numeral_bool(*b, self.numeral_mode),
            // GAP-ZYB-008 (and BUG-ZYB-003 before it): every value juxtaposes,
            // and to exactly what `>>` prints.
            //
            // There used to be a whitelist here, and `>>` did not use it — so
            // the same juxtaposition of the same value gave two answers
            // depending on where it was written. `>> "arr: " a ¶` printed
            // `arr: [1, 2, 3]` and `s = "" a` aborted, which meant an array
            // could not go into a log line, an error message, a stored value or
            // a test comparison. Only printed, once, and never kept.
            //
            // The register VM and the browser engine had no whitelist and never
            // had the split; this is the tree-walker catching up to them and to
            // its own `>>`.
            _ => v.to_display_string_in(self.numeral_mode),
        }
    }

    /// Evaluate arithmetic operations (sub, mul, mod)
    ///
    /// `int_op` returns `None` when the result is not a Zymbol integer; `op` is
    /// how the operator is spelled back in that error. The integer and float
    /// paths are separate because only the integer one has a range to leave —
    /// a float that overflows yields `inf`, which is a value.
    pub(crate) fn eval_arithmetic<F, G>(&self, left: &Value, right: &Value, int_op: F, float_op: G, op: &str, span: &Span) -> Result<Value>
    where
        F: Fn(i64, i64) -> Option<i64>,
        G: Fn(f64, f64) -> f64,
    {
        match (left, right) {
            // Integer operations
            (Value::Int(a), Value::Int(b)) => int_result(int_op(*a, *b), *a, op, *b, span),
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

    /// Evaluate modulo (%)
    ///
    /// Split out of `eval_arithmetic` because a zero divisor is not an overflow
    /// and because `a % b` on integers *panics* in Rust when `b` is 0 — the one
    /// arithmetic case where the tree-walker used to abort the process instead
    /// of raising a Zymbol error the program could catch.
    pub(crate) fn eval_mod(&self, left: &Value, right: &Value, span: &Span) -> Result<Value> {
        // A zero divisor of *either* type, matching `eval_div`. Checking only
        // `Int % Int` left `1 % 0.0` and `1.0 % 0` answering NaN here while the
        // OCaml and browser engines raised — and while `1 / 0.0` raised in all
        // four. Whether a zero divisor is an error cannot depend on which of the
        // two numeric types the zero was written as.
        let divides_by_zero = matches!(right, Value::Int(0)) || matches!(right, Value::Float(f) if *f == 0.0);
        if divides_by_zero && matches!(left, Value::Int(_) | Value::Float(_)) {
            return Err(RuntimeError::Generic {
                message: "modulo by zero".to_string(),
                span: *span,
            });
        }
        // The remainder of two in-range integers is always in range, so the
        // integer path can never overflow once the divisor is known non-zero.
        self.eval_arithmetic(left, right, |a, b| Some(a % b), |a, b| a % b, "%", span)
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
                message: "/ requires numeric operands — use $/ to split strings".to_string(),
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
                    // An exponent too large for u32 cannot produce an in-range
                    // result anyway (except for the bases below, which `num::pow`
                    // settles), so clamping it is safe and keeps the error the
                    // same one every other overflow reports.
                    let exp_u32 = u32::try_from(*exp).unwrap_or(u32::MAX);
                    int_result(num::pow(*base, exp_u32), *base, "^", *exp, span)
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
            // Float equality is exact, as IEEE-754 defines it and as the other
            // three engines have always implemented it.
            //
            // This used to be `(a - b).abs() < f64::EPSILON`, which is an
            // *absolute* tolerance: `f64::EPSILON` is the spacing of the floats
            // near 1.0, so the test said every pair of values closer together
            // than 2.2e-16 was equal — including `1e-20 == -5e-20`, a positive
            // and a negative. It also made equality non-transitive, and near
            // 1e300 it did nothing at all, the spacing there being ~1e284.
            //
            // The tolerance was introduced for Int/Float promotion (so that
            // `##.0 == 0` agreed with `##.0 >= 0`), but promotion is what fixes
            // that, not the epsilon — the two arms below promote and compare
            // exactly, and `1.0 == 1` is still true. A program that wants a
            // tolerance should name it: `(a - b)$abs < 0.001`.
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Int(a), Value::Float(b)) => *a as f64 == *b,
            (Value::Float(a), Value::Int(b)) => *a == *b as f64,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| Self::values_equal_static(x, y))
            }
            (Value::Tuple(a), Value::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| Self::values_equal_static(x, y))
            }
            // Two dictionaries are equal when they hold the same keys with the
            // same values (DM-22). They compared as `#0` here while the browser
            // engine said `#1`, and `#0` was indefensible: every other
            // collection compares by value, and a dictionary that never equals
            // another cannot be tested, deduplicated or asserted on.
            //
            // Key ORDER is not part of it. Insertion order is preserved for
            // walking, as in Python's dict, but two dictionaries built in a
            // different order still hold the same thing.
            (Value::NamedTuple(a), Value::NamedTuple(b)) => {
                a.len() == b.len()
                    && a.iter().all(|(ka, va)| {
                        b.iter().any(|(kb, vb)| ka == kb && Self::values_equal_static(va, vb))
                    })
            }
            (Value::Unit, Value::Unit) => true,
            // Two functions are equal when they are THE SAME function — the
            // same definition for a named one, the same evaluation for a lambda
            // (BUG-ZYB-012). There was no arm here at all, so every comparison
            // between two functions answered `#0`, including a function against
            // itself; the browser engine answered `#1` to every one of them,
            // including a named function against a lambda, because its fallback
            // compared two `undefined`s. Neither had been decided: nothing
            // documented what `==` means on a function, and no corpus file
            // compared two, so the gate could not see it.
            //
            // Identity and not structure: two functions with identical bodies
            // are two functions. See `FnIdentity`.
            (Value::Function(a), Value::Function(b)) => a == b,
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
            // Chars and Bools order by code point / #0 < #1. The VM has always
            // compared them; the tree-walker used to answer with an error, so
            // `'a' < 'b'` meant two different things depending on the engine.
            (Value::Char(a), Value::Char(b)) => Ok(Value::Bool(int_compare(*a as i64, *b as i64))),
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(int_compare(*a as i64, *b as i64))),
            // String comparison: numeric when both sides are numbers, lexicographic
            // when both are text. "Number" means what `#|…|` accepts — digits from
            // any of the 69 supported scripts, so `"४२" > "९"` compares 42 against
            // 9 exactly as `"42" > "9"` does. Anything else would make every script
            // but ASCII a second-class citizen of its own language.
            (Value::String(a), Value::String(b)) => {
                if let (Some(a_int), Some(b_int)) = (str_as_int(a), str_as_int(b)) {
                    Ok(Value::Bool(int_compare(a_int, b_int)))
                } else if let (Some(a_f), Some(b_f)) = (str_as_float(a), str_as_float(b)) {
                    Ok(Value::Bool(float_compare(a_f, b_f)))
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
            // String against a number: the string has to be a number too (in any script)
            (Value::String(s), Value::Int(i)) => {
                if let Some(s_int) = str_as_int(s) {
                    Ok(Value::Bool(int_compare(s_int, *i)))
                } else if let Some(s_f) = str_as_float(s) {
                    Ok(Value::Bool(float_compare(s_f, *i as f64)))
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
                if let Some(s_int) = str_as_int(s) {
                    Ok(Value::Bool(int_compare(*i, s_int)))
                } else if let Some(s_f) = str_as_float(s) {
                    Ok(Value::Bool(float_compare(*i as f64, s_f)))
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
            (Value::String(s), Value::Float(f)) => {
                if let Some(s_f) = str_as_float(s) {
                    Ok(Value::Bool(float_compare(s_f, *f)))
                } else {
                    Err(RuntimeError::Generic {
                        message: format!(
                            "cannot compare string '{}' with float {} using operator '{:?}'",
                            s, f, op
                        ),
                        span: Span::new(
                            zymbol_span::Position::start(),
                            zymbol_span::Position::start(),
                            zymbol_span::FileId(0),
                        ),
                    })
                }
            }
            (Value::Float(f), Value::String(s)) => {
                if let Some(s_f) = str_as_float(s) {
                    Ok(Value::Bool(float_compare(*f, s_f)))
                } else {
                    Err(RuntimeError::Generic {
                        message: format!(
                            "cannot compare float {} with string '{}' using operator '{:?}'",
                            f, s, op
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

#[cfg(test)]
mod equality_tests {
    use super::*;

    // HLZ-003 — `==` promotes Int and Float, matching what `<`, `>`, `<=` and
    // `>=` already did through compare_values. Before the fix `##.0 == 0` was
    // false while `##.0 >= 0` and `##.0 <= 0` were both true, and both values
    // print identically, so the contradiction never showed on screen.

    fn eq(a: Value, b: Value) -> bool {
        Interpreter::<Vec<u8>>::values_equal_static(&a, &b)
    }

    #[test]
    fn int_and_float_of_the_same_value_are_equal() {
        assert!(eq(Value::Int(0), Value::Float(0.0)));
        assert!(eq(Value::Float(0.0), Value::Int(0)));
        assert!(eq(Value::Int(20), Value::Float(20.0)));
        assert!(eq(Value::Float(-3.0), Value::Int(-3)));
    }

    #[test]
    fn different_values_stay_different_across_types() {
        assert!(!eq(Value::Int(1), Value::Float(2.0)));
        assert!(!eq(Value::Float(1.5), Value::Int(1)));
    }

    #[test]
    fn ordering_and_equality_agree() {
        // The property that was violated: a >= b && a <= b implies a == b.
        let a = Value::Float(7.0);
        let b = Value::Int(7);
        assert!(eq(a.clone(), b.clone()));
    }

    #[test]
    fn same_type_equality_is_unchanged() {
        assert!(eq(Value::Int(5), Value::Int(5)));
        assert!(!eq(Value::Int(5), Value::Int(6)));
        assert!(eq(Value::Float(2.5), Value::Float(2.5)));
        assert!(eq(
            Value::String("a".to_string()),
            Value::String("a".to_string())
        ));
        assert!(!eq(Value::Int(1), Value::String("1".to_string())));
    }
}
