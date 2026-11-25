//! std/bits — operations on the bits of an integer, and words of several bytes
//! (D-28, decided by the author 2026-11-25; BYTES.md B-4 and B-5).
//!
//! An integer of Zymbol has no width: it is ±(2⁵³ − 1), and an overflow is an
//! error. So the bits are those of a NON-NEGATIVE integer, from 0 to
//! 9 007 199 254 740 991, and an operation that needs a width — `not`,
//! `signed` — takes one. A Char is its code, as `#|c|` reads it (D-25), so
//! `and(0xA5, 0x0F)` takes the two byte literals as they are written.
//!
//! - `and(a, b)`, `or(a, b)`, `xor(a, b)`: bit by bit.
//! - `shl(n, k)`, `shr(n, k)`: shifted by `k` places, `k` from 0. A `shl` past
//!   the range is an overflow, as `*` is.
//! - `not(n, w)`: the `w` low bits of `n` inverted, `w` from 1 to 53.
//! - `to_le(n, k)`, `to_be(n, k)`: `n` in `k` bytes, `k` from 1 to 6, little
//!   or big end first — a String of `k` Chars from `0x00` to `0xFF`, the form a
//!   buffer takes (D-22).
//! - `from_le(s)`, `from_be(s)`: the number those bytes write, 1 to 6 of them.
//! - `signed(n, w)`: `n`, a `w`-bit word, read in two's complement.
//!
//! Six bytes is the widest word a reader can hand back exactly: seven would
//! name numbers past 2⁵³. Python's `int.to_bytes` and `int.from_bytes` are the
//! oracle (feedback: Python is the oracle).
//!
//! Every message is written here once, and its words choose the family
//! (`zymbol_common::errkind`): "out of range" and "overflow" are `##Range`,
//! "type" is `##Type`. zyjs words them the same way in `bits.js`.

/// `zymbol_common::num::ZY_INT_MAX`: 2⁵³ − 1. This crate depends on nothing of
/// the engines', so the bound is written here, and a test holds the two equal.
pub const ZY_INT_MAX: i64 = 9_007_199_254_740_991;

/// What a function of std/bits was handed, as the engine holds it.
pub enum Arg<'a> {
    Int(i64),
    Char(char),
    Text(&'a str),
    /// Anything else, by the label `type_label` gives it.
    Other(String),
}

/// What it gives back.
pub enum Out {
    Int(i64),
    /// A word of bytes: one Char `0x00`–`0xFF` each.
    Bytes(String),
}

/// The functions and their arities, for the engines' registries.
pub const FUNCTIONS: &[(&str, usize)] = &[
    ("and", 2),
    ("or", 2),
    ("xor", 2),
    ("shl", 2),
    ("shr", 2),
    ("not", 2),
    ("to_le", 2),
    ("to_be", 2),
    ("from_le", 1),
    ("from_be", 1),
    ("signed", 2),
];

/// Widest word, in bytes, `to_le`/`from_le` take.
pub const MAX_BYTES: i64 = 6;
/// Widest word, in bits, `not`/`signed` take.
pub const MAX_BITS: i64 = 53;

fn label(a: &Arg) -> String {
    match a {
        Arg::Int(_) => "Int".to_string(),
        Arg::Char(_) => "Char".to_string(),
        Arg::Text(_) => "String".to_string(),
        Arg::Other(t) => t.clone(),
    }
}

/// An integer argument: an Int, or a Char's code.
fn int(f: &str, a: Option<&Arg>) -> Result<i64, String> {
    match a {
        Some(Arg::Int(n)) => Ok(*n),
        Some(Arg::Char(c)) => Ok(*c as i64),
        Some(other) => Err(format!("bits::{f}: argument type must be Int or Char, got {}", label(other))),
        None => Err(format!("bits::{f}: argument type must be Int or Char, got nothing")),
    }
}

/// An integer whose bits are taken: from 0 to the largest integer.
fn bits_of(f: &str, a: Option<&Arg>) -> Result<i64, String> {
    let n = int(f, a)?;
    if n < 0 {
        return Err(format!("bits::{f}: {n} is out of range: bits are taken of integers from 0 to 9007199254740991"));
    }
    Ok(n)
}

fn width(f: &str, a: Option<&Arg>) -> Result<u32, String> {
    let w = int(f, a)?;
    if !(1..=MAX_BITS).contains(&w) {
        return Err(format!("bits::{f}: a width of {w} bits is out of range: it goes from 1 to 53"));
    }
    Ok(w as u32)
}

fn size(f: &str, k: i64) -> Result<u32, String> {
    if !(1..=MAX_BYTES).contains(&k) {
        return Err(format!("bits::{f}: {k} bytes is out of range: a word goes from 1 to 6"));
    }
    Ok(k as u32)
}

/// Call `f` of std/bits.
pub fn call(f: &str, args: &[Arg]) -> Result<Out, String> {
    let a = args.first();
    let b = args.get(1);
    match f {
        "and" => Ok(Out::Int(bits_of(f, a)? & bits_of(f, b)?)),
        "or" => Ok(Out::Int(bits_of(f, a)? | bits_of(f, b)?)),
        "xor" => Ok(Out::Int(bits_of(f, a)? ^ bits_of(f, b)?)),
        "shl" | "shr" => {
            let n = bits_of(f, a)?;
            let k = int(f, b)?;
            if k < 0 {
                return Err(format!("bits::{f}: a shift of {k} is out of range: it goes from 0"));
            }
            if f == "shr" {
                return Ok(Out::Int(if k >= 63 { 0 } else { n >> k }));
            }
            if n == 0 {
                return Ok(Out::Int(0));
            }
            // n ≥ 1, so a shift of 53 or more is past the range in any case.
            let r = if k >= MAX_BITS { None } else { (n as i128).checked_shl(k as u32) };
            match r {
                Some(r) if r <= ZY_INT_MAX as i128 => Ok(Out::Int(r as i64)),
                _ => Err(format!("integer overflow: bits::shl({n}, {k})")),
            }
        }
        "not" => {
            let n = bits_of(f, a)?;
            let w = width(f, b)?;
            let mask = (1i64 << w) - 1;
            if n > mask {
                return Err(format!("bits::{f}: {n} is out of range for {w} bits"));
            }
            Ok(Out::Int(mask ^ n))
        }
        "signed" => {
            let n = bits_of(f, a)?;
            let w = width(f, b)?;
            let top = 1i64 << w;
            if n >= top {
                return Err(format!("bits::{f}: {n} is out of range for {w} bits"));
            }
            Ok(Out::Int(if n >= top / 2 { n - top } else { n }))
        }
        "to_le" | "to_be" => {
            let n = bits_of(f, a)?;
            let k = size(f, int(f, b)?)?;
            if n >= 1i64 << (8 * k) {
                return Err(format!("bits::{f}: {n} is out of range for {k} bytes"));
            }
            let mut bytes: Vec<char> = (0..k).map(|i| char::from(((n >> (8 * i)) & 0xFF) as u8)).collect();
            if f == "to_be" {
                bytes.reverse();
            }
            Ok(Out::Bytes(bytes.into_iter().collect()))
        }
        "from_le" | "from_be" => {
            let text: String = match a {
                Some(Arg::Text(s)) => s.to_string(),
                Some(Arg::Char(c)) => c.to_string(),
                Some(other) => {
                    return Err(format!("bits::{f}: argument type must be String or Char, got {}", label(other)))
                }
                None => return Err(format!("bits::{f}: argument type must be String or Char, got nothing")),
            };
            let mut bytes = Vec::new();
            for c in text.chars() {
                if (c as u32) > 0xFF {
                    return Err(format!(
                        "bits::{f}: '{c}' is U+{:04X}, out of range for a byte: a byte is a Char from 0x00 to 0xFF",
                        c as u32
                    ));
                }
                bytes.push(c as i64);
            }
            size(f, bytes.len() as i64)?;
            if f == "from_le" {
                bytes.reverse();
            }
            Ok(Out::Int(bytes.into_iter().fold(0, |acc, byte| acc * 256 + byte)))
        }
        _ => Err(format!("bits::{f}: no such function")),
    }
}

#[cfg(test)]
mod tests {
    use super::{call, Arg, Out};

    fn i(f: &str, args: &[Arg]) -> i64 {
        match call(f, args) {
            Ok(Out::Int(n)) => n,
            Ok(Out::Bytes(s)) => panic!("{f}: bytes {s:?}"),
            Err(e) => panic!("{f}: {e}"),
        }
    }
    fn s(f: &str, args: &[Arg]) -> String {
        match call(f, args) {
            Ok(Out::Bytes(s)) => s,
            _ => panic!("{f}"),
        }
    }
    fn e(f: &str, args: &[Arg]) -> String {
        match call(f, args) {
            Err(e) => e,
            Ok(_) => panic!("{f}: no error"),
        }
    }

    #[test]
    fn bit_by_bit() {
        assert_eq!(i("and", &[Arg::Char('\u{A5}'), Arg::Int(0x0F)]), 5);
        assert_eq!(i("or", &[Arg::Int(0xA0), Arg::Int(5)]), 0xA5);
        assert_eq!(i("xor", &[Arg::Int(0xFF), Arg::Int(0x0F)]), 0xF0);
        assert_eq!(i("not", &[Arg::Int(0x0F), Arg::Int(8)]), 0xF0);
        let max = super::ZY_INT_MAX;
        assert_eq!(i("and", &[Arg::Int(max), Arg::Int(max)]), max);
        assert_eq!(i("not", &[Arg::Int(0), Arg::Int(53)]), max);
    }

    #[test]
    fn shifts_stop_at_the_range() {
        assert_eq!(i("shl", &[Arg::Int(1), Arg::Int(4)]), 16);
        assert_eq!(i("shr", &[Arg::Int(0xF0), Arg::Int(4)]), 15);
        assert_eq!(i("shl", &[Arg::Int(1), Arg::Int(52)]), 1 << 52);
        assert_eq!(i("shl", &[Arg::Int(0), Arg::Int(500)]), 0);
        assert_eq!(i("shr", &[Arg::Int(5), Arg::Int(500)]), 0);
        assert_eq!(e("shl", &[Arg::Int(1), Arg::Int(53)]), "integer overflow: bits::shl(1, 53)");
        assert!(e("shl", &[Arg::Int(1), Arg::Int(-1)]).contains("out of range"));
    }

    #[test]
    fn words_in_both_orders() {
        assert_eq!(s("to_le", &[Arg::Int(4660), Arg::Int(2)]), "\u{34}\u{12}");
        assert_eq!(s("to_be", &[Arg::Int(4660), Arg::Int(2)]), "\u{12}\u{34}");
        assert_eq!(i("from_le", &[Arg::Text("\u{34}\u{12}")]), 4660);
        assert_eq!(i("from_be", &[Arg::Text("\u{12}\u{34}")]), 4660);
        let top = (1i64 << 48) - 1;
        let w = s("to_be", &[Arg::Int(top), Arg::Int(6)]);
        assert_eq!(i("from_be", &[Arg::Text(&w)]), top);
        assert_eq!(i("signed", &[Arg::Int(0xFF), Arg::Int(8)]), -1);
        assert_eq!(i("signed", &[Arg::Int(0x7F), Arg::Int(8)]), 127);
        assert_eq!(i("signed", &[Arg::Int(0x8000), Arg::Int(16)]), -32768);
    }

    /// The words `zymbol_common::errkind` reads: "out of range" or "overflow"
    /// is a `##Range`, "type" a `##Type` — checked in the engines' own tests.
    fn kind(m: &str) -> &'static str {
        if m.contains("overflow") || m.contains("out of range") { "Range" } else if m.contains("type") { "Type" } else { "_" }
    }

    #[test]
    fn every_refusal_names_its_family_by_its_words() {
        for (f, args, family) in [
            ("and", vec![Arg::Int(-1), Arg::Int(1)], "Range"),
            ("and", vec![Arg::Other("Float".into()), Arg::Int(1)], "Type"),
            ("not", vec![Arg::Int(256), Arg::Int(8)], "Range"),
            ("not", vec![Arg::Int(1), Arg::Int(54)], "Range"),
            ("to_le", vec![Arg::Int(256), Arg::Int(1)], "Range"),
            ("to_le", vec![Arg::Int(1), Arg::Int(7)], "Range"),
            ("from_le", vec![Arg::Text("€")], "Range"),
            ("from_le", vec![Arg::Text("")], "Range"),
            ("from_le", vec![Arg::Text("1234567")], "Range"),
            ("from_be", vec![Arg::Int(1)], "Type"),
            ("signed", vec![Arg::Int(256), Arg::Int(8)], "Range"),
        ] {
            assert_eq!(kind(&e(f, &args)), family, "{f}");
        }
    }
}
