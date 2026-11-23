//! std/text — upper case, lower case and case folding (D-18).
//!
//! The rules live in `zymbol_intrinsics::text`, shared with the VM; this only
//! takes the argument out of its `Value` and puts the answer back.
//!
//! - `upper(s)` and `lower(s)` take a String or a Char and give a String: the
//!   upper case of `'ß'` is `"SS"`, two characters.
//! - `graphemes(s)` takes a String or a Char and gives an Array of Strings, one
//!   extended grapheme cluster each (D-19).
//! - `fold(s)` gives back what it was given — a String for a String, a Char for
//!   a Char — because it maps one code point to one.
//!
//! Names are the international (English) forms; a project wanting localized
//! names re-exports them through the i18n three-layer pattern (I18N.md).

use crate::{FunctionDef, Result, RuntimeError, Value};
use std::collections::HashMap;
use std::rc::Rc;
use zymbol_intrinsics::text;
use zymbol_span::Span;

/// The message of an argument that is not text. It names the type, never the
/// value, and says "type", so the error is a `##Type` (GLB-033, GLB-010).
pub(crate) fn wrong_type(fname: &str, got: &str) -> String {
    format!("text::{fname}: argument type must be String or Char, got {got}")
}

fn err(fname: &str, args: &[Value], span: Span) -> RuntimeError {
    let got = args.first().map(|v| v.type_label()).unwrap_or_else(|| "nothing".to_string());
    RuntimeError::Generic { message: wrong_type(fname, &got), span }
}

fn text_upper(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::String(text::upper(s).into())),
        Some(Value::Char(c)) => Ok(Value::String(text::upper_char(*c).into())),
        _ => Err(err("upper", &args, span)),
    }
}

fn text_lower(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::String(text::lower(s).into())),
        Some(Value::Char(c)) => Ok(Value::String(text::lower_char(*c).into())),
        _ => Err(err("lower", &args, span)),
    }
}

/// `graphemes(s)`: an Array of Strings, one cluster each (D-19).
fn text_graphemes(args: Vec<Value>, span: Span) -> Result<Value> {
    let parts: Vec<Value> = match args.first() {
        Some(Value::String(s)) => text::graphemes(s).into_iter().map(|g| Value::String(g.into())).collect(),
        Some(Value::Char(c)) => vec![Value::String(c.to_string().into())],
        _ => return Err(err("graphemes", &args, span)),
    };
    Ok(Value::array(parts))
}

fn text_fold(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::String(text::fold(s).into())),
        Some(Value::Char(c)) => Ok(Value::Char(text::fold_char(*c))),
        _ => Err(err("fold", &args, span)),
    }
}

pub(crate) fn register() -> HashMap<String, Rc<FunctionDef>> {
    let mut m: HashMap<String, Rc<FunctionDef>> = HashMap::new();
    macro_rules! native {
        ($name:literal, $arity:expr, $fn:expr) => {
            m.insert($name.into(), Rc::new(FunctionDef::Native {
                name: $name, arity: $arity, func: $fn,
            }));
        };
    }
    native!("upper", 1, text_upper);
    native!("lower", 1, text_lower);
    native!("fold",  1, text_fold);
    native!("graphemes", 1, text_graphemes);
    m
}
