//! std/random — pseudo-random number generation for Zymbol-Lang.
//!
//! The generator is `zymbol_intrinsics::random` (xoshiro256++), shared with
//! the VM so that `semilla(n)` gives one sequence in both (G8). Without a seed
//! the first draw takes one from the clock.

use crate::{FunctionDef, Result, RuntimeError, Value};
use std::collections::HashMap;
use std::rc::Rc;
use zymbol_intrinsics::random as rng;
use zymbol_span::Span;

fn clock() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345) as u64
}

// --- Native functions --------------------------------------------------------

fn random_entero(args: Vec<Value>, span: Span) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(lo)), Some(Value::Int(hi))) if hi >= lo => {
            Ok(Value::Int(rng::entero(*lo, *hi, clock)))
        }
        _ => Err(RuntimeError::Generic {
            message: "random::entero: expected (###, ###) with max >= min".into(),
            span,
        }),
    }
}

fn random_rango(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) if *n > 0 => Ok(Value::Int(rng::rango(*n, clock))),
        _ => Err(RuntimeError::Generic {
            message: "random::rango: expected positive ###".into(),
            span,
        }),
    }
}

fn random_peso_f64(args: Vec<Value>, span: Span) -> Result<Value> {
    let _ = (args, span);
    // Returns Float in [-0.1, 0.1] for neural-network weight initialization
    Ok(Value::Float(rng::peso_f64(clock)))
}

fn random_semilla(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) => {
            rng::seed(*n);
            Ok(Value::Unit)
        }
        _ => Err(RuntimeError::Generic {
            message: "random::semilla: expected ###".into(),
            span,
        }),
    }
}

// --- Registry ----------------------------------------------------------------

pub(crate) fn register() -> HashMap<String, Rc<FunctionDef>> {
    let mut m: HashMap<String, Rc<FunctionDef>> = HashMap::new();

    macro_rules! native {
        ($name:literal, $arity:expr, $fn:expr) => {
            m.insert($name.into(), Rc::new(FunctionDef::Native {
                name: $name, arity: $arity, func: $fn,
            }));
        };
    }

    native!("entero",   2, random_entero);
    native!("rango",    1, random_rango);
    native!("peso_f64", 0, random_peso_f64);
    native!("semilla",  1, random_semilla);

    m
}
