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
                    Value::Array(elements) => Ok(crate::own_elements(elements)),
                    Value::String(s) => {
                        // Convert string to array of chars for iteration
                        Ok(s.chars().map(Value::Char).collect())
                    }
                    // A dictionary yields its KEYS, in insertion order — `for k
                    // in d` as Python spells it. With `d[k]` available the key
                    // is enough to reach the value, so no destructuring pattern
                    // has to be introduced into `@` (decision 8).
                    //
                    // A dictionary whose keys cannot be enumerated only serves
                    // when the program already knows what it holds, which is the
                    // definition of a record — the thing decision 7 stopped it
                    // from being.
                    Value::NamedTuple(fields) => Ok(fields
                        .iter()
                        .map(|(k, _)| Value::String(k.clone()))
                        .collect()),
                    // A positional tuple is walked too (decision 21): the type
                    // system is dynamic and `#?` validates each element, so
                    // walking a mixed collection is not walking blind.
                    Value::Tuple(items) => Ok(items.to_vec()),
                    _ => Err(RuntimeError::Generic {
                        message: format!(
                            "can only iterate over ranges, arrays, strings, tuples and dictionaries, got {:?}",
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
            message: format!(
                "'{}' is undefined — did you mean '{}°' (hot definition)?",
                ident.name, ident.name
            ),
            span: ident.span,
        })
    }

    /// Evaluate member access expression: object.field
    /// Handles both module constants (module.CONSTANT) and named tuple fields (tuple.field)
    pub(crate) fn eval_member_access(&mut self, member: &zymbol_ast::MemberAccessExpr) -> Result<Value> {
        // Check if the object is a module alias (for module.CONSTANT access)
        if let Expr::Identifier(id) = member.object.unwrap_group() {
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

        // HLZ-012, the same copy as `eval_index`: reading ONE key must not copy
        // the dictionary. `eval_expr` on a name is `get_variable(..).clone()`,
        // so `d.clave` cloned every entry to hand back one of them.
        //
        // Only the dictionary takes the short path. Anything else falls through
        // and is cloned as before — it is on its way to an error, and the error
        // is worth more than the copy it costs.
        if let Expr::Identifier(id) = member.object.unwrap_group() {
            self.check_variable_alive(&id.name, &id.span)?;
            if let Some(Value::NamedTuple(fields)) = self.get_variable(&id.name) {
                if let Some((_, v)) = fields.iter().find(|(k, _)| k == &member.field) {
                    return Ok(v.clone());
                }
                let available: Vec<String> = fields.iter().map(|(k, _)| k.clone()).collect();
                return Err(RuntimeError::Generic {
                    message: crate::variables::missing_key_msg(&member.field, &available),
                    span: member.span,
                });
            }
        }

        // Not a module access, evaluate as regular member access (for named tuples)
        let object = self.eval_expr(&member.object)?;
        let got = crate::base_type_symbol(&object);

        match object {
            Value::NamedTuple(fields) => {
                // Search for field by name
                for (field_name, field_value) in fields.iter() {
                    if field_name == &member.field {
                        return Ok(field_value.clone());
                    }
                }
                // Field not found
                let available_fields: Vec<String> = fields.iter()
                    .map(|(name, _)| name.clone())
                    .collect();
                Err(RuntimeError::Generic {
                    message: crate::variables::missing_key_msg(&member.field, &available_fields),
                    span: member.span,
                })
            }
            Value::Tuple(_) => {
                Err(RuntimeError::Generic {
                    message: format!(
                        "a positional tuple is addressed by position, not by name: '{}'\nhelp: use t[1] — names live in a dictionary, #(key: value)",
                        member.field
                    ),
                    span: member.span,
                })
            }
            _ => {
                Err(RuntimeError::Generic {
                    message: format!(
                        "the dot reaches a dictionary key, and this is {}\nhelp: use d.{} on a #(…) — for a position, use x[1]",
                        got, member.field
                    ),
                    span: member.span,
                })
            }
        }
    }

    /// The elements a `@ (a, b):x` loop hands to its pattern.
    ///
    /// Identical to `eval_iterable` except on a DICTIONARY, where it yields
    /// `(clave, valor)` pairs instead of bare keys. `@ k:d` keeps yielding keys
    /// (decision 8) — the pattern form is what asks for both, and the pair is
    /// the language's own answer to "several values that travel together"
    /// (decision 24).
    pub(crate) fn eval_iterable_pairs(&mut self, expr: &zymbol_ast::Expr) -> Result<Vec<Value>> {
        if let Ok(v) = self.eval_expr(expr) {
            if let Value::NamedTuple(fields) = v {
                return Ok(fields
                    .iter()
                    .map(|(k, val)| Value::tuple(vec![Value::String(k.clone()), val.clone()]))
                    .collect());
            }
        }
        self.eval_iterable(expr)
    }

    /// Evaluate array/tuple indexing
    /// Supports arrays, tuples (both positional and named), and strings
    pub(crate) fn eval_index(&mut self, idx: &IndexExpr) -> Result<Value> {
        // HLZ-012: reading ONE element must not copy the collection.
        //
        // The general path below evaluates `idx.array` with `eval_expr`, and for
        // an identifier that is `get_variable(..).clone()` — so `a[7]` cloned
        // every element of `a` to hand back one of them, and the cost of a read
        // grew with the size of the state the program carried. A 19x19 board is
        // 361 cells, and `局面[点]` sits on every path of the go engine.
        //
        // When the collection is a name — on its own or at the root of a chain
        // like `m[i][j]` — the value is borrowed from the environment instead
        // and only the element is cloned. Same semantics: a read always did
        // return a copy of the element.
        let mut chain: Vec<&IndexExpr> = Vec::new();
        if let Some(root) = Self::index_chain(idx, &mut chain) {
            self.check_variable_alive(&root.name, &root.span)?;
            // A name the environment does not hold may still be a function used
            // as a value; that (and the error) belongs to the general path.
            if self.get_variable(&root.name).is_some() {
                // The indices are evaluated before anything is borrowed, in the
                // same order as the general path: innermost first.
                let mut indices = Vec::with_capacity(chain.len());
                for link in chain.iter().rev() {
                    indices.push((self.eval_expr(&link.index)?, link.span));
                }

                // Checked just above, and nothing between here and there can
                // remove it — the index expressions ran before the borrow.
                let Some(mut collection) = self.get_variable(&root.name) else {
                    unreachable!()
                };
                let mut step = 0;
                loop {
                    let (ref index_value, span) = indices[step];
                    let borrowable = matches!(
                        collection,
                        Value::Array(_) | Value::Tuple(_) | Value::NamedTuple(_)
                    );
                    // The last step, and any step through something whose
                    // elements are not Values living inside it (a String yields
                    // a fresh Char), finishes by value.
                    if !borrowable || step + 1 == indices.len() {
                        let mut value = Self::index_into(collection, index_value, span)?;
                        for (index_value, span) in &indices[step + 1..] {
                            value = Self::index_into(&value, index_value, *span)?;
                        }
                        return Ok(value);
                    }
                    // `borrowable` above is exactly the set `index_ref` answers
                    // with `Some`.
                    let Some(element) = Self::index_ref(collection, index_value, span)? else {
                        unreachable!()
                    };
                    collection = element;
                    step += 1;
                }
            }
        }

        let collection_value = self.eval_expr(&idx.array)?;
        let index_value = self.eval_expr(&idx.index)?;
        Self::index_into(&collection_value, &index_value, idx.span)
    }

    /// Walk `a[i][j][k]` down to the name at its root, collecting the links
    /// outermost-first. `None` when the root is anything but a plain name.
    fn index_chain<'e>(idx: &'e IndexExpr, chain: &mut Vec<&'e IndexExpr>) -> Option<&'e IdentifierExpr> {
        chain.push(idx);
        match idx.array.unwrap_group() {
            Expr::Identifier(id) => Some(id),
            Expr::Index(inner) => Self::index_chain(inner, chain),
            _ => None,
        }
    }

    /// Address one element of a collection, borrowing it where it lives.
    ///
    /// `Ok(None)` means the element is not a Value held inside the collection —
    /// a String hands back a fresh `Char`, and anything else is not indexable.
    /// `index_into` finishes those two cases.
    fn index_ref<'v>(
        collection: &'v Value,
        index_value: &Value,
        span: zymbol_span::Span,
    ) -> Result<Option<&'v Value>> {
        // A dictionary is addressed by KEY, and the key may be computed —
        // `d[clave]`, not just `d.nombre`. Without this the named tuple was a
        // record, not a dictionary: readable only when the program already knew
        // what it held (decision 7, DM-09).
        //
        // The dot only reaches keys that are identifiers; the bracket reaches
        // any key, which is what JSON needs — `d["mi clave"]` cannot be written
        // any other way.
        if let (Value::NamedTuple(fields), Value::String(key)) = (collection, index_value) {
            if let Some((_, v)) = fields.iter().find(|(k, _)| k == key) {
                return Ok(Some(v));
            }
            let available: Vec<String> = fields.iter().map(|(k, _)| k.clone()).collect();
            return Err(RuntimeError::Generic {
                message: crate::variables::missing_key_msg(key, &available),
                span,
            });
        }

        // Decision 11: a dictionary is addressed by KEY, never by position.
        // In a mutable dictionary a positional index is fragile — adding a key
        // changes what sits at each position, and a program that depended on
        // `d[2]` stops being correct with nothing to say so.
        //
        // The POSITIONAL tuple keeps `t[1]` in full: there the index is the only
        // address there is, and the size is fixed.
        if let (Value::NamedTuple(fields), Value::Int(_)) = (collection, index_value) {
            let first = fields.first().map(|(k, _)| k.clone()).unwrap_or_else(|| "clave".into());
            return Err(RuntimeError::Generic {
                message: format!(
                    "a dictionary is addressed by key, not by position\nhelp: use d[\"{}\"] — adding a key changes what sits at each position",
                    first
                ),
                span,
            });
        }

        // Extract index
        let index = match index_value {
            Value::Int(n) => *n,
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("index must be an integer, got {:?}", index_value),
                    span,
                })
            }
        };

        match collection {
            Value::Array(arr) => {
                let len = arr.len();
                match Self::resolve_index(index, len, span)? {
                    Some(i) => Ok(Some(&arr[i])),
                    None => Err(RuntimeError::Generic {
                        message: format!(
                            "array index out of bounds: index {} for array of length {}",
                            index, len
                        ),
                        span,
                    }),
                }
            }
            Value::Tuple(elements) => {
                let len = elements.len();
                match Self::resolve_index(index, len, span)? {
                    Some(i) => Ok(Some(&elements[i])),
                    None => Err(RuntimeError::Generic {
                        message: format!(
                            "tuple index out of bounds: index {} for tuple of length {}",
                            index, len
                        ),
                        span,
                    }),
                }
            }
            Value::NamedTuple(fields) => {
                // Unreachable: a named tuple with an Int index already returned
                // above, and every other index kind is rejected. Kept so the
                // shape of the match matches the shape of the values — and with
                // the wording it always had, down to naming the length after
                // the tuple.
                let len = fields.len();
                match Self::resolve_index(index, len, span)? {
                    Some(i) => Ok(Some(&fields[i].1)),
                    None => Err(RuntimeError::Generic {
                        message: format!(
                            "named tuple index out of bounds: index {} for tuple of length {}",
                            index, len
                        ),
                        span,
                    }),
                }
            }
            _ => Ok(None),
        }
    }

    /// Address one element, producing an owned value.
    ///
    /// Everything `index_ref` can borrow is cloned from it; what is left is the
    /// String, whose element is built on the spot, and the error.
    fn index_into(
        collection: &Value,
        index_value: &Value,
        span: zymbol_span::Span,
    ) -> Result<Value> {
        if let Some(element) = Self::index_ref(collection, index_value, span)? {
            return Ok(element.clone());
        }

        match collection {
            Value::String(s) => {
                // String indexing returns a char
                let chars: Vec<char> = s.chars().collect();
                // `index_ref` rejected every other index kind before this point.
                let Value::Int(index) = index_value else { unreachable!() };
                let len = chars.len();
                match Self::resolve_index(*index, len, span)? {
                    Some(i) => Ok(Value::Char(chars[i])),
                    None => Err(RuntimeError::Generic {
                        message: format!(
                            "string index out of bounds: index {} for string of length {}",
                            index, len
                        ),
                        span,
                    }),
                }
            }
            _ => Err(RuntimeError::Generic {
                message: format!("cannot index {:?} - only arrays, tuples, and strings are indexable", collection),
                span,
            }),
        }
    }

    /// Turn a 1-based Zymbol index into a 0-based offset. `None` means it falls
    /// outside the collection.
    ///
    /// Index 0 is answered here because its message is one text for every
    /// collection. Out of bounds is answered by the caller, because that message
    /// NAMES the collection — and each of those names is a message of its own,
    /// which is how the diagnostic inventory compares one engine with another.
    fn resolve_index(index: i64, len: usize, span: zymbol_span::Span) -> Result<Option<usize>> {
        if index == 0 {
            return Err(RuntimeError::Generic {
                message: "index 0 is invalid — Zymbol uses 1-based indexing (use 1 for the first element, -1 for the last)".to_string(),
                span,
            });
        }
        let i = if index < 0 { len as i64 + index } else { index - 1 };
        if i < 0 || i as usize >= len {
            return Ok(None);
        }
        Ok(Some(i as usize))
    }
}
