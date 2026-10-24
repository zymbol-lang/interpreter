//! std/net — synchronous HTTP for Zymbol-Lang.
//!
//! Blocking requests via `ureq` (no async, no tokio). Network failures (DNS,
//! timeout, non-2xx) are returned as a soft `Value::Error` of kind "Network" so
//! they can be caught with try-catch. A wrong argument type is a programmer error
//! and aborts with a hard `RuntimeError`.
//!
//!   get(url[, headers[, seconds]])             -> String | Error    HTTP GET, returns body
//!   post(url, body[, headers[, seconds]])      -> String | Error    POST text/plain, returns body
//!   post_json(url, body[, headers[, seconds]]) -> String | Error    POST application/json, returns body
//!   head(url[, seconds])                       -> Bool               #1 if reachable (2xx), else #0
//!
//!   timeout(seconds)                           -> Int | Float        the limit it replaces
//!
//! `seconds` is a time limit for the whole request, an Int or a Float, `0` or
//! more, `0` being none (ZethyCLI G1, decided by the author on 2026-10-24): past
//! it the answer is a soft `##Network(timeout: no answer within N s)`, and `head`
//! is `#0`. A request without its own takes the process's, set by `timeout`, and
//! without either it waits as long as the server takes — a server that accepts
//! and never answers held all three engines past two minutes. The rules are
//! `zymbol_common::net`, shared with the VM.
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
    Value::error(ErrorValue::new("Network", msg))
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
            for item in crate::own_elements(items) {
                match item {
                    Value::Tuple(pair) if pair.len() == 2 => match (&pair[0], &pair[1]) {
                        (Value::String(k), Value::String(v)) => out.push((k.to_string(), v.to_string())),
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

use zymbol_common::net::Limit;

/// A time limit as the program wrote it: seconds, an Int or a Float, `0` or
/// more (ZethyCLI G1).
fn limit_of(arg: Value, fname: &str, span: Span) -> Result<Limit> {
    match arg {
        Value::Int(n) if n >= 0 => Ok(Limit::Int(n)),
        Value::Float(f) if f >= 0.0 && f.is_finite() => Ok(Limit::Float(f)),
        _ => Err(RuntimeError::Generic {
            message: zymbol_common::net::bad_limit_message(fname),
            span,
        }),
    }
}

/// The seconds this request waits: the limit after its headers when it gave
/// one (`0` is none), the process's otherwise (`net::timeout`), `None` for none.
fn parse_limit(arg: Option<Value>, fname: &str, span: Span) -> Result<Option<f64>> {
    let own = arg.map(|v| limit_of(v, fname, span)).transpose()?;
    Ok(zymbol_common::net::effective(own))
}

/// net::timeout(seconds) -> the limit it replaces (0 when there was none)
fn net_timeout(args: Vec<Value>, span: Span) -> Result<Value> {
    let limit = match args.into_iter().next() {
        Some(v) => limit_of(v, "net::timeout", span)?,
        None => return Err(RuntimeError::Generic {
            message: zymbol_common::net::bad_limit_message("net::timeout"),
            span,
        }),
    };
    Ok(match zymbol_common::net::set_default(limit) {
        Limit::Int(n) => Value::Int(n),
        Limit::Float(f) => Value::Float(f),
    })
}

/// A request with the time limit applied, when there is one.
fn limited<B>(req: ureq::RequestBuilder<B>, limit: Option<f64>) -> ureq::RequestBuilder<B> {
    req.config().timeout_global(limit.map(std::time::Duration::from_secs_f64)).build()
}

/// Convert a ureq response into a Zymbol value (body String, or soft Network error).
/// Out of time, the words every engine uses (ZethyCLI G1).
fn response_to_value(
    resp: std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    limit: Option<f64>,
) -> Value {
    let said = |e: ureq::Error| match (e, limit) {
        (ureq::Error::Timeout(_), Some(s)) => zymbol_common::net::timeout_message(s),
        (e, _) => e.to_string(),
    };
    match resp {
        Ok(mut resp) => match resp.body_mut().read_to_string() {
            Ok(body) => Value::String(body.into()),
            Err(e) => net_error(said(e)),
        },
        Err(e) => net_error(said(e)),
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
    let limit = parse_limit(it.next(), "net::get", span)?;
    let mut req = limited(ureq::get(url.as_str()), limit);
    for (k, v) in &headers {
        req = req.header(k, v);
    }
    Ok(response_to_value(req.call(), limit))
}

/// net::post("url", "body"[, headers]) -> String | Error  (Content-Type: text/plain)
fn net_post(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    match (it.next(), it.next()) {
        (Some(Value::String(url)), Some(Value::String(body))) => {
            let headers = parse_headers(it.next(), "net::post", span)?;
            let limit = parse_limit(it.next(), "net::post", span)?;
            let mut req = limited(ureq::post(url.as_str()), limit).header("Content-Type", "text/plain");
            for (k, v) in &headers {
                req = req.header(k, v);
            }
            Ok(response_to_value(req.send(body.as_str()), limit))
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
            let limit = parse_limit(it.next(), "net::post_json", span)?;
            let mut req = limited(ureq::post(url.as_str()), limit).header("Content-Type", "application/json");
            for (k, v) in &headers {
                req = req.header(k, v);
            }
            Ok(response_to_value(req.send(body.as_str()), limit))
        }
        _ => Err(RuntimeError::Generic {
            message: "net::post_json: expected (String, String)".into(),
            span,
        }),
    }
}

/// net::head("url"[, seconds]) -> Bool  (#1 if reachable, #0 on error or out of time)
fn net_head(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    match it.next() {
        Some(Value::String(url)) => {
            let limit = parse_limit(it.next(), "net::head", span)?;
            Ok(Value::Bool(limited(ureq::head(url.as_str()), limit).call().is_ok()))
        }
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

    // arity -1 (variadic): get/post/post_json take optional headers and a time
    // limit after them, head a time limit (ZethyCLI G1).
    native!("get",       -1, net_get);
    native!("post",      -1, net_post);
    native!("post_json", -1, net_post_json);
    native!("head",      -1, net_head);
    native!("timeout",    1, net_timeout);

    m
}
