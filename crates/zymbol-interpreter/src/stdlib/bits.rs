//! std/bits — the bits of an integer, and words of several bytes (D-28).
//!
//! The rules and every message live in `zymbol_intrinsics::bits`, shared with
//! the VM; this only takes the arguments out of their `Value` and puts the
//! answer back. A message carries no kind of its own: its words choose the
//! family, as `zymbol_common::errkind` reads them.

use crate::{FunctionDef, Result, RuntimeError, Value};
use std::collections::HashMap;
use std::rc::Rc;
use zymbol_intrinsics::bits::{self, Arg, Out};
use zymbol_span::Span;

fn call(f: &str, args: Vec<Value>, span: Span) -> Result<Value> {
    let held: Vec<Arg> = args
        .iter()
        .map(|v| match v {
            Value::Int(n) => Arg::Int(*n),
            Value::Char(c) => Arg::Char(*c),
            Value::String(s) => Arg::Text(s.as_str()),
            other => Arg::Other(other.type_label()),
        })
        .collect();
    match bits::call(f, &held) {
        Ok(Out::Int(n)) => Ok(Value::Int(n)),
        Ok(Out::Bytes(s)) => Ok(Value::String(s.into())),
        Err(message) => Err(RuntimeError::Generic { message, span }),
    }
}

macro_rules! natives {
    ($($name:literal => $fn:ident),* $(,)?) => {
        $(fn $fn(args: Vec<Value>, span: Span) -> Result<Value> { call($name, args, span) })*
        pub(crate) fn register() -> HashMap<String, Rc<FunctionDef>> {
            let mut m: HashMap<String, Rc<FunctionDef>> = HashMap::new();
            $(
                let arity = bits::FUNCTIONS.iter().find(|(n, _)| *n == $name).map(|(_, a)| *a as i8).unwrap_or(1);
                m.insert($name.into(), Rc::new(FunctionDef::Native { name: $name, arity, func: $fn }));
            )*
            m
        }
    };
}

natives! {
    "and" => bits_and,
    "or" => bits_or,
    "xor" => bits_xor,
    "shl" => bits_shl,
    "shr" => bits_shr,
    "not" => bits_not,
    "to_le" => bits_to_le,
    "to_be" => bits_to_be,
    "from_le" => bits_from_le,
    "from_be" => bits_from_be,
    "signed" => bits_signed,
}
