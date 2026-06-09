//! std/net — synchronous HTTP for Zymbol-Lang.
//!
//! Blocking requests via `ureq` (no async, no tokio). Network failures (DNS,
//! timeout, non-2xx) are returned as a soft `Value::Error` of kind "Network" so
//! they can be caught with try-catch. A wrong argument type is a programmer error
//! and aborts with a hard `RuntimeError`.
//!
//!   get(url[, headers])             -> String | Error    HTTP GET, returns body
//!   post(url, body[, headers])      -> String | Error    POST text/plain, returns body
//!   post_json(url, body[, headers]) -> String | Error    POST application/json, returns body
//!   head(url)                       -> Bool               #1 if reachable (2xx), else #0
//!
//! The optional `headers` argument is an Array of 2-element `(String, String)`
//! tuples — e.g. `[("x-api-key", key), ("anthropic-version", "2023-06-01")]`.
//! Tuples are used (not named tuples) because header names like `x-api-key`
//! contain hyphens, which are not valid field identifiers. This is what lets the
//! module reach authenticated APIs (Claude, OpenAI, Groq, …).
//!
//! For localized names, use the i18n three-layer pattern to re-export under the
//! target language's names (e.g. Spanish: obtener, enviar, …).

use crate::{ErrorValue, FunctionDef, Result, RuntimeError, Value};
use std::collections::HashMap;
use std::rc::Rc;
use zymbol_span::Span;

fn net_error(msg: impl Into<String>) -> Value {
    Value::Error(ErrorValue::new("Network", msg))
}

/// Parse the optional `headers` argument into (name, value) pairs.
/// Accepts an Array of 2-element (String, String) tuples; `None`/absent → no headers.
fn parse_headers(arg: Option<Value>, fname: &str, span: Span) -> Result<Vec<(String, String)>> {
    let value = match arg {
        None => return Ok(Vec::new()),
        Some(v) => v,
    };
    let bad = || RuntimeError::Generic {
        message: format!("{}: headers must be an Array of (String, String) tuples", fname),
        span,
    };
    match value {
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::Tuple(pair) if pair.len() == 2 => match (&pair[0], &pair[1]) {
                        (Value::String(k), Value::String(v)) => out.push((k.clone(), v.clone())),
                        _ => return Err(bad()),
                    },
                    _ => return Err(bad()),
                }
            }
            Ok(out)
        }
        _ => Err(bad()),
    }
}

/// Convert a ureq response into a Zymbol value (body String, or soft Network error).
fn response_to_value(resp: std::result::Result<ureq::Response, ureq::Error>) -> Value {
    match resp {
        Ok(resp) => match resp.into_string() {
            Ok(body) => Value::String(body),
            Err(e) => net_error(e.to_string()),
        },
        Err(e) => net_error(e.to_string()),
    }
}

/// net::get("url"[, headers]) -> String | Error
fn net_get(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    let url = match it.next() {
        Some(Value::String(u)) => u,
        _ => return Err(RuntimeError::Generic {
            message: "net::get: expected String url".into(),
            span,
        }),
    };
    let headers = parse_headers(it.next(), "net::get", span)?;
    let mut req = ureq::get(&url);
    for (k, v) in &headers {
        req = req.set(k, v);
    }
    Ok(response_to_value(req.call()))
}

/// net::post("url", "body"[, headers]) -> String | Error  (Content-Type: text/plain)
fn net_post(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    match (it.next(), it.next()) {
        (Some(Value::String(url)), Some(Value::String(body))) => {
            let headers = parse_headers(it.next(), "net::post", span)?;
            let mut req = ureq::post(&url).set("Content-Type", "text/plain");
            for (k, v) in &headers {
                req = req.set(k, v);
            }
            Ok(response_to_value(req.send_string(&body)))
        }
        _ => Err(RuntimeError::Generic {
            message: "net::post: expected (String, String)".into(),
            span,
        }),
    }
}

/// net::post_json("url", "body"[, headers]) -> String | Error  (Content-Type: application/json)
fn net_post_json(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    match (it.next(), it.next()) {
        (Some(Value::String(url)), Some(Value::String(body))) => {
            let headers = parse_headers(it.next(), "net::post_json", span)?;
            let mut req = ureq::post(&url).set("Content-Type", "application/json");
            for (k, v) in &headers {
                req = req.set(k, v);
            }
            Ok(response_to_value(req.send_string(&body)))
        }
        _ => Err(RuntimeError::Generic {
            message: "net::post_json: expected (String, String)".into(),
            span,
        }),
    }
}

/// net::head("url") -> Bool  (#1 if reachable, #0 on error)
fn net_head(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.into_iter().next() {
        Some(Value::String(url)) => Ok(Value::Bool(ureq::head(&url).call().is_ok())),
        _ => Err(RuntimeError::Generic {
            message: "net::head: expected String url".into(),
            span,
        }),
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

    // arity -1 (variadic): get/post/post_json take an optional trailing headers arg.
    native!("get",       -1, net_get);
    native!("post",      -1, net_post);
    native!("post_json", -1, net_post_json);
    native!("head",       1, net_head);

    m
}
