//! std/json — JSON decoding and encoding for Zymbol-Lang.
//!
//! Two inverse operations:
//!   decode(text)  -> Value    parse a JSON string into a Zymbol value
//!   encode(value) -> String   serialize a Zymbol value into a JSON string
//!
//! Error convention (shared with the rest of the stdlib):
//!   - Hard `RuntimeError` for programmer misuse (wrong argument type).
//!   - Soft `Value::Error` for recoverable failures (malformed JSON, encode failure),
//!     so they can be caught with try-catch.
//!
//! Mapping between JSON and Zymbol values:
//!   JSON object  <-> NamedTuple   (key order preserved via serde_json "preserve_order")
//!   JSON array   <-> Array
//!   JSON null    <-> Unit
//!   number       -> Int when integral, otherwise Float
//!
//! For localized names, use the i18n three-layer pattern to re-export under the
//! target language's names (e.g. Spanish: decodificar, codificar).

use crate::{ErrorValue, FunctionDef, Result, RuntimeError, Value};
use std::collections::HashMap;
use std::rc::Rc;
use zymbol_span::Span;

/// Convert a serde_json value into a Zymbol value.
fn json_to_value(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Unit,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else {
                Value::Float(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(arr) => {
            Value::Array(arr.into_iter().map(json_to_value).collect())
        }
        serde_json::Value::Object(map) => Value::NamedTuple(
            map.into_iter().map(|(k, v)| (k, json_to_value(v))).collect(),
        ),
    }
}

/// Convert a Zymbol value into a serde_json value.
/// Tuples encode as arrays; named tuples as objects. Functions/errors become null.
fn value_to_json(v: Value) -> serde_json::Value {
    match v {
        Value::Unit => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(b),
        Value::Int(i) => serde_json::json!(i),
        Value::Float(f) => serde_json::json!(f),
        Value::String(s) => serde_json::Value::String(s),
        Value::Char(c) => serde_json::Value::String(c.to_string()),
        Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(value_to_json).collect())
        }
        Value::Tuple(fields) => {
            serde_json::Value::Array(fields.into_iter().map(value_to_json).collect())
        }
        Value::NamedTuple(pairs) => serde_json::Value::Object(
            pairs.into_iter().map(|(k, v)| (k, value_to_json(v))).collect(),
        ),
        Value::Function(_) | Value::Error(_) => serde_json::Value::Null,
    }
}

/// json::decode("text") -> Value | Error
fn json_decode(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.into_iter().next() {
        Some(Value::String(text)) => match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(v) => Ok(json_to_value(v)),
            Err(e) => Ok(Value::Error(ErrorValue::parse(e.to_string()))),
        },
        _ => Err(RuntimeError::Generic {
            message: "json::decode: expected String".into(),
            span,
        }),
    }
}

/// Build a source→target key-rename table from a NamedTuple map argument.
/// Each field name is a source (e.g. JSON) key; its String value is the new name.
fn build_rename_map(map: Value, span: Span) -> Result<HashMap<String, String>> {
    match map {
        // An empty `()` (Unit) means "no renames" — decode_map behaves like decode.
        Value::Unit => Ok(HashMap::new()),
        Value::NamedTuple(pairs) => {
            let mut table = HashMap::with_capacity(pairs.len());
            for (src, dst) in pairs {
                match dst {
                    Value::String(name) => {
                        table.insert(src, name);
                    }
                    _ => {
                        return Err(RuntimeError::Generic {
                            message: format!(
                                "json::decode_map: map value for '{}' must be a String (the new name)",
                                src
                            ),
                            span,
                        })
                    }
                }
            }
            Ok(table)
        }
        _ => Err(RuntimeError::Generic {
            message: "json::decode_map: expected a NamedTuple map as the second argument".into(),
            span,
        }),
    }
}

/// Recursively rename NamedTuple field names according to `table`, at any depth.
/// Arrays and tuples are traversed; scalars are returned unchanged.
fn rekey(value: Value, table: &HashMap<String, String>) -> Value {
    match value {
        Value::NamedTuple(pairs) => Value::NamedTuple(
            pairs
                .into_iter()
                .map(|(k, v)| {
                    let new_key = table.get(&k).cloned().unwrap_or(k);
                    (new_key, rekey(v, table))
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(|v| rekey(v, table)).collect()),
        Value::Tuple(items) => Value::Tuple(items.into_iter().map(|v| rekey(v, table)).collect()),
        other => other,
    }
}

/// json::decode_map("text", (src: "dst", ...)) -> Value | Error
/// Decodes JSON, then recursively renames object keys per the map so the
/// resulting structure reads in the consumer's language (data-level i18n).
fn json_decode_map(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    let text = match it.next() {
        Some(Value::String(text)) => text,
        _ => {
            return Err(RuntimeError::Generic {
                message: "json::decode_map: expected String as the first argument".into(),
                span,
            })
        }
    };
    let table = build_rename_map(it.next().unwrap_or(Value::Unit), span)?;
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(v) => Ok(rekey(json_to_value(v), &table)),
        Err(e) => Ok(Value::Error(ErrorValue::parse(e.to_string()))),
    }
}

/// json::encode(value) -> String | Error
fn json_encode(args: Vec<Value>, _span: Span) -> Result<Value> {
    // Arity is validated by the dispatcher; one argument is guaranteed here.
    let value = args.into_iter().next().unwrap_or(Value::Unit);
    match serde_json::to_string(&value_to_json(value)) {
        Ok(s) => Ok(Value::String(s)),
        Err(e) => Ok(Value::Error(ErrorValue::parse(e.to_string()))),
    }
}

// --- Registry -------------------------------------------------------------

pub(crate) fn register() -> HashMap<String, Rc<FunctionDef>> {
    let mut m: HashMap<String, Rc<FunctionDef>> = HashMap::new();

    macro_rules! native {
        ($name:literal, $arity:expr, $fn:expr) => {
            m.insert($name.into(), Rc::new(FunctionDef::Native {
                name: $name, arity: $arity, func: $fn,
            }));
        };
    }

    native!("decode",     1, json_decode);
    native!("decode_map", 2, json_decode_map);
    native!("encode",     1, json_encode);

    m
}
