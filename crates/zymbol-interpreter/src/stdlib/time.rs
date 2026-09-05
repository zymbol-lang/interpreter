//! std/time — the clock and the civil calendar for Zymbol-Lang.
//!
//! Until v0.0.9 the only way to learn the date was to leave the language:
//! `<\ "date +%F" \>`. That failed on Windows, failed in the browser, needed
//! `#09#` forced before every call because otherwise the shell's answer came
//! back in whatever script the numeral mode had selected, and gave no
//! arithmetic at all — "the last thirty days" cannot be asked of a string
//! (GAP-ZYB-002).
//!
//!   now()                                → Int, milliseconds since the epoch
//!   today([zone])                        → String "YYYY-MM-DD"
//!   parts(epoch [, zone])                → dictionary of civil fields
//!   of(y, m, d [, h, mi, s] [, zone])    → Int
//!   format(epoch, pattern [, zone])      → String
//!   add(epoch, count, unit [, zone])     → Int
//!   diff(a, b, unit [, zone])            → Int
//!
//! The zone is the last argument everywhere and always optional, defaulting to
//! `"UTC"`; `"local"` and a fixed `±HHMM` such as `"-0400"` are the other two.
//!
//! Everything this file does is unbox a `Value` into a primitive, hand it to
//! `zymbol_intrinsics::time`, and box the answer. The calendar lives there
//! because the register VM needs the identical one and `std/term` — duplicated
//! between the two engines and kept in step by inspection — showed how far that
//! goes: a padding rule can be read side by side, a leap year cannot.
//!
//! A wrong argument *type* is a programmer error and aborts with a hard
//! `RuntimeError`, as everywhere else in `std/`. A wrong *value* — month 13, an
//! unknown zone, `%Q` — is data, and comes back as a soft `##Time` so a program
//! reading dates from outside can catch it.
//!
//! For localized names, use the i18n three-layer pattern to re-export under the
//! target language's names (Spanish: ahora, hoy, partes, de, formato, …).

use crate::{ErrorValue, FunctionDef, Result, RuntimeError, Value};
use std::collections::HashMap;
use std::rc::Rc;
use zymbol_intrinsics::time as t;
use zymbol_span::Span;

/// A soft `##Time`: the call was well formed and the data was not.
fn soft(message: String) -> Value {
    Value::Error(ErrorValue::new("Time", message))
}

/// A hard error: this call could not be made at all.
fn hard(fname: &str, expected: &str, span: Span) -> RuntimeError {
    RuntimeError::Generic {
        message: format!("time::{fname}: expected {expected}"),
        span,
    }
}

fn as_int(v: Option<&Value>) -> Option<i64> {
    match v {
        Some(Value::Int(i)) => Some(*i),
        _ => None,
    }
}

fn as_text(v: Option<&Value>) -> Option<&str> {
    match v {
        Some(Value::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// The optional trailing zone: present and a String, or absent.
///
/// `Err(())` means it was there and was not text, which is a call the caller
/// got wrong rather than data that failed.
fn trailing_zone(args: &[Value], from: usize) -> std::result::Result<Option<&str>, ()> {
    match args.get(from) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(()),
    }
}

/// The dictionary `parts` answers with. Field order is the intrinsics' to
/// decide, because it is a dictionary's iteration order and both engines have
/// to walk it the same way.
fn parts_value(p: &t::Parts) -> Value {
    Value::named_tuple(
        t::parts_fields(p)
            .into_iter()
            .map(|(k, v)| (k.to_string(), Value::Int(v)))
            .collect(),
    )
}

// --- Native functions --------------------------------------------------------

/// time::now() -> Int — milliseconds since 1970-01-01T00:00:00Z.
fn time_now(_args: Vec<Value>, _span: Span) -> Result<Value> {
    Ok(Value::Int(t::now_ms()))
}

/// time::today([zone]) -> String | Error
fn time_today(args: Vec<Value>, span: Span) -> Result<Value> {
    let zone = trailing_zone(&args, 0).map_err(|_| hard("today", "an optional zone String", span))?;
    Ok(match t::call_today(zone) {
        Ok(s) => Value::String(s),
        Err(e) => soft(e),
    })
}

/// time::parts(epoch [, zone]) -> Dictionary | Error
fn time_parts(args: Vec<Value>, span: Span) -> Result<Value> {
    let epoch = as_int(args.first())
        .ok_or_else(|| hard("parts", "(### epoch [, zone])", span))?;
    let zone = trailing_zone(&args, 1).map_err(|_| hard("parts", "(### epoch [, zone])", span))?;
    Ok(match t::call_parts(epoch, zone) {
        Ok(p) => parts_value(&p),
        Err(e) => soft(e),
    })
}

/// time::of(y, m, d [, h, mi, s] [, zone]) -> Int | Error
fn time_of(args: Vec<Value>, span: Span) -> Result<Value> {
    let expected = "(year, month, day) or (year, month, day, hour, minute, second), each ###, plus an optional zone";
    // The zone is text and every field is a number, so which is which never
    // needs counting: a trailing String is the zone and nothing else can be.
    let zone_at = args.len().saturating_sub(1);
    let (fields, zone) = match args.last() {
        Some(Value::String(s)) => (&args[..zone_at], Some(s.as_str())),
        _ => (&args[..], None),
    };
    let mut numbers = Vec::with_capacity(fields.len());
    for v in fields {
        match v {
            Value::Int(i) => numbers.push(*i),
            _ => return Err(hard("of", expected, span)),
        }
    }
    Ok(match t::call_of(&numbers, zone) {
        Ok(ms) => Value::Int(ms),
        Err(e) => soft(e),
    })
}

/// time::format(epoch, pattern [, zone]) -> String | Error
fn time_format(args: Vec<Value>, span: Span) -> Result<Value> {
    let expected = "(### epoch, \"pattern\" [, zone])";
    let epoch = as_int(args.first()).ok_or_else(|| hard("format", expected, span))?;
    let pattern = as_text(args.get(1)).ok_or_else(|| hard("format", expected, span))?;
    let zone = trailing_zone(&args, 2).map_err(|_| hard("format", expected, span))?;
    Ok(match t::call_format(epoch, pattern, zone) {
        Ok(s) => Value::String(s),
        Err(e) => soft(e),
    })
}

/// time::add(epoch, count, unit [, zone]) -> Int | Error
fn time_add(args: Vec<Value>, span: Span) -> Result<Value> {
    let expected = "(### epoch, ### count, \"unit\" [, zone])";
    let epoch = as_int(args.first()).ok_or_else(|| hard("add", expected, span))?;
    let count = as_int(args.get(1)).ok_or_else(|| hard("add", expected, span))?;
    let unit = as_text(args.get(2)).ok_or_else(|| hard("add", expected, span))?;
    let zone = trailing_zone(&args, 3).map_err(|_| hard("add", expected, span))?;
    Ok(match t::call_add(epoch, count, unit, zone) {
        Ok(ms) => Value::Int(ms),
        Err(e) => soft(e),
    })
}

/// time::diff(a, b, unit [, zone]) -> Int | Error
fn time_diff(args: Vec<Value>, span: Span) -> Result<Value> {
    let expected = "(### a, ### b, \"unit\" [, zone])";
    let a = as_int(args.first()).ok_or_else(|| hard("diff", expected, span))?;
    let b = as_int(args.get(1)).ok_or_else(|| hard("diff", expected, span))?;
    let unit = as_text(args.get(2)).ok_or_else(|| hard("diff", expected, span))?;
    let zone = trailing_zone(&args, 3).map_err(|_| hard("diff", expected, span))?;
    Ok(match t::call_diff(a, b, unit, zone) {
        Ok(n) => Value::Int(n),
        Err(e) => soft(e),
    })
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

    native!("now",    0, time_now);
    native!("today", -1, time_today);
    native!("parts", -1, time_parts);
    native!("of",    -1, time_of);
    native!("format", -1, time_format);
    native!("add",   -1, time_add);
    native!("diff",  -1, time_diff);

    m
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A span these tests never look at: the functions take one to point an
    /// error at, and none of them reads it.
    fn nowhere() -> Span {
        let p = zymbol_span::Position::start();
        Span::new(p, p, zymbol_span::FileId(0))
    }

    fn call(f: fn(Vec<Value>, Span) -> Result<Value>, args: Vec<Value>) -> Value {
        f(args, nowhere()).expect("no hard error")
    }

    fn s(text: &str) -> Value {
        Value::String(text.to_string())
    }

    #[test]
    fn of_and_format_round_trip_through_iso() {
        let e = call(time_of, vec![Value::Int(2026), Value::Int(8), Value::Int(23)]);
        assert_eq!(call(time_format, vec![e.clone(), s("%F")]), s("2026-08-23"));
        assert_eq!(call(time_format, vec![e, s("%T")]), s("00:00:00"));
    }

    #[test]
    fn a_trailing_string_is_the_zone_and_a_number_is_a_field() {
        let utc = call(time_of, vec![Value::Int(2026), Value::Int(8), Value::Int(23)]);
        let offset = call(
            time_of,
            vec![Value::Int(2026), Value::Int(8), Value::Int(23), s("-0400")],
        );
        // Midnight four hours west is four hours later as an instant.
        match (utc, offset) {
            (Value::Int(a), Value::Int(b)) => assert_eq!(b - a, 4 * 60 * 60 * 1000),
            other => panic!("expected two Ints, got {other:?}"),
        }
    }

    #[test]
    fn parts_is_a_dictionary_in_a_fixed_order() {
        let e = call(time_of, vec![Value::Int(2026), Value::Int(8), Value::Int(23)]);
        match call(time_parts, vec![e]) {
            Value::NamedTuple(fields) => {
                let names: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
                assert_eq!(
                    names,
                    ["year", "month", "day", "hour", "minute", "second", "millisecond", "weekday", "offset"]
                );
                assert_eq!(fields[0].1, Value::Int(2026));
                assert_eq!(fields[7].1, Value::Int(7)); // a Sunday
            }
            other => panic!("expected a dictionary, got {other:?}"),
        }
    }

    #[test]
    fn bad_data_is_soft_and_a_bad_type_is_hard() {
        // month 13 is data: catchable
        let bad = call(time_of, vec![Value::Int(2026), Value::Int(13), Value::Int(1)]);
        assert!(matches!(&bad, Value::Error(e) if e.error_type == "Time"));
        // an unknown unit and an unknown zone likewise
        let e = call(time_of, vec![Value::Int(2026), Value::Int(8), Value::Int(23)]);
        assert!(matches!(
            call(time_add, vec![e.clone(), Value::Int(1), s("fortnight")]),
            Value::Error(_)
        ));
        assert!(matches!(call(time_today, vec![s("Mars")]), Value::Error(_)));
        assert!(matches!(call(time_format, vec![e, s("%Q")]), Value::Error(_)));
        // a String where the epoch goes is the caller's bug: hard
        assert!(time_parts(vec![s("hoy")], nowhere()).is_err());
        assert!(time_add(vec![Value::Int(0), s("uno"), s("day")], nowhere()).is_err());
    }

    #[test]
    fn now_is_an_instant_this_century() {
        match call(time_now, vec![]) {
            Value::Int(ms) => assert!(ms > 1_577_836_800_000 && ms < 4_102_444_800_000),
            other => panic!("expected an Int, got {other:?}"),
        }
    }
}
