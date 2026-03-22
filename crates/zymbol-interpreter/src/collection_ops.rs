//! Collection operation evaluation for Zymbol-Lang
//!
//! Handles runtime execution of all collection operators:
//! - $# (length/size)
//! - $+ (append element)
//! - $- (remove by index)
//! - $? (contains/search)
//! - $~ (update element)
//! - $[ (slice with range)
//! - $> (map - transform collection)
//! - $| (filter - select elements)
//! - $< (reduce - accumulate)

use zymbol_ast::{
    CollectionAppendExpr, CollectionContainsExpr, CollectionLengthExpr,
    CollectionRemoveExpr, CollectionSliceExpr, CollectionUpdateExpr, Expr,
};
use crate::{Interpreter, Result, RuntimeError, Value};
use std::io::Write;

impl<W: Write> Interpreter<W> {
    /// Evaluate collection length operator: collection$#
    pub(crate) fn eval_collection_length(&mut self, op: &CollectionLengthExpr) -> Result<Value> {
        let collection = self.eval_expr(&op.collection)?;

        match collection {
            Value::Array(ref arr) => Ok(Value::Int(arr.len() as i64)),
            Value::Tuple(ref tup) => Ok(Value::Int(tup.len() as i64)),
            Value::NamedTuple(ref fields) => Ok(Value::Int(fields.len() as i64)),
            Value::String(ref s) => Ok(Value::Int(if s.is_ascii() { s.len() as i64 } else { s.chars().count() as i64 })),
            _ => Err(RuntimeError::Generic {
                message: format!(
                    "cannot get length of {:?} - only arrays, tuples, and strings have length",
                    collection
                ),
                span: op.span,
            }),
        }
    }

    /// Evaluate collection append operator: collection$+ element
    pub(crate) fn eval_collection_append(&mut self, op: &CollectionAppendExpr) -> Result<Value> {
        let collection = self.eval_expr(&op.collection)?;
        let element = self.eval_expr(&op.element)?;

        match collection {
            Value::Array(mut arr) => {
                // Create a new array with the element appended (immutability)
                arr.push(element);
                Ok(Value::Array(arr))
            }
            Value::Tuple(mut tup) => {
                // Create a new tuple with the element appended (immutability)
                tup.push(element);
                Ok(Value::Tuple(tup))
            }
            _ => Err(RuntimeError::Generic {
                message: format!(
                    "cannot append to {:?} - only arrays and tuples support append",
                    collection
                ),
                span: op.span,
            }),
        }
    }

    /// Evaluate collection remove operator: collection$- index
    pub(crate) fn eval_collection_remove(&mut self, op: &CollectionRemoveExpr) -> Result<Value> {
        let collection = self.eval_expr(&op.collection)?;
        let index_value = self.eval_expr(&op.index)?;

        // Extract index as integer
        let index = match index_value {
            Value::Int(n) => n,
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("remove index must be an integer, got {:?}", index_value),
                    span: op.span,
                })
            }
        };

        match collection {
            Value::Array(mut arr) => {
                // Check bounds
                if index < 0 || index as usize >= arr.len() {
                    return Err(RuntimeError::Generic {
                        message: format!(
                            "index out of bounds: index {} for array of length {}",
                            index,
                            arr.len()
                        ),
                        span: op.span,
                    });
                }
                // Create a new array with the element removed (immutability)
                arr.remove(index as usize);
                Ok(Value::Array(arr))
            }
            Value::Tuple(mut tup) => {
                // Check bounds
                if index < 0 || index as usize >= tup.len() {
                    return Err(RuntimeError::Generic {
                        message: format!(
                            "index out of bounds: index {} for tuple of length {}",
                            index,
                            tup.len()
                        ),
                        span: op.span,
                    });
                }
                // Create a new tuple with the element removed (immutability)
                tup.remove(index as usize);
                Ok(Value::Tuple(tup))
            }
            _ => Err(RuntimeError::Generic {
                message: format!(
                    "cannot remove from {:?} - only arrays and tuples support remove",
                    collection
                ),
                span: op.span,
            }),
        }
    }

    /// Evaluate collection contains operator: collection$? element
    pub(crate) fn eval_collection_contains(&mut self, op: &CollectionContainsExpr) -> Result<Value> {
        let collection = self.eval_expr(&op.collection)?;
        let element = self.eval_expr(&op.element)?;

        match collection {
            Value::Array(ref arr) => {
                // Check if element exists in array using value equality
                let found = arr.iter().any(|item| self.values_equal(item, &element));
                Ok(Value::Bool(found))
            }
            Value::Tuple(ref tup) => {
                // Check if element exists in tuple using value equality
                let found = tup.iter().any(|item| self.values_equal(item, &element));
                Ok(Value::Bool(found))
            }
            Value::String(ref s) => {
                // Check if string contains character or substring
                match element {
                    Value::Char(c) => {
                        // Search for character in string
                        let found = s.contains(c);
                        Ok(Value::Bool(found))
                    }
                    Value::String(ref substring) => {
                        // Search for substring in string
                        let found = s.contains(substring.as_str());
                        Ok(Value::Bool(found))
                    }
                    _ => Err(RuntimeError::Generic {
                        message: format!(
                            "string contains only supports char or string search, got {:?}",
                            element
                        ),
                        span: op.span,
                    }),
                }
            }
            _ => Err(RuntimeError::Generic {
                message: format!(
                    "cannot search {:?} - only arrays, tuples, and strings support contains",
                    collection
                ),
                span: op.span,
            }),
        }
    }

    /// Evaluate collection update operator: collection[index]$~ value
    pub(crate) fn eval_collection_update(&mut self, op: &CollectionUpdateExpr) -> Result<Value> {
        // The target must be an IndexExpr
        let index_expr = match &*op.target {
            Expr::Index(idx) => idx,
            _ => {
                return Err(RuntimeError::Generic {
                    message: "update operator ($~) requires an indexed expression like arr[0]$~ value"
                        .to_string(),
                    span: op.span,
                });
            }
        };

        // Evaluate the collection, index, and new value
        let collection = self.eval_expr(&index_expr.array)?;
        let index_value = self.eval_expr(&index_expr.index)?;
        let new_value = self.eval_expr(&op.value)?;

        // Extract index as integer
        let index = match index_value {
            Value::Int(n) => n,
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("update index must be an integer, got {:?}", index_value),
                    span: op.span,
                })
            }
        };

        match collection {
            Value::Array(mut arr) => {
                // Check bounds
                if index < 0 || index as usize >= arr.len() {
                    return Err(RuntimeError::Generic {
                        message: format!(
                            "index out of bounds: index {} for array of length {}",
                            index,
                            arr.len()
                        ),
                        span: op.span,
                    });
                }
                // Create a new array with the value updated (immutability)
                arr[index as usize] = new_value;
                Ok(Value::Array(arr))
            }
            Value::Tuple(mut tup) => {
                // Check bounds
                if index < 0 || index as usize >= tup.len() {
                    return Err(RuntimeError::Generic {
                        message: format!(
                            "index out of bounds: index {} for tuple of length {}",
                            index,
                            tup.len()
                        ),
                        span: op.span,
                    });
                }
                // Create a new tuple with the value updated (immutability)
                tup[index as usize] = new_value;
                Ok(Value::Tuple(tup))
            }
            _ => Err(RuntimeError::Generic {
                message: format!(
                    "cannot update {:?} - only arrays and tuples support update",
                    collection
                ),
                span: op.span,
            }),
        }
    }

    /// Evaluate collection slice operator: collection$[start..end]
    pub(crate) fn eval_collection_slice(&mut self, op: &CollectionSliceExpr) -> Result<Value> {
        let collection = self.eval_expr(&op.collection)?;

        // Determine collection length for bounds checking
        let length = match &collection {
            Value::Array(arr) => arr.len(),
            Value::Tuple(tup) => tup.len(),
            Value::String(s) => s.chars().count(),
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!(
                        "cannot slice {:?} - only arrays, tuples, and strings support slice",
                        collection
                    ),
                    span: op.span,
                });
            }
        };

        // Evaluate start index (default to 0 if None)
        let start = if let Some(ref start_expr) = op.start {
            let start_value = self.eval_expr(start_expr)?;
            match start_value {
                Value::Int(n) => n,
                _ => {
                    return Err(RuntimeError::Generic {
                        message: format!("slice start must be an integer, got {:?}", start_value),
                        span: op.span,
                    })
                }
            }
        } else {
            0
        };

        // Evaluate end index (default to length if None)
        let end = if let Some(ref end_expr) = op.end {
            let end_value = self.eval_expr(end_expr)?;
            match end_value {
                Value::Int(n) => n,
                _ => {
                    return Err(RuntimeError::Generic {
                        message: format!("slice end must be an integer, got {:?}", end_value),
                        span: op.span,
                    })
                }
            }
        } else {
            length as i64
        };

        // Validate indices
        if start < 0 || end < 0 || start > length as i64 || end > length as i64 {
            return Err(RuntimeError::Generic {
                message: format!(
                    "slice indices out of bounds: [{}..{}] for collection of length {}",
                    start, end, length
                ),
                span: op.span,
            });
        }

        if start > end {
            return Err(RuntimeError::Generic {
                message: format!(
                    "slice start ({}) cannot be greater than end ({})",
                    start, end
                ),
                span: op.span,
            });
        }

        // Create the slice (immutable)
        match collection {
            Value::Array(arr) => {
                let slice = arr[(start as usize)..(end as usize)].to_vec();
                Ok(Value::Array(slice))
            }
            Value::Tuple(tup) => {
                let slice = tup[(start as usize)..(end as usize)].to_vec();
                Ok(Value::Tuple(slice))
            }
            Value::String(s) => {
                // Convert string to chars, slice, then back to string
                let chars: Vec<char> = s.chars().collect();
                let sliced_chars = &chars[(start as usize)..(end as usize)];
                let slice: String = sliced_chars.iter().collect();
                Ok(Value::String(slice))
            }
            _ => unreachable!(), // Already checked above
        }
    }

    /// Evaluate collection map: collection$> (x -> x * 2)
    pub(crate) fn eval_collection_map(&mut self, op: &zymbol_ast::CollectionMapExpr) -> Result<Value> {
        let collection = self.eval_expr(&op.collection)?;
        let lambda = self.eval_expr(&op.lambda)?;

        let func = match lambda {
            Value::Function(f) => f,
            _ => {
                return Err(RuntimeError::Generic {
                    message: "map requires lambda function".to_string(),
                    span: op.span,
                });
            }
        };

        match collection {
            Value::Array(arr) => {
                let mut result = Vec::new();

                for element in arr {
                    // Call lambda with element
                    let transformed = self.eval_lambda_call(
                        func.clone(),
                        vec![element],
                        &op.span,
                    )?;
                    result.push(transformed);
                }

                Ok(Value::Array(result))
            }
            _ => Err(RuntimeError::Generic {
                message: format!("map requires array, got {:?}", collection),
                span: op.span,
            }),
        }
    }

    /// Evaluate collection filter: collection$| (x -> x > 0)
    pub(crate) fn eval_collection_filter(&mut self, op: &zymbol_ast::CollectionFilterExpr) -> Result<Value> {
        let collection = self.eval_expr(&op.collection)?;
        let lambda = self.eval_expr(&op.lambda)?;

        let func = match lambda {
            Value::Function(f) => f,
            _ => {
                return Err(RuntimeError::Generic {
                    message: "filter requires lambda function".to_string(),
                    span: op.span,
                });
            }
        };

        match collection {
            Value::Array(arr) => {
                let mut result = Vec::new();

                for element in arr {
                    // Call lambda with element
                    let keep = self.eval_lambda_call(
                        func.clone(),
                        vec![element.clone()],
                        &op.span,
                    )?;

                    // Check if result is boolean
                    match keep {
                        Value::Bool(true) => result.push(element),
                        Value::Bool(false) => {}
                        _ => {
                            return Err(RuntimeError::Generic {
                                message: format!("filter lambda must return boolean, got {:?}", keep),
                                span: op.span,
                            });
                        }
                    }
                }

                Ok(Value::Array(result))
            }
            _ => Err(RuntimeError::Generic {
                message: format!("filter requires array, got {:?}", collection),
                span: op.span,
            }),
        }
    }

    /// Evaluate collection reduce: collection$< (0, (acc, x) -> acc + x)
    pub(crate) fn eval_collection_reduce(&mut self, op: &zymbol_ast::CollectionReduceExpr) -> Result<Value> {
        let collection = self.eval_expr(&op.collection)?;
        let initial = self.eval_expr(&op.initial)?;
        let lambda = self.eval_expr(&op.lambda)?;

        let func = match lambda {
            Value::Function(f) => f,
            _ => {
                return Err(RuntimeError::Generic {
                    message: "reduce requires lambda function".to_string(),
                    span: op.span,
                });
            }
        };

        // Validate lambda has 2 parameters
        if func.params.len() != 2 {
            return Err(RuntimeError::Generic {
                message: format!(
                    "reduce lambda requires 2 parameters (accumulator, element), got {}",
                    func.params.len()
                ),
                span: op.span,
            });
        }

        match collection {
            Value::Array(arr) => {
                let mut accumulator = initial;

                for element in arr {
                    // Call lambda with (accumulator, element)
                    accumulator = self.eval_lambda_call(
                        func.clone(),
                        vec![accumulator, element],
                        &op.span,
                    )?;
                }

                Ok(accumulator)
            }
            _ => Err(RuntimeError::Generic {
                message: format!("reduce requires array, got {:?}", collection),
                span: op.span,
            }),
        }
    }
}
