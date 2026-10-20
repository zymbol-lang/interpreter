//! Native stdlib function dispatch for the register VM.
//!
//! Mirrors the implementations in zymbol-interpreter/src/stdlib/, but without
//! the interpreter dependency. Called from CallBuiltin instructions.
//!
//! What needs the system — the clock, the seed of `std/random`, `std/net`,
//! `std/db` — goes through the [`Host`] the VM was given (3.1).

use zymbol_common::num;
use std::collections::HashMap;
use std::rc::Rc;

use zymbol_bytecode::builtins as B;

use crate::host::{Host, HttpError, HttpMethod, HttpRequest};
use crate::{Value, ZyStr};

#[inline]
fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Float(x) => Some(*x),
        Value::Int(x)   => Some(*x as f64),
        _               => None,
    }
}

fn type_err(fname: &str, args: &[Value]) -> String {
    // The types in words, joined — as the tree-walker says them since GLB-033.
    let types: Vec<String> = args.iter().map(|v| v.type_label()).collect();
    format!("mat::{}: incompatible argument type(s) {}", fname, types.join(", "))
}

// ── std/math ─────────────────────────────────────────────────────────────────

fn math_sqrt(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.sqrt())),
        None    => Err(type_err("sqrt", &args)),
    }
}

fn math_exp(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.exp())),
        None    => Err(type_err("exp", &args)),
    }
}

fn math_ln(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) if x > 0.0 => Ok(Value::Float(x.ln())),
        Some(_) => Err("mat::ln: argument must be positive".into()),
        None    => Err(type_err("ln", &args)),
    }
}

fn math_log(args: Vec<Value>) -> Result<Value, String> {
    match (args.first().and_then(as_f64), args.get(1).and_then(as_f64)) {
        (Some(x), Some(base)) if x > 0.0 && base > 0.0 && base != 1.0 => {
            Ok(Value::Float(x.log(base)))
        }
        (Some(x), None) if x > 0.0 => Ok(Value::Float(x.ln())),
        (Some(_), Some(_)) => Err("mat::log: x and base must be positive; base ≠ 1".into()),
        _ => Err(type_err("log", &args)),
    }
}

fn math_pow(args: Vec<Value>) -> Result<Value, String> {
    match (args.first().and_then(as_f64), args.get(1).and_then(as_f64)) {
        (Some(b), Some(e)) => Ok(Value::Float(b.powf(e))),
        _ => Err(type_err("pow", &args)),
    }
}

fn math_sin(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.sin())),
        None    => Err(type_err("sin", &args)),
    }
}

fn math_cos(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.cos())),
        None    => Err(type_err("cos", &args)),
    }
}

fn math_tan(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.tan())),
        None    => Err(type_err("tan", &args)),
    }
}

fn math_asin(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) if (-1.0..=1.0).contains(&x) => Ok(Value::Float(x.asin())),
        Some(_) => Err("mat::asin: argument must be in [-1, 1]".into()),
        None    => Err(type_err("asin", &args)),
    }
}

fn math_acos(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) if (-1.0..=1.0).contains(&x) => Ok(Value::Float(x.acos())),
        Some(_) => Err("mat::acos: argument must be in [-1, 1]".into()),
        None    => Err(type_err("acos", &args)),
    }
}

fn math_atan(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.atan())),
        None    => Err(type_err("atan", &args)),
    }
}

fn math_atan2(args: Vec<Value>) -> Result<Value, String> {
    match (args.first().and_then(as_f64), args.get(1).and_then(as_f64)) {
        (Some(y), Some(x)) => Ok(Value::Float(y.atan2(x))),
        _ => Err(type_err("atan2", &args)),
    }
}

fn math_tanh(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.tanh())),
        None    => Err(type_err("tanh", &args)),
    }
}

fn math_sinh(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.sinh())),
        None    => Err(type_err("sinh", &args)),
    }
}

fn math_cosh(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.cosh())),
        None    => Err(type_err("cosh", &args)),
    }
}

fn math_sigmoid(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(1.0 / (1.0 + (-x).exp()))),
        None    => Err(type_err("sigmoid", &args)),
    }
}

fn math_abs(args: Vec<Value>) -> Result<Value, String> {
    match args.first() {
        Some(Value::Int(x))   => Ok(Value::Int(x.abs())),
        Some(Value::Float(x)) => Ok(Value::Float(x.abs())),
        _ => Err(type_err("abs", &args)),
    }
}

fn math_max(args: Vec<Value>) -> Result<Value, String> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(a)), Some(Value::Int(b))) => Ok(Value::Int(*a.max(b))),
        (Some(a), Some(b)) => match (as_f64(a), as_f64(b)) {
            (Some(fa), Some(fb)) => Ok(Value::Float(fa.max(fb))),
            _ => Err(type_err("max", &args)),
        },
        _ => Err(type_err("max", &args)),
    }
}

fn math_min(args: Vec<Value>) -> Result<Value, String> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(a)), Some(Value::Int(b))) => Ok(Value::Int(*a.min(b))),
        (Some(a), Some(b)) => match (as_f64(a), as_f64(b)) {
            (Some(fa), Some(fb)) => Ok(Value::Float(fa.min(fb))),
            _ => Err(type_err("min", &args)),
        },
        _ => Err(type_err("min", &args)),
    }
}

fn math_floor(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.floor())),
        None    => Err(type_err("floor", &args)),
    }
}

fn math_ceil(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.ceil())),
        None    => Err(type_err("ceil", &args)),
    }
}

fn math_round(args: Vec<Value>) -> Result<Value, String> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.round())),
        None    => Err(type_err("round", &args)),
    }
}

// ── std/random ────────────────────────────────────────────────────────────────

use std::cell::Cell;

thread_local! {
    static STATE:  Cell<[u64; 4]> = const { Cell::new([0u64; 4]) };
    static SEEDED: Cell<bool>     = const { Cell::new(false) };
}

fn xoshiro_next(host: &mut dyn Host) -> u64 {
    STATE.with(|s| {
        let mut st = s.get();
        if !SEEDED.with(|b| b.get()) {
            let ns = host.entropy();
            st = [
                ns ^ 0xdead_beef,
                ns.wrapping_mul(6_364_136_223_846_793_005),
                ns ^ 0x00c0_ffee,
                ns.wrapping_add(1_442_695_040_888_963_407),
            ];
            SEEDED.with(|b| b.set(true));
        }
        let result = st[0].wrapping_add(st[3]).rotate_left(23).wrapping_add(st[0]);
        let t = st[1] << 17;
        st[2] ^= st[0]; st[3] ^= st[1]; st[1] ^= st[2]; st[0] ^= st[3];
        st[2] ^= t; st[3] = st[3].rotate_left(45);
        s.set(st);
        result
    })
}

fn rand_entero(args: Vec<Value>, host: &mut dyn Host) -> Result<Value, String> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(lo)), Some(Value::Int(hi))) if hi >= lo => {
            let range = (hi - lo + 1) as u64;
            Ok(Value::Int(*lo + (xoshiro_next(host) % range) as i64))
        }
        _ => Err("random::entero: expected (###, ###) with max >= min".into()),
    }
}

fn rand_rango(args: Vec<Value>, host: &mut dyn Host) -> Result<Value, String> {
    match args.first() {
        Some(Value::Int(n)) if *n > 0 => {
            Ok(Value::Int((xoshiro_next(host) % (*n as u64)) as i64))
        }
        _ => Err("random::rango: expected positive ###".into()),
    }
}

fn rand_peso_f64(_args: Vec<Value>, host: &mut dyn Host) -> Result<Value, String> {
    Ok(Value::Float(((xoshiro_next(host) % 201) as f64 - 100.0) / 1000.0))
}

// ── std/json ───────────────────────────────────────────────────────────────────
//
// Mirrors zymbol-interpreter/src/stdlib/json.rs. Malformed JSON and encode
// failures are returned as soft `Value::Error` (catchable), not as `Err`.

/// Convert a serde_json value into a VM value.
fn json_to_value(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Unit,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            // Past the Zymbol range a JSON integer becomes a Float, as it does
            // in the tree-walker and as the browser's own parser would have it.
            match n.as_i64().filter(|i| num::in_int_range(*i)) {
                Some(i) => Value::Int(i),
                None => Value::Float(n.as_f64().unwrap_or(f64::NAN)),
            }
        }
        serde_json::Value::String(s) => Value::String(ZyStr::new(s)),
        serde_json::Value::Array(arr) => {
            Value::Array(Rc::new(arr.into_iter().map(json_to_value).collect()))
        }
        serde_json::Value::Object(map) => Value::NamedTuple(Rc::new(
            map.into_iter().map(|(k, v)| (k, json_to_value(v))).collect(),
        )),
    }
}

/// Convert a VM value into a serde_json value.
/// Tuples encode as arrays; named tuples as objects. Functions/errors become null.
fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Unit => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::json!(i),
        Value::Float(f) => serde_json::json!(f),
        Value::String(s) => serde_json::Value::String(s.as_str().to_string()),
        Value::Char(c) => serde_json::Value::String(c.to_string()),
        Value::Array(arr) | Value::Tuple(arr) => {
            serde_json::Value::Array(arr.iter().map(value_to_json).collect())
        }
        Value::NamedTuple(pairs) => serde_json::Value::Object(
            pairs.iter().map(|(k, v)| (k.clone(), value_to_json(v))).collect(),
        ),
        Value::Function(..) | Value::Closure(..) | Value::Error(_) => serde_json::Value::Null,
    }
}

fn json_decode(args: Vec<Value>) -> Result<Value, String> {
    match args.into_iter().next() {
        Some(Value::String(text)) => match serde_json::from_str::<serde_json::Value>(text.as_str()) {
            Ok(v) => Ok(json_to_value(v)),
            Err(e) => Ok(Value::Error(ZyStr::new(format!("##Parse({})", e)))),
        },
        _ => Err("json::decode: expected String".into()),
    }
}

/// Build a source→target key-rename table from a NamedTuple map argument.
fn build_rename_map(map: Value) -> Result<HashMap<String, String>, String> {
    match map {
        // An empty `()` (Unit) means "no renames" — decode_map behaves like decode.
        Value::Unit => Ok(HashMap::new()),
        Value::NamedTuple(pairs) => {
            let mut table = HashMap::with_capacity(pairs.len());
            for (src, dst) in pairs.iter() {
                match dst {
                    Value::String(name) => {
                        table.insert(src.clone(), name.as_str().to_string());
                    }
                    _ => {
                        return Err(format!(
                            "json::decode_map: map value for '{}' must be a String (the new name)",
                            src
                        ))
                    }
                }
            }
            Ok(table)
        }
        _ => Err("json::decode_map: expected a NamedTuple map as the second argument".into()),
    }
}

/// Recursively rename NamedTuple field names according to `table`, at any depth.
fn rekey(value: Value, table: &HashMap<String, String>) -> Value {
    match value {
        Value::NamedTuple(pairs) => Value::NamedTuple(Rc::new(
            pairs
                .iter()
                .map(|(k, v)| {
                    let new_key = table.get(k).cloned().unwrap_or_else(|| k.clone());
                    (new_key, rekey(v.clone(), table))
                })
                .collect(),
        )),
        Value::Array(items) => {
            Value::Array(Rc::new(items.iter().map(|v| rekey(v.clone(), table)).collect()))
        }
        Value::Tuple(items) => {
            Value::Tuple(Rc::new(items.iter().map(|v| rekey(v.clone(), table)).collect()))
        }
        other => other,
    }
}

fn json_decode_map(args: Vec<Value>) -> Result<Value, String> {
    let mut it = args.into_iter();
    let text = match it.next() {
        Some(Value::String(text)) => text,
        _ => return Err("json::decode_map: expected String as the first argument".into()),
    };
    let table = build_rename_map(it.next().unwrap_or(Value::Unit))?;
    match serde_json::from_str::<serde_json::Value>(text.as_str()) {
        Ok(v) => Ok(rekey(json_to_value(v), &table)),
        Err(e) => Ok(Value::Error(ZyStr::new(format!("##Parse({})", e)))),
    }
}

fn json_encode(args: Vec<Value>) -> Result<Value, String> {
    // Arity is validated by the compiler/dispatcher; one argument is guaranteed.
    let value = args.first().cloned().unwrap_or(Value::Unit);
    match serde_json::to_string(&value_to_json(&value)) {
        Ok(s) => Ok(Value::String(ZyStr::new(s))),
        Err(e) => Ok(Value::Error(ZyStr::new(format!("##Parse({})", e)))),
    }
}

// ── std/io ───────────────────────────────────────────────────────────────────
//
// Mirrors zymbol-interpreter/src/stdlib/io.rs. Filesystem failures are returned as
// soft `Value::Error("##IO(...)")` (catchable); a wrong argument type is a hard `Err`.

fn io_err(e: std::io::Error) -> Value {
    Value::Error(ZyStr::new(format!("##IO({})", e)))
}

fn io_read(args: Vec<Value>) -> Result<Value, String> {
    match args.into_iter().next() {
        Some(Value::String(path)) => match std::fs::read_to_string(path.as_str()) {
            Ok(content) => Ok(Value::String(ZyStr::new(content))),
            Err(e) => Ok(io_err(e)),
        },
        _ => Err("io::read: expected String path".into()),
    }
}

fn io_write(args: Vec<Value>) -> Result<Value, String> {
    let mut it = args.into_iter();
    match (it.next(), it.next()) {
        (Some(Value::String(path)), Some(Value::String(content))) => {
            match std::fs::write(path.as_str(), content.as_str().as_bytes()) {
                Ok(_) => Ok(Value::Unit),
                Err(e) => Ok(io_err(e)),
            }
        }
        _ => Err("io::write: expected (String, String)".into()),
    }
}

fn io_append(args: Vec<Value>) -> Result<Value, String> {
    use std::io::Write;
    let mut it = args.into_iter();
    match (it.next(), it.next()) {
        (Some(Value::String(path)), Some(Value::String(content))) => {
            match std::fs::OpenOptions::new().append(true).create(true).open(path.as_str()) {
                Ok(mut f) => match f.write_all(content.as_str().as_bytes()) {
                    Ok(_) => Ok(Value::Unit),
                    Err(e) => Ok(io_err(e)),
                },
                Err(e) => Ok(io_err(e)),
            }
        }
        _ => Err("io::append: expected (String, String)".into()),
    }
}

fn io_exists(args: Vec<Value>) -> Result<Value, String> {
    match args.into_iter().next() {
        Some(Value::String(path)) => Ok(Value::Bool(std::path::Path::new(path.as_str()).exists())),
        _ => Err("io::exists: expected String path".into()),
    }
}

fn io_delete(args: Vec<Value>) -> Result<Value, String> {
    match args.into_iter().next() {
        Some(Value::String(path)) => {
            let p = std::path::Path::new(path.as_str());
            let result = if p.is_dir() {
                std::fs::remove_dir_all(p)
            } else {
                std::fs::remove_file(p)
            };
            match result {
                Ok(_) => Ok(Value::Unit),
                Err(e) => Ok(io_err(e)),
            }
        }
        _ => Err("io::delete: expected String path".into()),
    }
}

fn io_list(args: Vec<Value>) -> Result<Value, String> {
    match args.into_iter().next() {
        Some(Value::String(path)) => match std::fs::read_dir(path.as_str()) {
            Ok(entries) => {
                let mut names = Vec::new();
                for entry in entries.flatten() {
                    names.push(Value::String(ZyStr::new(
                        entry.file_name().to_string_lossy().into_owned(),
                    )));
                }
                Ok(Value::Array(Rc::new(names)))
            }
            Err(e) => Ok(io_err(e)),
        },
        _ => Err("io::list: expected String path".into()),
    }
}

fn io_mkdir(args: Vec<Value>) -> Result<Value, String> {
    match args.into_iter().next() {
        Some(Value::String(path)) => match std::fs::create_dir_all(path.as_str()) {
            Ok(_) => Ok(Value::Unit),
            Err(e) => Ok(io_err(e)),
        },
        _ => Err("io::mkdir: expected String path".into()),
    }
}

// ── std/net ──────────────────────────────────────────────────────────────────
//
// Mirrors zymbol-interpreter/src/stdlib/net.rs. Network failures are returned as
// soft `Value::Error("##Network(...)")` (catchable); a wrong argument type is a hard `Err`.
// The request itself is the host's (`Host::http`); a host without a network
// answers with a hard error, which a compiler without `http` never lets a
// program reach, since it refuses `<# std/net`.

fn net_err(msg: impl Into<String>) -> Value {
    Value::Error(ZyStr::new(format!("##Network({})", msg.into())))
}

/// Parse the optional headers arg: Array of 2-element (String, String) tuples.
fn parse_headers(arg: Option<Value>, fname: &str) -> Result<Vec<(String, String)>, String> {
    let value = match arg {
        None => return Ok(Vec::new()),
        Some(v) => v,
    };
    let bad = || format!("{}: headers must be an Array of (String, String) tuples", fname);
    match value {
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items.iter() {
                match item {
                    Value::Tuple(pair) if pair.len() == 2 => match (&pair[0], &pair[1]) {
                        (Value::String(k), Value::String(v)) => {
                            out.push((k.as_str().to_string(), v.as_str().to_string()))
                        }
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

/// The body as a String, a network failure as a soft error, no network at all
/// as a hard one.
fn response_to_value(resp: Result<String, HttpError>) -> Result<Value, String> {
    match resp {
        Ok(body) => Ok(Value::String(ZyStr::new(body))),
        Err(HttpError::Network(e)) => Ok(net_err(e)),
        Err(HttpError::Unavailable(e)) => Err(e),
    }
}

fn net_get(args: Vec<Value>, host: &mut dyn Host) -> Result<Value, String> {
    let mut it = args.into_iter();
    let url = match it.next() {
        Some(Value::String(u)) => u,
        _ => return Err("net::get: expected String url".into()),
    };
    let headers = parse_headers(it.next(), "net::get")?;
    response_to_value(host.http(HttpRequest {
        method: HttpMethod::Get, url: url.as_str(), headers: &headers, body: None,
    }))
}

fn net_post_as(args: Vec<Value>, host: &mut dyn Host, fname: &str, content_type: &str) -> Result<Value, String> {
    let mut it = args.into_iter();
    match (it.next(), it.next()) {
        (Some(Value::String(url)), Some(Value::String(body))) => {
            let headers = parse_headers(it.next(), fname)?;
            response_to_value(host.http(HttpRequest {
                method: HttpMethod::Post, url: url.as_str(), headers: &headers,
                body: Some((content_type, body.as_str())),
            }))
        }
        _ => Err(format!("{}: expected (String, String)", fname)),
    }
}

fn net_head(args: Vec<Value>, host: &mut dyn Host) -> Result<Value, String> {
    match args.into_iter().next() {
        Some(Value::String(url)) => match host.http_head(url.as_str()) {
            Ok(b) => Ok(Value::Bool(b)),
            Err(HttpError::Network(_)) => Ok(Value::Bool(false)),
            Err(HttpError::Unavailable(e)) => Err(e),
        },
        _ => Err("net::head: expected String url".into()),
    }
}

fn net_dispatch(builtin_id: u16, args: Vec<Value>, host: &mut dyn Host) -> Result<Value, String> {
    match builtin_id {
        B::NET_GET       => net_get(args, host),
        B::NET_POST      => net_post_as(args, host, "net::post", "text/plain"),
        B::NET_POST_JSON => net_post_as(args, host, "net::post_json", "application/json"),
        B::NET_HEAD      => net_head(args, host),
        other => Err(format!("unknown net builtin id {}", other)),
    }
}

// ── std/time ──────────────────────────────────────────────────────────────────
//
// Adapter only. The clock, the civil calendar and every rule about what a month
// is live in `zymbol_intrinsics::time`, shared with the tree-walker: a table of
// names can be kept in step between two engines by reading them side by side,
// and a leap year cannot. What differs here is unboxing, and nothing else.
//
// A soft error is `##Time(...)`, catchable like `##IO`; a wrong argument type
// is `Err`, which stops the program, because it is the caller's bug.

use zymbol_intrinsics::time as zt;

fn time_soft(message: String) -> Value {
    Value::Error(ZyStr::new(format!("##Time({})", message)))
}

fn time_int(v: Option<&Value>) -> Option<i64> {
    match v {
        Some(Value::Int(i)) => Some(*i),
        _ => None,
    }
}

fn time_text(v: Option<&Value>) -> Option<&str> {
    match v {
        Some(Value::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// The optional trailing zone: absent, or present and text. `Err(())` is a
/// zone argument that is not a String, which is a call the caller got wrong.
fn time_zone(args: &[Value], from: usize) -> Result<Option<&str>, ()> {
    match args.get(from) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(()),
    }
}

fn time_now(_args: Vec<Value>, host: &mut dyn Host) -> Result<Value, String> {
    Ok(Value::Int(host.clock().now_ms()))
}

fn time_today(args: Vec<Value>, host: &mut dyn Host) -> Result<Value, String> {
    let zone = time_zone(&args, 0).map_err(|_| "time::today: expected an optional zone String".to_string())?;
    Ok(match zt::call_today_with(host.clock(), zone) {
        Ok(s) => Value::String(ZyStr::new(s)),
        Err(e) => time_soft(e),
    })
}

fn time_parts(args: Vec<Value>, host: &mut dyn Host) -> Result<Value, String> {
    let expected = "time::parts: expected (### epoch [, zone])";
    let epoch = time_int(args.first()).ok_or_else(|| expected.to_string())?;
    let zone = time_zone(&args, 1).map_err(|_| expected.to_string())?;
    Ok(match zt::call_parts_with(host.clock(), epoch, zone) {
        Ok(p) => Value::NamedTuple(Rc::new(
            zt::parts_fields(&p)
                .into_iter()
                .map(|(k, v)| (k.to_string(), Value::Int(v)))
                .collect(),
        )),
        Err(e) => time_soft(e),
    })
}

fn time_of(args: Vec<Value>, host: &mut dyn Host) -> Result<Value, String> {
    let expected = "time::of: expected (year, month, day) or (year, month, day, hour, minute, second), each ###, plus an optional zone";
    // The zone is text and every field is a number, so a trailing String is the
    // zone and nothing else can be — no counting of arguments decides it.
    let cut = args.len().saturating_sub(1);
    let (fields, zone) = match args.last() {
        Some(Value::String(s)) => (&args[..cut], Some(s.as_str())),
        _ => (&args[..], None),
    };
    let mut numbers = Vec::with_capacity(fields.len());
    for v in fields {
        match v {
            Value::Int(i) => numbers.push(*i),
            _ => return Err(expected.to_string()),
        }
    }
    Ok(match zt::call_of_with(host.clock(), &numbers, zone) {
        Ok(ms) => Value::Int(ms),
        Err(e) => time_soft(e),
    })
}

fn time_format(args: Vec<Value>, host: &mut dyn Host) -> Result<Value, String> {
    let expected = "time::format: expected (### epoch, \"pattern\" [, zone])";
    let epoch = time_int(args.first()).ok_or_else(|| expected.to_string())?;
    let pattern = time_text(args.get(1)).ok_or_else(|| expected.to_string())?;
    let zone = time_zone(&args, 2).map_err(|_| expected.to_string())?;
    Ok(match zt::call_format_with(host.clock(), epoch, pattern, zone) {
        Ok(s) => Value::String(ZyStr::new(s)),
        Err(e) => time_soft(e),
    })
}

fn time_add(args: Vec<Value>, host: &mut dyn Host) -> Result<Value, String> {
    let expected = "time::add: expected (### epoch, ### count, \"unit\" [, zone])";
    let epoch = time_int(args.first()).ok_or_else(|| expected.to_string())?;
    let count = time_int(args.get(1)).ok_or_else(|| expected.to_string())?;
    let unit = time_text(args.get(2)).ok_or_else(|| expected.to_string())?;
    let zone = time_zone(&args, 3).map_err(|_| expected.to_string())?;
    Ok(match zt::call_add_with(host.clock(), epoch, count, unit, zone) {
        Ok(ms) => Value::Int(ms),
        Err(e) => time_soft(e),
    })
}

fn time_diff(args: Vec<Value>, host: &mut dyn Host) -> Result<Value, String> {
    let expected = "time::diff: expected (### a, ### b, \"unit\" [, zone])";
    let a = time_int(args.first()).ok_or_else(|| expected.to_string())?;
    let b = time_int(args.get(1)).ok_or_else(|| expected.to_string())?;
    let unit = time_text(args.get(2)).ok_or_else(|| expected.to_string())?;
    let zone = time_zone(&args, 3).map_err(|_| expected.to_string())?;
    Ok(match zt::call_diff_with(host.clock(), a, b, unit, zone) {
        Ok(n) => Value::Int(n),
        Err(e) => time_soft(e),
    })
}

// ── std/term ───────────────────────────────────────────────────────────────────
//
// Terminal display metrics. Mirrors zymbol-interpreter/src/stdlib/term.rs.
// Width is measured in terminal columns over grapheme clusters, which is not the
// same as grapheme count (`$#`): CJK and most emoji take two columns each.

use unicode_segmentation::UnicodeSegmentation as _;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

fn term_pad(s: &str, cols: i64, on_left: bool) -> String {
    let deficit = cols - UnicodeWidthStr::width(s) as i64;
    if deficit <= 0 {
        return s.to_string();
    }
    let spaces = " ".repeat(deficit as usize);
    if on_left { format!("{spaces}{s}") } else { format!("{s}{spaces}") }
}

fn term_center_str(s: &str, cols: i64) -> String {
    let deficit = cols - UnicodeWidthStr::width(s) as i64;
    if deficit <= 0 {
        return s.to_string();
    }
    let left = deficit / 2;
    let right = deficit - left;
    format!("{}{s}{}", " ".repeat(left as usize), " ".repeat(right as usize))
}

fn term_truncate_str(s: &str, cols: i64) -> String {
    if UnicodeWidthStr::width(s) as i64 <= cols {
        return s.to_string();
    }
    let mut used = 0i64;
    let mut out = String::new();
    for g in s.graphemes(true) {
        let w = UnicodeWidthStr::width(g) as i64;
        if used + w > cols {
            break;
        }
        out.push_str(g);
        used += w;
    }
    out
}

fn term_width(args: Vec<Value>) -> Result<Value, String> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::Int(UnicodeWidthStr::width(s.as_str()) as i64)),
        Some(Value::Char(c))   => Ok(Value::Int(UnicodeWidthChar::width(*c).unwrap_or(0) as i64)),
        other => Err(format!(
            "term::width: expected a String or Char, got {}",
            other.map(|v| v.type_label()).unwrap_or_else(|| "nothing".to_string())
        )),
    }
}

fn term_pad_left(args: Vec<Value>) -> Result<Value, String> {
    match (args.first(), args.get(1)) {
        (Some(Value::String(s)), Some(Value::Int(n))) => Ok(Value::String(ZyStr::new(term_pad(s.as_str(), *n, true)))),
        _ => Err("term::pad_left: expected (String, ###)".into()),
    }
}

fn term_pad_right(args: Vec<Value>) -> Result<Value, String> {
    match (args.first(), args.get(1)) {
        (Some(Value::String(s)), Some(Value::Int(n))) => Ok(Value::String(ZyStr::new(term_pad(s.as_str(), *n, false)))),
        _ => Err("term::pad_right: expected (String, ###)".into()),
    }
}

fn term_center(args: Vec<Value>) -> Result<Value, String> {
    match (args.first(), args.get(1)) {
        (Some(Value::String(s)), Some(Value::Int(n))) => Ok(Value::String(ZyStr::new(term_center_str(s.as_str(), *n)))),
        _ => Err("term::center: expected (String, ###)".into()),
    }
}

fn term_truncate(args: Vec<Value>) -> Result<Value, String> {
    match (args.first(), args.get(1)) {
        (Some(Value::String(s)), Some(Value::Int(n))) => Ok(Value::String(ZyStr::new(term_truncate_str(s.as_str(), *n)))),
        _ => Err("term::truncate: expected (String, ###)".into()),
    }
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub fn call(builtin_id: u16, args: Vec<Value>, host: &mut dyn Host) -> Result<Value, String> {
    match builtin_id {
        B::SQRT    => math_sqrt(args),
        B::EXP     => math_exp(args),
        B::LN      => math_ln(args),
        B::LOG     => math_log(args),
        B::POW     => math_pow(args),
        B::SIN     => math_sin(args),
        B::COS     => math_cos(args),
        B::TAN     => math_tan(args),
        B::ASIN    => math_asin(args),
        B::ACOS    => math_acos(args),
        B::ATAN    => math_atan(args),
        B::ATAN2   => math_atan2(args),
        B::TANH    => math_tanh(args),
        B::SINH    => math_sinh(args),
        B::COSH    => math_cosh(args),
        B::SIGMOID => math_sigmoid(args),
        B::ABS     => math_abs(args),
        B::MAX     => math_max(args),
        B::MIN     => math_min(args),
        B::FLOOR   => math_floor(args),
        B::CEIL    => math_ceil(args),
        B::ROUND   => math_round(args),
        B::RAND_ENTERO   => rand_entero(args, host),
        B::RAND_RANGO    => rand_rango(args, host),
        B::RAND_PESO_F64 => rand_peso_f64(args, host),
        B::JSON_DECODE   => json_decode(args),
        B::JSON_DECODE_MAP => json_decode_map(args),
        B::JSON_ENCODE   => json_encode(args),
        B::IO_READ       => io_read(args),
        B::IO_WRITE      => io_write(args),
        B::IO_APPEND     => io_append(args),
        B::IO_EXISTS     => io_exists(args),
        B::IO_DELETE     => io_delete(args),
        B::IO_LIST       => io_list(args),
        B::IO_MKDIR      => io_mkdir(args),
        B::TERM_WIDTH     => term_width(args),
        B::TERM_PAD_LEFT  => term_pad_left(args),
        B::TERM_PAD_RIGHT => term_pad_right(args),
        B::TERM_CENTER    => term_center(args),
        B::TERM_TRUNCATE  => term_truncate(args),
        B::TIME_NOW       => time_now(args, host),
        B::TIME_TODAY     => time_today(args, host),
        B::TIME_PARTS     => time_parts(args, host),
        B::TIME_OF        => time_of(args, host),
        B::TIME_FORMAT    => time_format(args, host),
        B::TIME_ADD       => time_add(args, host),
        B::TIME_DIFF      => time_diff(args, host),
        id if (B::NET_GET..=B::NET_HEAD).contains(&id) => net_dispatch(id, args, host),
        id if (B::DB_CONNECT..=B::DB_TABLE_EXISTS).contains(&id) => host.db(id, args),
        other => Err(format!("unknown builtin id {}", other)),
    }
}

