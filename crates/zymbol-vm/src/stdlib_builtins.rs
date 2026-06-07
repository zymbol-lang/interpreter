//! Native stdlib function dispatch for the register VM.
//!
//! Mirrors the implementations in zymbol-interpreter/src/stdlib/, but without
//! the interpreter dependency. Called from CallBuiltin instructions.

use zymbol_bytecode::builtins as B;

use crate::Value;

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
        other => Err(format!("unknown builtin id {}", other)),
    }
}
