//! Evaluation of multi-dimensional indexing and restructuring expressions.
//!
//! Implements:
//! - `eval_deep_index`       — arr[i>j>k]  → single Value
//! - `eval_flat_extract`     — arr[p;q] or arr[[i>j]]  → Value::Array (flat)
//! - `eval_structured_extract` — arr[[g];[g]]  → Value::Array of Value::Array

use zymbol_ast::{DeepIndexExpr, ExtractGroup, FlatExtractExpr, NavPath, NavStep, StructuredExtractExpr};
use crate::{Interpreter, Result, RuntimeError, Value};
use std::io::Write;

impl<W: Write> Interpreter<W> {
    // ── Deep scalar access ────────────────────────────────────────────────────

    /// Evaluate `arr[i>j>k]` — descend into a nested collection step by step.
    pub(crate) fn eval_deep_index(&mut self, di: &DeepIndexExpr) -> Result<Value> {
        let mut current = self.eval_expr(&di.array)?;
        for step in &di.path.steps {
            // A step is an ordinary expression, and its VALUE says how to
            // address: Int → position, String → dictionary key. Same rule as
            // `d[clave]`, one level down, which is why `config[k1>k2]` works
            // with `k1 = "servidor"`.
            let step_val = self.eval_expr(&step.index)?;
            if let Value::String(key) = &step_val {
                current = descend_key(current, key, di.span)?;
                continue;
            }
            let idx = match step_val {
                Value::Int(n) => n,
                other => return Err(RuntimeError::Generic {
                    message: format!(
                        "a navigation step is a position (Int) or a dictionary key (String), got {:?}",
                        other
                    ),
                    span: step.index.span(),
                }),
            };
            current = descend(current, idx, step.index.span(), di.span)?;
        }
        Ok(current)
    }

    // ── Flat extraction ───────────────────────────────────────────────────────

    /// Evaluate `arr[p;q;r]` or `arr[[i>j]]` — collect values into a flat array.
    pub(crate) fn eval_flat_extract(&mut self, fe: &FlatExtractExpr) -> Result<Value> {
        let base = self.eval_expr(&fe.array)?;
        let mut result = Vec::new();

        for path in &fe.paths {
            let values = self.walk_nav_path(base.clone(), path, fe.span)?;
            result.extend(values);
        }

        Ok(Value::Array(result))
    }

    // ── Structured extraction ─────────────────────────────────────────────────

    /// Evaluate `arr[[g];[g]]` — collect each group into a sub-array.
    pub(crate) fn eval_structured_extract(&mut self, se: &StructuredExtractExpr) -> Result<Value> {
        let base = self.eval_expr(&se.array)?;
        let mut groups_out = Vec::new();

        for group in &se.groups {
            let sub = self.eval_extract_group(base.clone(), group, se.span)?;
            groups_out.push(Value::Array(sub));
        }

        Ok(Value::Array(groups_out))
    }

    fn eval_extract_group(
        &mut self,
        base: Value,
        group: &ExtractGroup,
        span: zymbol_span::Span,
    ) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        for path in &group.paths {
            let values = self.walk_nav_path(base.clone(), path, span)?;
            out.extend(values);
        }
        Ok(out)
    }

    // ── Navigation helpers ────────────────────────────────────────────────────

    /// Walk a `NavPath` through `base`, returning a `Vec<Value>`.
    ///
    /// If a step has a `range_end`, that dimension is expanded:
    /// - **last step** with range → collects values along the final axis
    /// - **intermediate step** with range → fan-out: remaining steps are applied to
    ///   each element in the expanded dimension, results are flattened
    fn walk_nav_path(
        &mut self,
        base: Value,
        path: &NavPath,
        span: zymbol_span::Span,
    ) -> Result<Vec<Value>> {
        self.walk_steps(base, &path.steps, span)
    }

    fn walk_steps(
        &mut self,
        current: Value,
        steps: &[NavStep],
        span: zymbol_span::Span,
    ) -> Result<Vec<Value>> {
        if steps.is_empty() {
            return Ok(vec![current]);
        }

        let step = &steps[0];
        let rest = &steps[1..];

        if let Some(range_end_expr) = &step.range_end {
            // Ranged step — expand this dimension
            let start_idx = self.eval_nav_atom(&step.index, step)?;
            let end_idx = self.eval_nav_atom(range_end_expr, step)?;

            let (start, end) = match (start_idx, end_idx) {
                (i, j) if i < 0 || j < 0 => {
                    return Err(RuntimeError::Generic {
                        message: "range indices in nav path must be positive integers".to_string(),
                        span,
                    })
                }
                (i, j) => (i, j),
            };

            if start < 1 || end < start {
                return Err(RuntimeError::Generic {
                    message: format!(
                        "invalid nav range {}..{} — indices are 1-based and start must be ≤ end",
                        start, end
                    ),
                    span,
                });
            }

            let mut collected = Vec::new();
            for i in start..=end {
                let elem = descend(current.clone(), i, step.index.span(), span)?;
                if rest.is_empty() {
                    collected.push(elem);
                } else {
                    let sub = self.walk_steps(elem, rest, span)?;
                    collected.extend(sub);
                }
            }
            Ok(collected)
        } else {
            // Plain step — descend. A step is an ORDINARY EXPRESSION, and its
            // value says how to address: an Int is a position, a String is a
            // dictionary key. That is what makes `config[k1>k2]` work with
            // `k1 = "servidor"` — the same rule as `d[clave]`, one level down.
            //
            // It has to be the value and not the spelling: a bare identifier
            // inside `[…]` is a VARIABLE, which is exactly what makes a computed
            // key possible. If `config[servidor>…]` meant the key named
            // `servidor`, then `d[clave]` would mean the key named `clave` and
            // computed keys could not exist.
            let step_val = self.eval_expr(&step.index)?;
            if let Value::String(key) = &step_val {
                let next = descend_key(current, key, span)?;
                return self.walk_steps(next, rest, span);
            }
            let idx = match step_val {
                Value::Int(n) => n,
                other => return Err(RuntimeError::Generic {
                    message: format!(
                        "a navigation step is a position (Int) or a dictionary key (String), got {:?}",
                        other
                    ),
                    span: step.index.span(),
                }),
            };
            let next = descend(current, idx, step.index.span(), span)?;
            self.walk_steps(next, rest, span)
        }
    }

    /// Evaluate a `nav_atom` expression and extract an `i64` index.
    fn eval_nav_atom(
        &mut self,
        expr: &zymbol_ast::Expr,
        _step: &NavStep,
    ) -> Result<i64> {
        let val = self.eval_expr(expr)?;
        match val {
            Value::Int(n) => Ok(n),
            other => Err(RuntimeError::Generic {
                message: format!(
                    "navigation index must be an integer, got {:?}",
                    other
                ),
                span: expr.span(),
            }),
        }
    }
}

/// Descend into a dictionary by KEY — the String half of a navigation step.
fn descend_key(collection: Value, key: &str, op_span: zymbol_span::Span) -> Result<Value> {
    match collection {
        Value::NamedTuple(fields) => match fields.iter().find(|(k, _)| k == key) {
            Some((_, v)) => Ok(v.clone()),
            None => {
                let available: Vec<String> = fields.iter().map(|(k, _)| k.clone()).collect();
                Err(RuntimeError::Generic {
                    message: crate::variables::missing_key_msg(key, &available),
                    span: op_span,
                })
            }
        },
        other => Err(RuntimeError::Generic {
            message: format!(
                "a String navigation step addresses a dictionary key, and this is {}",
                other.type_name()
            ),
            span: op_span,
        }),
    }
}

// ── Shared descent helper (not a method — no `self` needed) ─────────────────

/// Descend into `collection` by 1-based `index`, returning the element.
///
/// Supports `Value::Array`, `Value::Tuple`, `Value::NamedTuple`, and `Value::String`.
fn descend(
    collection: Value,
    index: i64,
    index_span: zymbol_span::Span,
    op_span: zymbol_span::Span,
) -> Result<Value> {
    if index == 0 {
        return Err(RuntimeError::Generic {
            message: "index 0 is invalid — Zymbol uses 1-based indexing (use 1 for the first element, -1 for the last)".to_string(),
            span: index_span,
        });
    }

    match collection {
        Value::Array(arr) => {
            let len = arr.len();
            let i = resolve_index(index, len, op_span, "array")?;
            Ok(arr[i].clone())
        }
        Value::Tuple(elems) => {
            let len = elems.len();
            let i = resolve_index(index, len, op_span, "tuple")?;
            Ok(elems[i].clone())
        }
        Value::NamedTuple(fields) => {
            let len = fields.len();
            let i = resolve_index(index, len, op_span, "named tuple")?;
            Ok(fields[i].1.clone())
        }
        Value::String(s) => {
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len();
            let i = resolve_index(index, len, op_span, "string")?;
            Ok(Value::String(chars[i].to_string()))
        }
        other => Err(RuntimeError::Generic {
            message: format!(
                "cannot index into {:?} — expected array, tuple, or string",
                other
            ),
            span: op_span,
        }),
    }
}

/// Convert a 1-based (or negative) index to a 0-based usize, checking bounds.
///
/// `container` names the thing that was too short, because the read path
/// (`expr_eval`) names it — "array index out of bounds … for array of length 2"
/// — and this write path said "collection" for all four. Same program, two
/// texts, decided by whether the index was being read or written; `zyq
/// consensus` never saw it because no corpus file writes past the end.
fn resolve_index(
    index: i64,
    len: usize,
    span: zymbol_span::Span,
    container: &str,
) -> Result<usize> {
    let i = if index < 0 {
        len as i64 + index
    } else {
        index - 1
    };
    if i < 0 || i as usize >= len {
        return Err(RuntimeError::Generic {
            message: format!(
                "{} index out of bounds: index {} for {} of length {}",
                container, index, container, len
            ),
            span,
        });
    }
    Ok(i as usize)
}
