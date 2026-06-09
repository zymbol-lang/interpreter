//! Native stdlib function dispatch for the register VM.
//!
//! Mirrors the implementations in zymbol-interpreter/src/stdlib/, but without
//! the interpreter dependency. Called from CallBuiltin instructions.

use std::rc::Rc;

use zymbol_bytecode::builtins as B;

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
    let types: Vec<&str> = args.iter().map(|v| v.zymbol_type_name()).collect();
    format!("mat::{}: incompatible argument type(s) {:?}", fname, types)
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

fn xoshiro_next() -> u64 {
    STATE.with(|s| {
        let mut st = s.get();
        if !SEEDED.with(|b| b.get()) {
            let ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(12345) as u64;
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

fn rand_entero(args: Vec<Value>) -> Result<Value, String> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(lo)), Some(Value::Int(hi))) if hi >= lo => {
            let range = (hi - lo + 1) as u64;
            Ok(Value::Int(*lo + (xoshiro_next() % range) as i64))
        }
        _ => Err("random::entero: expected (###, ###) with max >= min".into()),
    }
}

fn rand_rango(args: Vec<Value>) -> Result<Value, String> {
    match args.first() {
        Some(Value::Int(n)) if *n > 0 => {
            Ok(Value::Int((xoshiro_next() % (*n as u64)) as i64))
        }
        _ => Err("random::rango: expected positive ###".into()),
    }
}

fn rand_peso_f64(_args: Vec<Value>) -> Result<Value, String> {
    Ok(Value::Float(((xoshiro_next() % 201) as f64 - 100.0) / 1000.0))
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
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else {
                Value::Float(n.as_f64().unwrap_or(f64::NAN))
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

fn response_to_value(resp: Result<ureq::Response, ureq::Error>) -> Value {
    match resp {
        Ok(resp) => match resp.into_string() {
            Ok(body) => Value::String(ZyStr::new(body)),
            Err(e) => net_err(e.to_string()),
        },
        Err(e) => net_err(e.to_string()),
    }
}

fn net_get(args: Vec<Value>) -> Result<Value, String> {
    let mut it = args.into_iter();
    let url = match it.next() {
        Some(Value::String(u)) => u,
        _ => return Err("net::get: expected String url".into()),
    };
    let headers = parse_headers(it.next(), "net::get")?;
    let mut req = ureq::get(url.as_str());
    for (k, v) in &headers {
        req = req.set(k, v);
    }
    Ok(response_to_value(req.call()))
}

fn net_post(args: Vec<Value>) -> Result<Value, String> {
    let mut it = args.into_iter();
    match (it.next(), it.next()) {
        (Some(Value::String(url)), Some(Value::String(body))) => {
            let headers = parse_headers(it.next(), "net::post")?;
            let mut req = ureq::post(url.as_str()).set("Content-Type", "text/plain");
            for (k, v) in &headers {
                req = req.set(k, v);
            }
            Ok(response_to_value(req.send_string(body.as_str())))
        }
        _ => Err("net::post: expected (String, String)".into()),
    }
}

fn net_post_json(args: Vec<Value>) -> Result<Value, String> {
    let mut it = args.into_iter();
    match (it.next(), it.next()) {
        (Some(Value::String(url)), Some(Value::String(body))) => {
            let headers = parse_headers(it.next(), "net::post_json")?;
            let mut req = ureq::post(url.as_str()).set("Content-Type", "application/json");
            for (k, v) in &headers {
                req = req.set(k, v);
            }
            Ok(response_to_value(req.send_string(body.as_str())))
        }
        _ => Err("net::post_json: expected (String, String)".into()),
    }
}

fn net_head(args: Vec<Value>) -> Result<Value, String> {
    match args.into_iter().next() {
        Some(Value::String(url)) => Ok(Value::Bool(ureq::head(url.as_str()).call().is_ok())),
        _ => Err("net::head: expected String url".into()),
    }
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub fn call(builtin_id: u16, args: Vec<Value>) -> Result<Value, String> {
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
        B::RAND_ENTERO   => rand_entero(args),
        B::RAND_RANGO    => rand_rango(args),
        B::RAND_PESO_F64 => rand_peso_f64(args),
        B::JSON_DECODE   => json_decode(args),
        B::JSON_ENCODE   => json_encode(args),
        B::IO_READ       => io_read(args),
        B::IO_WRITE      => io_write(args),
        B::IO_APPEND     => io_append(args),
        B::IO_EXISTS     => io_exists(args),
        B::IO_DELETE     => io_delete(args),
        B::IO_LIST       => io_list(args),
        B::IO_MKDIR      => io_mkdir(args),
        B::NET_GET       => net_get(args),
        B::NET_POST      => net_post(args),
        B::NET_POST_JSON => net_post_json(args),
        B::NET_HEAD      => net_head(args),
        other => Err(format!("unknown builtin id {}", other)),
    }
}
