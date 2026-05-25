//! std/math — mathematical functions for Zymbol-Lang.
//!
//! All functions accept Int or Float arguments (Int is promoted to f64).
//! Return type: Float for transcendental functions; polymorphic for abs/max/min.
//!
//! Names follow the international standard (C/Python/Rust):
//!   sqrt, exp, ln, log, pow, sin, cos, tan, asin, acos, atan, atan2,
//!   tanh, sinh, cosh, sigmoid, abs, max, min, floor, ceil, round
//! For localized names, use the i18n three-layer pattern to re-export under
//! the target language's names (e.g. Spanish: raiz, sen, pot, piso, techo, redondear).

use crate::{FunctionDef, Result, RuntimeError, Value};
use std::collections::HashMap;
use std::rc::Rc;
use zymbol_span::Span;

#[inline]
fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Float(x) => Some(*x),
        Value::Int(x)   => Some(*x as f64),
        _               => None,
    }
}

fn type_err(fname: &str, args: &[Value], span: Span) -> RuntimeError {
    let types: Vec<&str> = args.iter().map(|v| v.type_name()).collect();
    RuntimeError::Generic {
        message: format!("mat::{}: incompatible argument type(s) {:?}", fname, types),
        span,
    }
}

// --- Unary (Float → Float) ------------------------------------------------

fn math_sqrt(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.sqrt())),
        None    => Err(type_err("sqrt", &args, span)),
    }
}

fn math_exp(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.exp())),
        None    => Err(type_err("exp", &args, span)),
    }
}

fn math_ln(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) if x > 0.0 => Ok(Value::Float(x.ln())),
        Some(_) => Err(RuntimeError::Generic {
            message: "mat::ln: argument must be positive".into(), span,
        }),
        None => Err(type_err("ln", &args, span)),
    }
}

fn math_sin(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.sin())),
        None    => Err(type_err("sin", &args, span)),
    }
}

fn math_cos(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.cos())),
        None    => Err(type_err("cos", &args, span)),
    }
}

fn math_tan(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.tan())),
        None    => Err(type_err("tan", &args, span)),
    }
}

fn math_asin(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) if (-1.0..=1.0).contains(&x) => Ok(Value::Float(x.asin())),
        Some(_) => Err(RuntimeError::Generic {
            message: "mat::asin: argument must be in [-1, 1]".into(), span,
        }),
        None => Err(type_err("asin", &args, span)),
    }
}

fn math_acos(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) if (-1.0..=1.0).contains(&x) => Ok(Value::Float(x.acos())),
        Some(_) => Err(RuntimeError::Generic {
            message: "mat::acos: argument must be in [-1, 1]".into(), span,
        }),
        None => Err(type_err("acos", &args, span)),
    }
}

fn math_atan(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.atan())),
        None    => Err(type_err("atan", &args, span)),
    }
}

fn math_atan2(args: Vec<Value>, span: Span) -> Result<Value> {
    match (args.first().and_then(as_f64), args.get(1).and_then(as_f64)) {
        (Some(y), Some(x)) => Ok(Value::Float(y.atan2(x))),
        _ => Err(type_err("atan2", &args, span)),
    }
}

fn math_tanh(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.tanh())),
        None    => Err(type_err("tanh", &args, span)),
    }
}

fn math_sinh(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.sinh())),
        None    => Err(type_err("sinh", &args, span)),
    }
}

fn math_cosh(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.cosh())),
        None    => Err(type_err("cosh", &args, span)),
    }
}

fn math_sigmoid(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(1.0 / (1.0 + (-x).exp()))),
        None    => Err(type_err("sigmoid", &args, span)),
    }
}

fn math_floor(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.floor())),
        None    => Err(type_err("floor", &args, span)),
    }
}

fn math_ceil(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.ceil())),
        None    => Err(type_err("ceil", &args, span)),
    }
}

fn math_round(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.round())),
        None    => Err(type_err("round", &args, span)),
    }
}

// --- abs: polymorphic (Int → Int, Float → Float) --------------------------

fn math_abs(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first() {
        Some(Value::Int(x))   => Ok(Value::Int(x.abs())),
        Some(Value::Float(x)) => Ok(Value::Float(x.abs())),
        _ => Err(type_err("abs", &args, span)),
    }
}

// --- Binary ---------------------------------------------------------------

fn math_log(args: Vec<Value>, span: Span) -> Result<Value> {
    // log(x) → natural log; log(x, base) → log in given base
    match (args.first().and_then(as_f64), args.get(1).and_then(as_f64)) {
        (Some(x), Some(base)) if x > 0.0 && base > 0.0 && base != 1.0 => {
            Ok(Value::Float(x.log(base)))
        }
        (Some(x), None) if x > 0.0 => Ok(Value::Float(x.ln())),
        (Some(_), Some(_)) => Err(RuntimeError::Generic {
            message: "mat::log: x and base must be positive; base ≠ 1".into(), span,
        }),
        _ => Err(type_err("log", &args, span)),
    }
}

fn math_pow(args: Vec<Value>, span: Span) -> Result<Value> {
    match (args.first().and_then(as_f64), args.get(1).and_then(as_f64)) {
        (Some(base), Some(exp)) => Ok(Value::Float(base.powf(exp))),
        _ => Err(type_err("pow", &args, span)),
    }
}

fn math_max(args: Vec<Value>, span: Span) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(a)), Some(Value::Int(b)))     => Ok(Value::Int(*a.max(b))),
        (Some(a), Some(b)) => match (as_f64(a), as_f64(b)) {
            (Some(fa), Some(fb)) => Ok(Value::Float(fa.max(fb))),
            _ => Err(type_err("max", &args, span)),
        },
        _ => Err(type_err("max", &args, span)),
    }
}

fn math_min(args: Vec<Value>, span: Span) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(a)), Some(Value::Int(b)))     => Ok(Value::Int(*a.min(b))),
        (Some(a), Some(b)) => match (as_f64(a), as_f64(b)) {
            (Some(fa), Some(fb)) => Ok(Value::Float(fa.min(fb))),
            _ => Err(type_err("min", &args, span)),
        },
        _ => Err(type_err("min", &args, span)),
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

    native!("sqrt",    1,  math_sqrt);
    native!("exp",     1,  math_exp);
    native!("ln",      1,  math_ln);
    native!("log",    -1,  math_log);
    native!("pow",     2,  math_pow);
    native!("sin",     1,  math_sin);
    native!("cos",     1,  math_cos);
    native!("tan",     1,  math_tan);
    native!("asin",    1,  math_asin);
    native!("acos",    1,  math_acos);
    native!("atan",    1,  math_atan);
    native!("atan2",   2,  math_atan2);
    native!("tanh",    1,  math_tanh);
    native!("sinh",    1,  math_sinh);
    native!("cosh",    1,  math_cosh);
    native!("sigmoid", 1,  math_sigmoid);
    native!("abs",     1,  math_abs);
    native!("max",     2,  math_max);
    native!("min",     2,  math_min);
    native!("floor",   1,  math_floor);
    native!("ceil",    1,  math_ceil);
    native!("round",   1,  math_round);

    m
}
