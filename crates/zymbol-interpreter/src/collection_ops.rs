//! Collection operation evaluation for Zymbol-Lang
//!
//! Handles runtime execution of all collection operators:
//! - $# (length/size)
//! - $+ (append element by value)
//! - $+[i] (insert element at position)
//! - $- (remove first occurrence by value)
//! - $-- (remove all occurrences by value)
//! - $-[i] (remove element at index)
//! - $-[i..j] (remove range of elements)
//! - $? (contains/search by value)
//! - $?? (find all indices of value)
//! - $~ (update element at index)
//! - $[ (slice with range)
//! - $> (map - transform collection)
//! - $| (filter - select elements)
//! - $< (reduce - accumulate)

use zymbol_ast::{
    CollectionAppendExpr, CollectionContainsExpr, CollectionFindAllExpr,
    CollectionInsertExpr, CollectionLengthExpr,
    CollectionRemoveAllExpr, CollectionRemoveAtExpr, CollectionRemoveRangeExpr,
    CollectionRemoveValueExpr, CollectionSliceExpr, CollectionUpdateExpr, Expr,
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
                arr.push(element);
                Ok(Value::Array(arr))
            }
            Value::Tuple(mut tup) => {
                tup.push(element);
                Ok(Value::Tuple(tup))
            }
            Value::NamedTuple(_) => Err(RuntimeError::Generic {
                message: "$+ is not supported on named tuples — no field name available".to_string(),
                span: op.span,
            }),
            Value::String(s) => {
                let result = match element {
                    Value::Char(c) => { let mut out = s; out.push(c); out }
                    Value::String(ref suffix) => { let mut out = s; out.push_str(suffix); out }
                    _ => return Err(RuntimeError::Generic {
                        message: format!("$+ on string requires char or string element, got {:?}", element),
                        span: op.span,
                    }),
                };
                Ok(Value::String(result))
            }
            _ => Err(RuntimeError::Generic {
                message: format!(
                    "cannot append to {:?} - only arrays, tuples, and strings support $+",
                    collection
                ),
                span: op.span,
            }),
        }
    }

    /// Evaluate collection remove at operator: collection$-[index]
    pub(crate) fn eval_collection_remove(&mut self, op: &CollectionRemoveAtExpr) -> Result<Value> {
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
                let len = arr.len();
                let i = if index == 0 {
                    return Err(RuntimeError::Generic {
                        message: "index 0 is invalid — Zymbol uses 1-based indexing (use 1 for the first element, -1 for the last)".to_string(),
                        span: op.span,
                    });
                } else if index < 0 {
                    len as i64 + index
                } else {
                    index - 1
                };
                if i < 0 || i as usize >= len {
                    return Err(RuntimeError::Generic {
                        message: format!("index out of bounds: index {} for array of length {}", index, len),
                        span: op.span,
                    });
                }
                arr.remove(i as usize);
                Ok(Value::Array(arr))
            }
            Value::Tuple(mut tup) => {
                let len = tup.len();
                let i = if index == 0 {
                    return Err(RuntimeError::Generic {
                        message: "index 0 is invalid — Zymbol uses 1-based indexing (use 1 for the first element, -1 for the last)".to_string(),
                        span: op.span,
                    });
                } else if index < 0 {
                    len as i64 + index
                } else {
                    index - 1
                };
                if i < 0 || i as usize >= len {
                    return Err(RuntimeError::Generic {
                        message: format!("index out of bounds: index {} for tuple of length {}", index, len),
                        span: op.span,
                    });
                }
                tup.remove(i as usize);
                Ok(Value::Tuple(tup))
            }
            Value::NamedTuple(mut fields) => {
                let len = fields.len();
                let i = if index == 0 {
                    return Err(RuntimeError::Generic {
                        message: "index 0 is invalid — Zymbol uses 1-based indexing (use 1 for the first element, -1 for the last)".to_string(),
                        span: op.span,
                    });
                } else if index < 0 {
                    len as i64 + index
                } else {
                    index - 1
                };
                if i < 0 || i as usize >= len {
                    return Err(RuntimeError::Generic {
                        message: format!("index out of bounds: index {} for named tuple of length {}", index, len),
                        span: op.span,
                    });
                }
                fields.remove(i as usize);
                Ok(Value::NamedTuple(fields))
            }
            Value::String(s) => {
                let mut chars: Vec<char> = s.chars().collect();
                let len = chars.len();
                let i = if index == 0 {
                    return Err(RuntimeError::Generic {
                        message: "index 0 is invalid — Zymbol uses 1-based indexing (use 1 for the first element, -1 for the last)".to_string(),
                        span: op.span,
                    });
                } else if index < 0 {
                    len as i64 + index
                } else {
                    index - 1
                };
                if i < 0 || i as usize >= len {
                    return Err(RuntimeError::Generic {
                        message: format!("index out of bounds: index {} for string of length {}", index, len),
                        span: op.span,
                    });
                }
                chars.remove(i as usize);
                Ok(Value::String(chars.iter().collect()))
            }
            _ => Err(RuntimeError::Generic {
                message: format!(
                    "cannot remove from {:?} - only arrays, tuples, and strings support $-[i]",
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
                let len = arr.len();
                let i = if index == 0 {
                    return Err(RuntimeError::Generic {
                        message: "index 0 is invalid — Zymbol uses 1-based indexing".to_string(),
                        span: op.span,
                    });
                } else if index < 0 {
                    let i = len as i64 + index;
                    if i < 0 || i as usize >= len {
                        return Err(RuntimeError::Generic {
                            message: format!("index out of bounds: index {} for array of length {}", index, len),
                            span: op.span,
                        });
                    }
                    i as usize
                } else {
                    let i = (index - 1) as usize;
                    if i >= len {
                        return Err(RuntimeError::Generic {
                            message: format!("index out of bounds: index {} for array of length {}", index, len),
                            span: op.span,
                        });
                    }
                    i
                };
                // Create a new array with the value updated (immutability)
                arr[i] = new_value;
                Ok(Value::Array(arr))
            }
            Value::Tuple(mut tup) => {
                let len = tup.len();
                let i = if index == 0 {
                    return Err(RuntimeError::Generic {
                        message: "index 0 is invalid — Zymbol uses 1-based indexing".to_string(),
                        span: op.span,
                    });
                } else if index < 0 {
                    let i = len as i64 + index;
                    if i < 0 || i as usize >= len {
                        return Err(RuntimeError::Generic {
                            message: format!("index out of bounds: index {} for tuple of length {}", index, len),
                            span: op.span,
                        });
                    }
                    i as usize
                } else {
                    let i = (index - 1) as usize;
                    if i >= len {
                        return Err(RuntimeError::Generic {
                            message: format!("index out of bounds: index {} for tuple of length {}", index, len),
                            span: op.span,
                        });
                    }
                    i
                };
                // Create a new tuple with the value updated (immutability)
                tup[i] = new_value;
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
            Value::NamedTuple(fields) => fields.len(),
            Value::String(s) => s.chars().count(),
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!(
                        "cannot slice {:?} - only arrays, tuples, named tuples, and strings support slice",
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
        let raw_end = if let Some(ref end_expr) = op.end {
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

        // Normalize indices (1-based: positive i maps to internal i-1; 0 = default start)
        let start = if start == 0 {
            // None/default: maps to 0-based start (first element)
            0
        } else if start < 0 {
            length as i64 + start
        } else {
            start - 1  // 1-based to 0-based
        };
        let raw_end = if !op.count_based {
            if raw_end < 0 {
                length as i64 + raw_end + 1  // 1-based inclusive negative → 0-based exclusive
            } else {
                raw_end  // 1-based inclusive positive = 0-based exclusive (no change)
            }
        } else {
            raw_end  // count_based: count is unchanged, start was already normalized
        };

        // count_based: end field holds count → actual_end = start + count
        let end = if op.count_based { start + raw_end } else { raw_end };

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
            Value::NamedTuple(fields) => {
                let slice = fields[(start as usize)..(end as usize)].to_vec();
                Ok(Value::NamedTuple(slice))
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

    /// Evaluate collection sort: collection$^+ or collection$^-
    /// Natural order for numbers/strings; custom comparator (a, b) -> Bool for named tuples.
    pub(crate) fn eval_collection_sort(&mut self, op: &zymbol_ast::CollectionSortExpr) -> Result<Value> {
        let collection = self.eval_expr(&op.collection)?;

        match collection {
            Value::Array(arr) => {
                let mut items = arr.clone();

                if let Some(ref cmp_expr) = op.comparator {
                    // Custom comparator: (a, b) -> Bool
                    let lambda = self.eval_expr(cmp_expr)?;
                    let func = match lambda {
                        Value::Function(f) => f,
                        _ => return Err(RuntimeError::Generic {
                            message: "sort comparator must be a lambda (a, b) -> Bool".to_string(),
                            span: op.span,
                        }),
                    };
                    // Bubble sort — stable, correct for small arrays; avoids unsafe sort_by
                    let n = items.len();
                    for i in 0..n {
                        for j in 0..n.saturating_sub(i + 1) {
                            let keep = self.eval_lambda_call(
                                func.clone(),
                                vec![items[j].clone(), items[j + 1].clone()],
                                &op.span,
                            )?;
                            let a_before_b = matches!(keep, Value::Bool(true));
                            if !a_before_b {
                                items.swap(j, j + 1);
                            }
                        }
                    }
                } else {
                    // Natural order
                    items.sort_by(|a, b| {
                        natural_cmp(a, b).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    if !op.ascending {
                        items.reverse();
                    }
                }

                Ok(Value::Array(items))
            }
            _ => Err(RuntimeError::Generic {
                message: format!("sort requires an array, got {:?}", collection),
                span: op.span,
            }),
        }
    }

    /// Evaluate collection insert operator: collection$+[index] element
    pub(crate) fn eval_collection_insert(&mut self, op: &CollectionInsertExpr) -> Result<Value> {
        let collection = self.eval_expr(&op.collection)?;
        let index_val = self.eval_expr(&op.index)?;
        let element = self.eval_expr(&op.element)?;

        let index = match index_val {
            Value::Int(n) => n,
            _ => return Err(RuntimeError::Generic {
                message: format!("$+[i] index must be an integer, got {:?}", index_val),
                span: op.span,
            }),
        };
        if index <= 0 {
            return Err(RuntimeError::Generic {
                message: format!("$+[i] index must be positive (1-based, use 1 to insert at the beginning), got {}", index),
                span: op.span,
            });
        }
        let i = (index - 1) as usize;

        match collection {
            Value::Array(mut arr) => {
                if i > arr.len() {
                    return Err(RuntimeError::Generic {
                        message: format!("$+[{}] index out of bounds for array of length {}", i, arr.len()),
                        span: op.span,
                    });
                }
                arr.insert(i, element);
                Ok(Value::Array(arr))
            }
            Value::Tuple(mut tup) => {
                if i > tup.len() {
                    return Err(RuntimeError::Generic {
                        message: format!("$+[{}] index out of bounds for tuple of length {}", i, tup.len()),
                        span: op.span,
                    });
                }
                tup.insert(i, element);
                Ok(Value::Tuple(tup))
            }
            Value::NamedTuple(_) => Err(RuntimeError::Generic {
                message: "$+[i] is not supported on named tuples — no field name available".to_string(),
                span: op.span,
            }),
            Value::String(s) => {
                let mut chars: Vec<char> = s.chars().collect();
                if i > chars.len() {
                    return Err(RuntimeError::Generic {
                        message: format!("$+[{}] index out of bounds for string of length {}", i, chars.len()),
                        span: op.span,
                    });
                }
                match element {
                    Value::Char(c) => { chars.insert(i, c); Ok(Value::String(chars.iter().collect())) }
                    Value::String(ref ins) => {
                        let insert_chars: Vec<char> = ins.chars().collect();
                        for (j, c) in insert_chars.iter().enumerate() {
                            chars.insert(i + j, *c);
                        }
                        Ok(Value::String(chars.iter().collect()))
                    }
                    _ => Err(RuntimeError::Generic {
                        message: format!("$+[i] on string requires char or string element, got {:?}", element),
                        span: op.span,
                    }),
                }
            }
            _ => Err(RuntimeError::Generic {
                message: format!("$+[i] requires an array, tuple, or string, got {:?}", collection),
                span: op.span,
            }),
        }
    }

    /// Evaluate collection remove value operator: collection$- value
    /// Removes the first occurrence of value. Silent no-op if not found.
    pub(crate) fn eval_collection_remove_value(&mut self, op: &CollectionRemoveValueExpr) -> Result<Value> {
        let collection = self.eval_expr(&op.collection)?;
        let value = self.eval_expr(&op.value)?;

        match collection {
            Value::Array(mut arr) => {
                if let Some(pos) = arr.iter().position(|item| self.values_equal(item, &value)) {
                    arr.remove(pos);
                }
                Ok(Value::Array(arr))
            }
            Value::Tuple(mut tup) => {
                if let Some(pos) = tup.iter().position(|item| self.values_equal(item, &value)) {
                    tup.remove(pos);
                }
                Ok(Value::Tuple(tup))
            }
            Value::NamedTuple(mut fields) => {
                if let Some(pos) = fields.iter().position(|(_, v)| self.values_equal(v, &value)) {
                    fields.remove(pos);
                }
                Ok(Value::NamedTuple(fields))
            }
            Value::String(s) => {
                let chars: Vec<char> = s.chars().collect();
                let result = match value {
                    Value::Char(c) => {
                        if let Some(pos) = chars.iter().position(|ch| *ch == c) {
                            let mut out = chars.clone();
                            out.remove(pos);
                            out.iter().collect()
                        } else {
                            s.clone()
                        }
                    }
                    Value::String(ref pattern) => {
                        let pattern_chars: Vec<char> = pattern.chars().collect();
                        if pattern_chars.is_empty() { return Ok(Value::String(s)); }
                        for i in 0..=(chars.len().saturating_sub(pattern_chars.len())) {
                            if chars[i..i + pattern_chars.len()] == pattern_chars[..] {
                                let mut out = chars.clone();
                                out.drain(i..i + pattern_chars.len());
                                return Ok(Value::String(out.iter().collect()));
                            }
                        }
                        s.clone()
                    }
                    _ => return Err(RuntimeError::Generic {
                        message: format!("$- on string requires char or string value, got {:?}", value),
                        span: op.span,
                    }),
                };
                Ok(Value::String(result))
            }
            _ => Err(RuntimeError::Generic {
                message: format!("$- requires an array, tuple, or string, got {:?}", collection),
                span: op.span,
            }),
        }
    }

    /// Evaluate collection remove all operator: collection$-- value
    /// Removes all occurrences of value. Silent no-op if not found.
    pub(crate) fn eval_collection_remove_all(&mut self, op: &CollectionRemoveAllExpr) -> Result<Value> {
        let collection = self.eval_expr(&op.collection)?;
        let value = self.eval_expr(&op.value)?;

        match collection {
            Value::Array(arr) => {
                let result: Vec<Value> = arr.into_iter()
                    .filter(|item| !self.values_equal(item, &value))
                    .collect();
                Ok(Value::Array(result))
            }
            Value::Tuple(tup) => {
                let result: Vec<Value> = tup.into_iter()
                    .filter(|item| !self.values_equal(item, &value))
                    .collect();
                Ok(Value::Tuple(result))
            }
            Value::NamedTuple(fields) => {
                let result: Vec<(String, Value)> = fields.into_iter()
                    .filter(|(_, v)| !self.values_equal(v, &value))
                    .collect();
                Ok(Value::NamedTuple(result))
            }
            Value::String(s) => {
                let result = match value {
                    Value::Char(c) => s.chars().filter(|ch| *ch != c).collect(),
                    Value::String(ref pattern) => {
                        if pattern.is_empty() { return Ok(Value::String(s)); }
                        s.replace(pattern.as_str(), "")
                    }
                    _ => return Err(RuntimeError::Generic {
                        message: format!("$-- on string requires char or string value, got {:?}", value),
                        span: op.span,
                    }),
                };
                Ok(Value::String(result))
            }
            _ => Err(RuntimeError::Generic {
                message: format!("$-- requires an array, tuple, or string, got {:?}", collection),
                span: op.span,
            }),
        }
    }

    /// Evaluate collection remove range operator: collection$-[start..end]
    pub(crate) fn eval_collection_remove_range(&mut self, op: &CollectionRemoveRangeExpr) -> Result<Value> {
        let collection = self.eval_expr(&op.collection)?;

        let length = match &collection {
            Value::Array(a) => a.len(),
            Value::Tuple(t) => t.len(),
            Value::NamedTuple(f) => f.len(),
            Value::String(s) => s.chars().count(),
            _ => return Err(RuntimeError::Generic {
                message: format!("$-[..] requires an array, tuple, or string, got {:?}", collection),
                span: op.span,
            }),
        };

        let start = if let Some(ref s_expr) = op.start {
            match self.eval_expr(s_expr)? {
                Value::Int(n) => {
                    if n <= 0 { return Err(RuntimeError::Generic {
                        message: format!("$-[start..] start must be positive (1-based), got {}", n),
                        span: op.span,
                    }); }
                    (n - 1) as usize  // normalize 1-based to 0-based
                }
                other => return Err(RuntimeError::Generic {
                    message: format!("$-[..] start must be an integer, got {:?}", other),
                    span: op.span,
                }),
            }
        } else { 0 };

        let raw_end = if let Some(ref e_expr) = op.end {
            match self.eval_expr(e_expr)? {
                Value::Int(n) => {
                    if !op.count_based && n <= 0 {
                        // For range-based $-[start..end], end must be positive (1-based)
                        return Err(RuntimeError::Generic {
                            message: format!("$-[..end] end must be positive (1-based), got {}", n),
                            span: op.span,
                        });
                    }
                    if n < 0 {
                        return Err(RuntimeError::Generic {
                            message: format!("$-[..] count must be non-negative, got {}", n),
                            span: op.span,
                        });
                    }
                    n as usize  // range: 1-based inclusive = 0-based exclusive; count: raw count
                }
                other => return Err(RuntimeError::Generic {
                    message: format!("$-[..] end must be an integer, got {:?}", other),
                    span: op.span,
                }),
            }
        } else { length };

        // count_based: end field holds count → actual_end = start + count
        let end = if op.count_based { start + raw_end } else { raw_end };

        if start > end {
            return Err(RuntimeError::Generic {
                message: format!("$-[start..end]: start ({}) cannot be greater than end ({})", start, end),
                span: op.span,
            });
        }
        if start > length || end > length {
            return Err(RuntimeError::Generic {
                message: format!("$-[{}..{}] out of bounds for collection of length {}", start, end, length),
                span: op.span,
            });
        }
        // i == j → no-op
        if start == end {
            return Ok(collection);
        }

        match collection {
            Value::Array(mut arr) => { arr.drain(start..end); Ok(Value::Array(arr)) }
            Value::Tuple(mut tup) => { tup.drain(start..end); Ok(Value::Tuple(tup)) }
            Value::NamedTuple(mut fields) => { fields.drain(start..end); Ok(Value::NamedTuple(fields)) }
            Value::String(s) => {
                let mut chars: Vec<char> = s.chars().collect();
                chars.drain(start..end);
                Ok(Value::String(chars.iter().collect()))
            }
            _ => unreachable!(),
        }
    }

    /// Evaluate collection find all operator: collection$?? value
    /// Returns an array of indices where value is found
    /// Supports: arrays (element equality), tuples, strings (char/substring search)
    pub(crate) fn eval_collection_find_all(&mut self, op: &CollectionFindAllExpr) -> Result<Value> {
        let collection = self.eval_expr(&op.collection)?;
        let value = self.eval_expr(&op.value)?;

        match collection {
            Value::Array(ref arr) => {
                let indices: Vec<Value> = arr.iter()
                    .enumerate()
                    .filter(|(_, item)| self.values_equal(item, &value))
                    .map(|(i, _)| Value::Int((i + 1) as i64))
                    .collect();
                Ok(Value::Array(indices))
            }
            Value::Tuple(ref tup) => {
                let indices: Vec<Value> = tup.iter()
                    .enumerate()
                    .filter(|(_, item)| self.values_equal(item, &value))
                    .map(|(i, _)| Value::Int((i + 1) as i64))
                    .collect();
                Ok(Value::Array(indices))
            }
            Value::String(ref s) => {
                let string_chars: Vec<char> = s.chars().collect();
                let positions = match value {
                    Value::String(ref pattern) => {
                        let pattern_chars: Vec<char> = pattern.chars().collect();
                        if pattern_chars.is_empty() {
                            return Ok(Value::Array(vec![]));
                        }
                        let mut positions = Vec::new();
                        for i in 0..=(string_chars.len().saturating_sub(pattern_chars.len())) {
                            if string_chars[i..i + pattern_chars.len()] == pattern_chars[..] {
                                positions.push(Value::Int((i + 1) as i64));
                            }
                        }
                        positions
                    }
                    Value::Char(ch) => {
                        string_chars.iter()
                            .enumerate()
                            .filter(|(_, c)| **c == ch)
                            .map(|(i, _)| Value::Int((i + 1) as i64))
                            .collect()
                    }
                    _ => {
                        return Err(RuntimeError::Generic {
                            message: format!(
                                "$?? on string requires char or string value, got {:?}",
                                value
                            ),
                            span: op.span,
                        })
                    }
                };
                Ok(Value::Array(positions))
            }
            _ => Err(RuntimeError::Generic {
                message: format!(
                    "$?? requires an array, tuple, or string, got {:?}",
                    collection
                ),
                span: op.span,
            }),
        }
    }
}

/// Natural comparison for sort: numbers, strings, booleans.
fn natural_cmp(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Int(x), Value::Int(y))       => Some(x.cmp(y)),
        (Value::Float(x), Value::Float(y))   => x.partial_cmp(y),
        (Value::Int(x), Value::Float(y))     => (*x as f64).partial_cmp(y),
        (Value::Float(x), Value::Int(y))     => x.partial_cmp(&(*y as f64)),
        (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
        (Value::Bool(x), Value::Bool(y))     => Some(x.cmp(y)),
        _ => None,
    }
}

