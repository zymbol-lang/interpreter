//! The numeric model of Zymbol — one definition, four engines.
//!
//! # Why this file exists
//!
//! Zymbol never declared its integer type, so each engine inherited the one its
//! host offered: `i64` in the two Rust engines, OCaml's 63-bit `int` in `zyml`,
//! an IEEE-754 double in the browser. `10 ^ 20` produced four different answers
//! — a runtime error, two different wrapped values, and a float that happened
//! to be exact — and the parity gate never noticed, because the corpus held no
//! program that could overflow.
//!
//! The type is now **the safe integer**: the widest range every engine can
//! represent exactly and natively, which is the range a `f64` mantissa holds.
//! No engine pays for boxing (`Int64` in OCaml) or for `BigInt` in the browser,
//! and no engine can silently disagree, because none of them is approximating.
//!
//! # The rules
//!
//! - An integer is in `[ZY_INT_MIN, ZY_INT_MAX]` = ±(2⁵³ − 1). Anything outside
//!   is not a Zymbol integer, whether it arrives from a literal, an arithmetic
//!   result, a cast, or a database column.
//! - Leaving the range is a **`##Range` error**, never a wrapped value and never
//!   a silent promotion to float. The whole point of a fixed range is that
//!   crossing it is observable.
//! - Floats are IEEE-754 doubles and keep IEEE-754 semantics: overflow yields
//!   `inf`, which is a value, not an error.
//!
//! The engines that are not Rust reimplement these same rules — see
//! `zyml/src/value.ml` and `web/src/zymbol/zymbol.js`. The error messages are
//! spelled identically in all four, because `zyq consensus` compares text.

/// The largest Zymbol integer: 2⁵³ − 1.
///
/// The bound is the f64 mantissa, not a Rust type: it is the last integer that
/// `zyjs` can hold with no rounding, so it is the last one all four engines
/// agree on. `i64` and OCaml's 63-bit `int` both have room to spare, which is
/// what makes the check below cheap everywhere.
pub const ZY_INT_MAX: i64 = 9_007_199_254_740_991;

/// The smallest Zymbol integer: −(2⁵³ − 1).
///
/// Deliberately symmetric. `f64` can represent −2⁵³ exactly, but `-ZY_INT_MAX`
/// keeps negation total: every integer's negation is an integer, so `-x` is the
/// one arithmetic operation that can never raise `##Range`.
pub const ZY_INT_MIN: i64 = -ZY_INT_MAX;

/// The range as it is spelled to users. Kept next to the constants so a
/// message can never drift from the bound it is quoting.
pub const ZY_INT_RANGE_HELP: &str = "integers range from -9007199254740991 to 9007199254740991 (±2⁵³−1)";

/// True when `v` is a Zymbol integer.
#[inline(always)]
pub const fn in_int_range(v: i64) -> bool {
    v >= ZY_INT_MIN && v <= ZY_INT_MAX
}

/// The result of an integer operation, or `None` if it left the range.
///
/// Takes the `i64` the operation would have produced. `i64` has 10 bits of
/// headroom over `ZY_INT_MAX`, so `+`, `-` and unary `-` cannot themselves
/// overflow `i64` on in-range operands and can be computed first and checked
/// after. Multiplication and exponentiation can, so those callers must use the
/// `checked_*` family and treat `None` as out of range — see [`mul`] and [`pow`].
#[inline(always)]
pub const fn checked(v: i64) -> Option<i64> {
    if in_int_range(v) { Some(v) } else { None }
}

/// `a + b`, or `None` if the sum is not a Zymbol integer.
#[inline(always)]
pub const fn add(a: i64, b: i64) -> Option<i64> {
    match a.checked_add(b) {
        Some(v) => checked(v),
        None => None,
    }
}

/// `a - b`, or `None` if the difference is not a Zymbol integer.
#[inline(always)]
pub const fn sub(a: i64, b: i64) -> Option<i64> {
    match a.checked_sub(b) {
        Some(v) => checked(v),
        None => None,
    }
}

/// `a * b`, or `None` if the product is not a Zymbol integer.
#[inline(always)]
pub const fn mul(a: i64, b: i64) -> Option<i64> {
    match a.checked_mul(b) {
        Some(v) => checked(v),
        None => None,
    }
}

/// `-a`. Total on in-range operands, by the symmetry of the bounds.
#[inline(always)]
pub const fn neg(a: i64) -> Option<i64> {
    checked(-a)
}

/// `base ^ exp` for a non-negative `exp`, or `None` if it is not a Zymbol
/// integer. A negative exponent is a float operation and never reaches here.
///
/// Checks every intermediate product rather than only the result: with an
/// `i64` accumulator a partial product could wrap past the range and land back
/// inside it, which is exactly the silent answer this module exists to prevent.
pub fn pow(base: i64, exp: u32) -> Option<i64> {
    // Settle the three bases whose powers never leave the range first. Without
    // this, `1 ^ 4000000000` would spin through four billion multiplications to
    // arrive at 1. Every other base has |base| ≥ 2, so the loop below leaves
    // the range within 53 steps.
    match base {
        0 => return Some(if exp == 0 { 1 } else { 0 }),
        1 => return Some(1),
        -1 => return Some(if exp % 2 == 0 { 1 } else { -1 }),
        _ => {}
    }
    let mut acc: i64 = 1;
    let mut i = 0;
    while i < exp {
        acc = mul(acc, base)?;
        i += 1;
    }
    Some(acc)
}

/// The `f64` as a Zymbol integer, or `None` if it has no exact integer form in
/// range. Used by the `###` cast and by every reader that turns external
/// numeric data (JSON, a database column) into a `Value::Int`.
#[inline]
pub fn from_f64(v: f64) -> Option<i64> {
    if !v.is_finite() {
        return None;
    }
    let t = v.trunc();
    if t < ZY_INT_MIN as f64 || t > ZY_INT_MAX as f64 {
        return None;
    }
    Some(t as i64)
}

/// The message an out-of-range binary operation reports, spelled the same way
/// in all four engines: `integer overflow: 9007199254740991 + 1`.
pub fn overflow_msg(a: i64, op: &str, b: i64) -> String {
    format!("integer overflow: {} {} {}", a, op, b)
}

/// What a piece of text spells, numerically.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Num {
    Int(i64),
    Float(f64),
    /// Not a number the language can hold — including digits that spell an
    /// integer past the range.
    None,
}

/// Read `text` as a number, by the one rule every engine follows.
///
/// The engines each build their own `Value`, but they must not each decide
/// *what* the text says: this used to be four hand-copied if-chains (one in the
/// tree-walker, three in the VM) that agreed only by luck.
///
/// Text that spells an integer is judged by the integer rules alone. Out of
/// range it is [`Num::None`] and is **not** retried as a float, because a
/// fail-safe reader must never answer with a value the text did not hold:
/// `"9007199254740993"` is not 9007199254740992.0. Text with a point or an
/// exponent is a float, where the range does not apply.
///
/// `text` should already be trimmed, and already normalised to ASCII digits if
/// the caller supports other numeral scripts.
pub fn parse(text: &str) -> Num {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        return match text.parse::<i64>().ok().filter(|n| in_int_range(*n)) {
            Some(n) => Num::Int(n),
            None => Num::None,
        };
    }
    match text.parse::<f64>() {
        Ok(f) => Num::Float(f),
        Err(_) => Num::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_are_the_f64_mantissa() {
        assert_eq!(ZY_INT_MAX, (1i64 << 53) - 1);
        assert_eq!(ZY_INT_MIN, -ZY_INT_MAX);
        // The defining property: every bound survives the round trip through a
        // double, which is what makes zyjs able to hold it.
        assert_eq!(ZY_INT_MAX as f64 as i64, ZY_INT_MAX);
        assert_eq!(ZY_INT_MIN as f64 as i64, ZY_INT_MIN);
        assert_ne!((ZY_INT_MAX + 1) as f64 as i64, ZY_INT_MAX + 1 + 1);
    }

    #[test]
    fn edges_are_in_range_and_one_past_is_not() {
        assert!(in_int_range(ZY_INT_MAX));
        assert!(in_int_range(ZY_INT_MIN));
        assert!(!in_int_range(ZY_INT_MAX + 1));
        assert!(!in_int_range(ZY_INT_MIN - 1));
    }

    #[test]
    fn add_sub_stop_at_the_edge() {
        assert_eq!(add(ZY_INT_MAX - 1, 1), Some(ZY_INT_MAX));
        assert_eq!(add(ZY_INT_MAX, 1), None);
        assert_eq!(sub(ZY_INT_MIN, 1), None);
        // Past the i64 edge too, not just past the Zymbol edge.
        assert_eq!(add(i64::MAX, 1), None);
    }

    #[test]
    fn negation_is_total() {
        assert_eq!(neg(ZY_INT_MAX), Some(ZY_INT_MIN));
        assert_eq!(neg(ZY_INT_MIN), Some(ZY_INT_MAX));
    }

    #[test]
    fn pow_catches_the_case_that_started_this() {
        assert_eq!(pow(10, 15), Some(1_000_000_000_000_000));
        assert_eq!(pow(10, 20), None);
        assert_eq!(pow(2, 53), None);
        assert_eq!(pow(2, 52), Some(4_503_599_627_370_496));
        assert_eq!(pow(0, 0), Some(1));
        assert_eq!(pow(-2, 3), Some(-8));
        // The bases that short-circuit: answered, not looped four billion times.
        assert_eq!(pow(1, u32::MAX), Some(1));
        assert_eq!(pow(-1, u32::MAX), Some(-1));
        assert_eq!(pow(-1, u32::MAX - 1), Some(1));
        assert_eq!(pow(0, u32::MAX), Some(0));
        // An intermediate that wraps i64 back into range must still be None.
        assert_eq!(pow(3, 41), None);
    }

    #[test]
    fn f64_conversion_rejects_what_it_cannot_hold() {
        assert_eq!(from_f64(42.9), Some(42));
        assert_eq!(from_f64(-42.9), Some(-42));
        assert_eq!(from_f64(ZY_INT_MAX as f64), Some(ZY_INT_MAX));
        assert_eq!(from_f64(1e300), None);
        assert_eq!(from_f64(f64::INFINITY), None);
        assert_eq!(from_f64(f64::NAN), None);
    }

    #[test]
    fn parse_reads_integers_by_the_integer_rules() {
        assert_eq!(parse("42"), Num::Int(42));
        assert_eq!(parse("-42"), Num::Int(-42));
        assert_eq!(parse("+42"), Num::Int(42));
        assert_eq!(parse("9007199254740991"), Num::Int(ZY_INT_MAX));
        // Out of range: None, and specifically *not* a float. The old chains
        // fell through to `parse::<f64>()` and answered 9007199254740992.0.
        assert_eq!(parse("9007199254740992"), Num::None);
        assert_eq!(parse("9223372036854775807"), Num::None);
        assert_eq!(parse("99999999999999999999999"), Num::None);
    }

    #[test]
    fn parse_reads_floats_and_refuses_the_rest() {
        assert_eq!(parse("4.5"), Num::Float(4.5));
        assert_eq!(parse("1e300"), Num::Float(1e300));
        assert_eq!(parse("-0.5"), Num::Float(-0.5));
        assert_eq!(parse("abc"), Num::None);
        assert_eq!(parse(""), Num::None);
        assert_eq!(parse("-"), Num::None);
        assert_eq!(parse("12abc"), Num::None);
    }

    #[test]
    fn the_message_is_the_one_the_other_engines_spell() {
        assert_eq!(overflow_msg(9007199254740991, "+", 1), "integer overflow: 9007199254740991 + 1");
    }
}
