//! std/random — pseudo-random number generation for Zymbol-Lang.
//!
//! Uses xoshiro256++ seeded from system time on first call.
//! State lives in a thread-local cell: no seed object from Zymbol's perspective.

use crate::{FunctionDef, Result, RuntimeError, Value};
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use zymbol_span::Span;

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
        st[2] ^= st[0];
        st[3] ^= st[1];
        st[1] ^= st[2];
        st[0] ^= st[3];
        st[2] ^= t;
        st[3] = st[3].rotate_left(45);
        s.set(st);
        result
    })
}

// --- Native functions --------------------------------------------------------

fn random_entero(args: Vec<Value>, span: Span) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(lo)), Some(Value::Int(hi))) if hi >= lo => {
            let range = (hi - lo + 1) as u64;
            Ok(Value::Int(*lo + (xoshiro_next() % range) as i64))
        }
        _ => Err(RuntimeError::Generic {
            message: "random::entero: expected (###, ###) with max >= min".into(),
            span,
        }),
    }
}

fn random_rango(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first() {
        Some(Value::Int(n)) if *n > 0 => {
            Ok(Value::Int((xoshiro_next() % (*n as u64)) as i64))
        }
        _ => Err(RuntimeError::Generic {
            message: "random::rango: expected positive ###".into(),
            span,
        }),
    }
}

fn random_peso_f64(args: Vec<Value>, span: Span) -> Result<Value> {
    let _ = (args, span);
    // Returns Float in [-0.1, 0.1] for neural-network weight initialization
    let val = ((xoshiro_next() % 201) as f64 - 100.0) / 1000.0;
    Ok(Value::Float(val))
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

    m
}
