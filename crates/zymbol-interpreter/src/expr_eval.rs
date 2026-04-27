//! Expression evaluation for Zymbol-Lang
//!
//! Handles runtime evaluation of specific expression types:
//! - Iterables: Ranges, arrays, strings (for loops)
//! - Identifiers: Variable lookup
//! - Member access: Tuple fields, module constants
//! - Indexing: Arrays, tuples, strings

use zymbol_ast::{Expr, IdentifierExpr, IndexExpr};
use crate::{Interpreter, Result, RuntimeError, Value};
use std::io::Write;

impl<W: Write> Interpreter<W> {
    /// Evaluate an iterable expression (range, array, or string)
    /// Used primarily for for-each loops
    pub(crate) fn eval_iterable(&mut self, expr: &Expr) -> Result<Vec<Value>> {
        match expr {
            Expr::Range(range_expr) => {
                // Evaluate start and end
                let start_value = self.eval_expr(&range_expr.start)?;
                let end_value = self.eval_expr(&range_expr.end)?;

                // Extract integers
                let start = match start_value {
                    Value::Int(n) => n,
                    _ => {
                        return Err(RuntimeError::Generic {
                            message: format!("range start must be an integer, got {:?}", start_value),
                            span: range_expr.start.span(),
                        })
                    }
                };

                let end = match end_value {
                    Value::Int(n) => n,
                    _ => {
                        return Err(RuntimeError::Generic {
                            message: format!("range end must be an integer, got {:?}", end_value),
                            span: range_expr.end.span(),
                        })
                    }
                };

                // Evaluate optional step (default: 1)
                let step = if let Some(step_expr) = &range_expr.step {
                    let step_value = self.eval_expr(step_expr)?;
                    match step_value {
                        Value::Int(n) if n > 0 => n,
                        Value::Int(n) if n <= 0 => {
                            return Err(RuntimeError::Generic {
                                message: format!("step must be positive, got {}", n),
                                span: step_expr.span(),
                            })
                        }
                        _ => {
                            return Err(RuntimeError::Generic {
                                message: format!("step must be an integer, got {:?}", step_value),
                                span: step_expr.span(),
                            })
                        }
                    }
                } else {
                    1  // Default step
                };

                // Create range vector (inclusive) with step
                // Support both forward (1..10:2) and reverse (10..1:2) ranges
                let values: Vec<Value> = if start <= end {
                    // Forward range: 1..10:2 → [1, 3, 5, 7, 9]
                    (0..)
                        .map(|i| start + i * step)
                        .take_while(|&x| x <= end)
                        .map(Value::Int)
                        .collect()
                } else {
                    // Reverse range: 10..1:2 → [10, 8, 6, 4, 2]
                    (0..)
                        .map(|i| start - i * step)
                        .take_while(|&x| x >= end)
                        .map(Value::Int)
                        .collect()
                };

                Ok(values)
            }
            _ => {
                // Try to evaluate as expression - might be an array, string, or identifier
                let value = self.eval_expr(expr)?;
                match value {
                    Value::Array(elements) => Ok(elements),
                    Value::String(s) => {
                        // Convert string to array of chars for iteration
                        Ok(s.chars().map(Value::Char).collect())
                    }
                    _ => Err(RuntimeError::Generic {
                        message: format!(
                            "can only iterate over ranges, arrays, and strings, got {:?}",
                            value
                        ),
                        span: expr.span(),
                    }),
                }
            }
        }
    }

    /// Evaluate an identifier (variable reference)
    pub(crate) fn eval_identifier(&self, ident: &IdentifierExpr) -> Result<Value> {
        self.check_variable_alive(&ident.name, &ident.span)?;

        if let Some(val) = self.get_variable(&ident.name) {
            return Ok(val.clone());
        }

        // Not in scope — check if it's a named function used as a first-class value.
        // Captures the current scope at point of use (Opción A).
        if let Some(func_def) = self.functions.get(&ident.name) {
            return Ok(Value::Function(self.func_def_to_value(func_def)));
        }

        Err(RuntimeError::Generic {
            message: format!("undefined variable: '{}'", ident.name),
            span: ident.span,
        })
    }

    /// Evaluate member access expression: object.field
    /// Handles both module constants (module.CONSTANT) and named tuple fields (tuple.field)
    pub(crate) fn eval_member_access(&mut self, member: &zymbol_ast::MemberAccessExpr) -> Result<Value> {
        // Check if the object is a module alias (for module.CONSTANT access)
        if let Expr::Identifier(id) = member.object.as_ref() {
            if let Some(module_path) = self.import_aliases.get(&id.name) {
                // This is a module constant access
                let module = self.loaded_modules.get(module_path).ok_or_else(|| {
                    RuntimeError::Generic {
                        message: format!("Module '{}' not loaded", id.name),
                        span: member.span,
                    }
                })?;

                // Look up the constant in the module
                if let Some(constant_value) = module.constants.get(&member.field) {
                    return Ok(constant_value.clone());
                } else {
                    let available_constants: Vec<String> = module.constants.keys()
                        .cloned()
                        .collect();
                    return Err(RuntimeError::Generic {
                        message: format!(
                            "Module '{}' has no constant '{}'. Available constants: {}",
                            id.name,
                            member.field,
                            if available_constants.is_empty() {
                                "none".to_string()
                            } else {
                                available_constants.join(", ")
                            }
                        ),
                        span: member.span,
                    });
                }
            }
        }

        // Not a module access, evaluate as regular member access (for named tuples)
        let object = self.eval_expr(&member.object)?;

        match object {
            Value::NamedTuple(fields) => {
                // Search for field by name
                for (field_name, field_value) in &fields {
                    if field_name == &member.field {
                        return Ok(field_value.clone());
                    }
                }
                // Field not found
                let available_fields: Vec<String> = fields.iter()
                    .map(|(name, _)| name.clone())
                    .collect();
                Err(RuntimeError::Generic {
                    message: format!(
                        "Named tuple has no field '{}'. Available fields: {}",
                        member.field,
                        available_fields.join(", ")
                    ),
                    span: member.span,
                })
            }
            Value::Tuple(_) => {
                Err(RuntimeError::Generic {
                    message: format!(
                        "Cannot access field '{}' on positional tuple. Use positional indexing like tuple[1]",
                        member.field
                    ),
                    span: member.span,
                })
            }
            _ => {
                Err(RuntimeError::Generic {
                    message: format!(
                        "Cannot access member '{}' on non-tuple value",
                        member.field
                    ),
                    span: member.span,
                })
            }
        }
    }

    /// Evaluate array/tuple indexing
    /// Supports arrays, tuples (both positional and named), and strings
    pub(crate) fn eval_index(&mut self, idx: &IndexExpr) -> Result<Value> {
        let collection_value = self.eval_expr(&idx.array)?;
        let index_value = self.eval_expr(&idx.index)?;

        // Extract index
        let index = match index_value {
            Value::Int(n) => n,
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("index must be an integer, got {:?}", index_value),
                    span: idx.span,
                })
            }
        };

        // Handle both arrays and tuples
        match collection_value {
            Value::Array(ref arr) => {
                let len = arr.len();
                let i = if index == 0 {
                    return Err(RuntimeError::Generic {
                        message: "index 0 is invalid — Zymbol uses 1-based indexing (use 1 for the first element, -1 for the last)".to_string(),
                        span: idx.span,
                    });
                } else if index < 0 {
                    len as i64 + index
                } else {
                    index - 1
                };
                if i < 0 || i as usize >= len {
                    return Err(RuntimeError::Generic {
                        message: format!(
                            "array index out of bounds: index {} for array of length {}",
                            index,
                            len
                        ),
                        span: idx.span,
                    });
                }

                Ok(arr[i as usize].clone())
            }
            Value::Tuple(ref elements) => {
                let len = elements.len();
                let i = if index == 0 {
                    return Err(RuntimeError::Generic {
                        message: "index 0 is invalid — Zymbol uses 1-based indexing (use 1 for the first element, -1 for the last)".to_string(),
                        span: idx.span,
                    });
                } else if index < 0 {
                    len as i64 + index
                } else {
                    index - 1
                };
                if i < 0 || i as usize >= len {
                    return Err(RuntimeError::Generic {
                        message: format!(
                            "tuple index out of bounds: index {} for tuple of length {}",
                            index,
                            len
                        ),
                        span: idx.span,
                    });
                }

                Ok(elements[i as usize].clone())
            }
            Value::NamedTuple(ref fields) => {
                // Named tuples support positional indexing (backward compatibility)
                let len = fields.len();
                let i = if index == 0 {
                    return Err(RuntimeError::Generic {
                        message: "index 0 is invalid — Zymbol uses 1-based indexing (use 1 for the first element, -1 for the last)".to_string(),
                        span: idx.span,
                    });
                } else if index < 0 {
                    len as i64 + index
                } else {
                    index - 1
                };
                if i < 0 || i as usize >= len {
                    return Err(RuntimeError::Generic {
                        message: format!(
                            "named tuple index out of bounds: index {} for tuple of length {}",
                            index,
                            len
                        ),
                        span: idx.span,
                    });
                }

                Ok(fields[i as usize].1.clone())
            }
            Value::String(ref s) => {
                // String indexing returns a char
                let chars: Vec<char> = s.chars().collect();
                let len = chars.len();
                let i = if index == 0 {
                    return Err(RuntimeError::Generic {
                        message: "index 0 is invalid — Zymbol uses 1-based indexing (use 1 for the first element, -1 for the last)".to_string(),
                        span: idx.span,
                    });
                } else if index < 0 {
                    len as i64 + index
                } else {
                    index - 1
                };

                if i < 0 || i as usize >= len {
                    return Err(RuntimeError::Generic {
                        message: format!(
                            "string index out of bounds: index {} for string of length {}",
                            index,
                            len
                        ),
                        span: idx.span,
                    });
                }

                Ok(Value::Char(chars[i as usize]))
            }
            _ => {
                Err(RuntimeError::Generic {
                    message: format!("cannot index {:?} - only arrays, tuples, and strings are indexable", collection_value),
                    span: idx.span,
                })
            }
        }
    }
}
