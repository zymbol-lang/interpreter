//! String operation evaluation for Zymbol-Lang
//!
//! Handles runtime execution of all string operators:
//! - $?? (find all positions of pattern in string)
//! - $++ (insert text at position)
//! - $-- (remove text by count)
//! - $~~ (replace pattern with replacement text)

use zymbol_ast::{
    StringFindPositionsExpr, StringInsertExpr, StringRemoveExpr, StringReplaceExpr,
};
use crate::{Interpreter, Result, RuntimeError, Value};
use std::io::Write;

impl<W: Write> Interpreter<W> {
    /// Evaluate string find positions operator: string$?? pattern
    /// Returns an array of integer positions where the pattern is found
    pub(crate) fn eval_string_find_positions(&mut self, op: &StringFindPositionsExpr) -> Result<Value> {
        let string_value = self.eval_expr(&op.string)?;
        let pattern_value = self.eval_expr(&op.pattern)?;

        // Extract string
        let string = match string_value {
            Value::String(ref s) => s.clone(),
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("$?? requires a string, got {:?}", string_value),
                    span: op.span,
                })
            }
        };

        // Pattern can be String or Char
        let positions = match pattern_value {
            Value::String(ref pattern) => {
                // Find all occurrences of substring
                let mut positions = Vec::new();
                let string_chars: Vec<char> = string.chars().collect();
                let pattern_chars: Vec<char> = pattern.chars().collect();

                if pattern_chars.is_empty() {
                    // Empty pattern returns empty array
                    return Ok(Value::Array(vec![]));
                }

                for i in 0..=(string_chars.len().saturating_sub(pattern_chars.len())) {
                    let substring: Vec<char> = string_chars[i..i + pattern_chars.len()].to_vec();
                    if substring == pattern_chars {
                        positions.push(Value::Int(i as i64));
                    }
                }
                positions
            }
            Value::Char(ch) => {
                // Find all occurrences of character
                let mut positions = Vec::new();
                for (i, c) in string.chars().enumerate() {
                    if c == ch {
                        positions.push(Value::Int(i as i64));
                    }
                }
                positions
            }
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("$?? pattern must be a string or char, got {:?}", pattern_value),
                    span: op.span,
                })
            }
        };

        Ok(Value::Array(positions))
    }

    /// Evaluate string insert operator: string$++[position:text]
    /// Inserts text at the specified position
    pub(crate) fn eval_string_insert(&mut self, op: &StringInsertExpr) -> Result<Value> {
        let string_value = self.eval_expr(&op.string)?;
        let position_value = self.eval_expr(&op.position)?;
        let text_value = self.eval_expr(&op.text)?;

        // Extract string
        let string = match string_value {
            Value::String(ref s) => s.clone(),
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("$++ requires a string, got {:?}", string_value),
                    span: op.span,
                })
            }
        };

        // Extract position
        let position = match position_value {
            Value::Int(n) => n,
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("$++ position must be an integer, got {:?}", position_value),
                    span: op.span,
                })
            }
        };

        // Extract text to insert
        let insert_text = match text_value {
            Value::String(ref s) => s.clone(),
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("$++ text must be a string, got {:?}", text_value),
                    span: op.span,
                })
            }
        };

        // Validate position
        let char_vec: Vec<char> = string.chars().collect();
        if position < 0 || position as usize > char_vec.len() {
            return Err(RuntimeError::Generic {
                message: format!(
                    "insert position {} out of bounds for string of length {}",
                    position,
                    char_vec.len()
                ),
                span: op.span,
            });
        }

        // Build result string
        let pos = position as usize;
        let before: String = char_vec[0..pos].iter().collect();
        let after: String = char_vec[pos..].iter().collect();
        let result = format!("{}{}{}", before, insert_text, after);

        Ok(Value::String(result))
    }

    /// Evaluate string remove operator: string$--[position:count]
    /// Removes count characters starting at position
    pub(crate) fn eval_string_remove(&mut self, op: &StringRemoveExpr) -> Result<Value> {
        let string_value = self.eval_expr(&op.string)?;
        let position_value = self.eval_expr(&op.position)?;
        let count_value = self.eval_expr(&op.count)?;

        // Extract string
        let string = match string_value {
            Value::String(ref s) => s.clone(),
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("$-- requires a string, got {:?}", string_value),
                    span: op.span,
                })
            }
        };

        // Extract position
        let position = match position_value {
            Value::Int(n) => n,
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("$-- position must be an integer, got {:?}", position_value),
                    span: op.span,
                })
            }
        };

        // Extract count
        let count = match count_value {
            Value::Int(n) => n,
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("$-- count must be an integer, got {:?}", count_value),
                    span: op.span,
                })
            }
        };

        // Validate count is non-negative
        if count < 0 {
            return Err(RuntimeError::Generic {
                message: format!("count must be non-negative, got {}", count),
                span: op.span,
            });
        }

        // Validate position
        let char_vec: Vec<char> = string.chars().collect();
        if position < 0 || position as usize > char_vec.len() {
            return Err(RuntimeError::Generic {
                message: format!(
                    "remove position {} out of bounds for string of length {}",
                    position,
                    char_vec.len()
                ),
                span: op.span,
            });
        }

        // Build result string (truncate if count exceeds remaining chars)
        let pos = position as usize;
        let end_pos = std::cmp::min(pos + count as usize, char_vec.len());

        let before: String = char_vec[0..pos].iter().collect();
        let after: String = char_vec[end_pos..].iter().collect();
        let result = format!("{}{}", before, after);

        Ok(Value::String(result))
    }

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
