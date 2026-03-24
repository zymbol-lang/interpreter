//! String operation evaluation for Zymbol-Lang
//!
//! Handles runtime execution of string-specific operators:
//! - $~~ (replace pattern with replacement text)
//!
//! Note: $?? is now CollectionFindAllExpr (eval_collection_find_all in collection_ops.rs).
//! $++ and old $-- are retired in v0.0.2.

use zymbol_ast::StringReplaceExpr;
use crate::{Interpreter, Result, RuntimeError, Value};
use std::io::Write;

impl<W: Write> Interpreter<W> {
    /// Evaluate string replace operator: string$~~[pattern:replacement] or string$~~[pattern:replacement:count]
    /// Replaces pattern with replacement text
    /// - If count not provided or is 0, replaces all occurrences
    /// - If count is N, replaces first N occurrences
    pub(crate) fn eval_string_replace(&mut self, op: &StringReplaceExpr) -> Result<Value> {
        let string_value = self.eval_expr(&op.string)?;
        let pattern_value = self.eval_expr(&op.pattern)?;
        let replacement_value = self.eval_expr(&op.replacement)?;

        // Extract string
        let string = match string_value {
            Value::String(ref s) => s.clone(),
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("$~~ requires a string, got {:?}", string_value),
                    span: op.span,
                })
            }
        };

        // Extract replacement
        let replacement = match replacement_value {
            Value::String(ref s) => s.clone(),
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("$~~ replacement must be a string, got {:?}", replacement_value),
                    span: op.span,
                })
            }
        };

        // Extract optional count
        let max_replacements = if let Some(count_expr) = &op.count {
            let count_value = self.eval_expr(count_expr)?;
            match count_value {
                Value::Int(n) if n < 0 => {
                    return Err(RuntimeError::Generic {
                        message: format!("replacement count must be non-negative, got {}", n),
                        span: op.span,
                    });
                }
                Value::Int(0) => None, // 0 means replace all
                Value::Int(n) => Some(n as usize),
                _ => {
                    return Err(RuntimeError::Generic {
                        message: format!("$~~ count must be an integer, got {:?}", count_value),
                        span: op.span,
                    })
                }
            }
        } else {
            None // No count means replace all
        };

        // Pattern can be String or Char
        let result = match pattern_value {
            Value::String(ref pattern) => {
                if pattern.is_empty() {
                    // Empty pattern - return original string
                    return Ok(Value::String(string));
                }

                // Perform replacement
                if let Some(max) = max_replacements {
                    // Replace first N occurrences
                    let mut result = string.clone();
                    let mut count = 0;
                    while count < max {
                        if let Some(pos) = result.find(pattern) {
                            // Replace this occurrence
                            let before = &result[0..pos];
                            let after = &result[pos + pattern.len()..];
                            result = format!("{}{}{}", before, replacement, after);
                            count += 1;
                        } else {
                            break; // No more occurrences
                        }
                    }
                    result
                } else {
                    // Replace all occurrences
                    string.replace(pattern, &replacement)
                }
            }
            Value::Char(ch) => {
                // Convert char to string and replace
                let pattern_str = ch.to_string();

                if let Some(max) = max_replacements {
                    // Replace first N occurrences
                    let mut result = string.clone();
                    let mut count = 0;
                    while count < max {
                        if let Some(pos) = result.find(&pattern_str) {
                            // Replace this occurrence
                            let before = &result[0..pos];
                            let after = &result[pos + pattern_str.len()..];
                            result = format!("{}{}{}", before, replacement, after);
                            count += 1;
                        } else {
                            break; // No more occurrences
                        }
                    }
                    result
                } else {
                    // Replace all occurrences
                    string.replace(&pattern_str, &replacement)
                }
            }
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("$~~ pattern must be a string or char, got {:?}", pattern_value),
                    span: op.span,
                })
            }
        };

        Ok(Value::String(result))
    }
}
