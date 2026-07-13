//! Literal evaluation for Zymbol-Lang
//!
//! Handles runtime evaluation of literal values:
//! - Strings, numbers, chars, booleans

use std::io::Write;
use zymbol_ast::LiteralExpr;
use zymbol_common::Literal;
use zymbol_span::Span;

use crate::{Interpreter, Result, RuntimeError, Value};

impl<W: Write> Interpreter<W> {
    /// Evaluate a literal expression
    pub(crate) fn eval_literal(&mut self, lit: &LiteralExpr) -> Result<Value> {
        Ok(match &lit.value {
            // Plain string — never interpolated; sentinels → literal braces
            Literal::String(s) => Value::String(s.replace('\x01', "{").replace('\x02', "}")),
            // Interpolated string — resolve {var} at runtime, then restore escaped braces
            Literal::InterpolatedString(s) => {
                Value::String(self.interpolate_string(s, &lit.span)?.replace('\x01', "{").replace('\x02', "}"))
            }
            Literal::Int(n) => Value::Int(*n),
            Literal::Float(f) => Value::Float(*f),
            Literal::Char(c) => Value::Char(*c),
            Literal::Bool(b) => Value::Bool(*b),
        })
    }

    /// Expand `{var}` placeholders in a string using the current scope.
    fn interpolate_string(&mut self, s: &str, span: &Span) -> Result<String> {
        let mut result = String::with_capacity(s.len());
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '{' {
                // Find closing '}'
                let start = i + 1;
                let mut end = start;
                while end < chars.len() && chars[end] != '}' {
                    end += 1;
                }
                if end < chars.len() {
                    let var_name: String = chars[start..end].iter().collect();
                    // Look up the variable; if not found, leave `{var}` as-is
                    let maybe_display = self.get_variable(&var_name)
                        .map(|v| v.to_display_string());
                    if let Some(display) = maybe_display {
                        result.push_str(&display);
                    } else {
                        // Auto-free is invisible by design: a missing variable
                        // that the last-use schedule destroyed means the
                        // analyzer missed this interpolation — fail loudly
                        // instead of silently printing the literal `{var}`.
                        if self.auto_dead_variables.contains(&var_name) {
                            return Err(RuntimeError::Generic {
                                message: format!(
                                    "internal: use of '{}' after auto-destruction (string interpolation) — this is a bug in the last-use analyzer, please report it",
                                    var_name
                                ),
                                span: *span,
                            });
                        }
                        result.push('{');
                        result.push_str(&var_name);
                        result.push('}');
                    }
                    i = end + 1;
                } else {
                    // No closing brace: emit as literal
                    result.push('{');
                    i += 1;
                }
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }
        Ok(result)
    }
}
