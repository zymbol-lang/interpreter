//! Runtime numeral-mode conversion for multi-script output.
//!
//! The active numeral mode is stored in `Interpreter::numeral_mode` as the
//! block base codepoint of the chosen script (e.g. `0x0030` for ASCII,
//! `0x0966` for Devanagari).  Every `>>` output that produces a number maps
//! each decimal digit through the active script before writing.
//!
//! Non-numeric values (strings, arrays, lambdas …) are not affected.
//! The `-` sign, `.` decimal separator, and `e`/`E` exponent marker always
//! remain ASCII — only the digit characters change.

/// Block base for the ASCII digit block (default numeral mode).
pub const ASCII_BASE: u32 = 0x0030;

/// Converts an `i64` to a string in the numeral system identified by `block_base`.
///
/// Negative values retain their ASCII `-` prefix; only digit characters are
/// mapped to the target script.
pub fn to_numeral_int(value: i64, block_base: u32) -> String {
    let s = value.to_string();
    map_ascii_digits(&s, block_base)
}

/// Converts an `f64` to a string in the numeral system identified by `block_base`.
///
/// The integer and fractional digit groups are both converted.
/// The `.` separator and any `e`/`E` exponent marker remain ASCII.
pub fn to_numeral_float(value: f64, block_base: u32) -> String {
    let s = value.to_string();
    map_ascii_digits(&s, block_base)
}

/// Converts a `bool` to `"#0"` or `"#1"` in the active numeral system.
///
/// The `#` prefix is always ASCII so that boolean output is visually distinct
/// from integer output. The digit is `digit_at(block_base + 0)` for `false`
/// and `digit_at(block_base + 1)` for `true`.
pub fn to_numeral_bool(value: bool, block_base: u32) -> String {
    format!("#{}", to_numeral_int(if value { 1 } else { 0 }, block_base))
}

/// Replaces every ASCII digit in `s` with its equivalent in the script
/// identified by `block_base`.  All other characters pass through unchanged.
///
/// Fast-path: returns a clone of `s` without allocation when `block_base`
/// is `ASCII_BASE` (0x0030).
fn map_ascii_digits(s: &str, block_base: u32) -> String {
    if block_base == ASCII_BASE {
        return s.to_string();
    }
    s.chars()
        .map(|ch| {
            if ch.is_ascii_digit() {
                let d = ch as u32 - ASCII_BASE;
                char::from_u32(block_base + d).unwrap_or(ch)
            } else {
                ch
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── to_numeral_int ────────────────────────────────────────────────────────

    #[test]
    fn int_ascii_passthrough() {
        assert_eq!(to_numeral_int(42, ASCII_BASE), "42");
        assert_eq!(to_numeral_int(0, ASCII_BASE), "0");
        assert_eq!(to_numeral_int(-7, ASCII_BASE), "-7");
    }

    #[test]
    fn int_devanagari() {
        assert_eq!(to_numeral_int(42, 0x0966), "४२");
        assert_eq!(to_numeral_int(0, 0x0966), "०");
        assert_eq!(to_numeral_int(-7, 0x0966), "-७");
        assert_eq!(to_numeral_int(255, 0x0966), "२५५");
    }

    #[test]
    fn int_arabic_indic() {
        assert_eq!(to_numeral_int(42, 0x0660), "٤٢");
    }

    #[test]
    fn int_thai() {
        assert_eq!(to_numeral_int(123, 0x0E50), "๑๒๓");
    }

    #[test]
    fn int_adlam() {
        // 𞥐=0, 𞥑=1, 𞥒=2 (U+1E950-U+1E952)
        let zero = char::from_u32(0x1E950).unwrap();
        let one  = char::from_u32(0x1E951).unwrap();
        let two  = char::from_u32(0x1E952).unwrap();
        let expected: String = [one, two].iter().collect();
        assert_eq!(to_numeral_int(12, 0x1E950), expected);
        let expected_zero: String = [zero].iter().collect();
        assert_eq!(to_numeral_int(0, 0x1E950), expected_zero);
    }

    #[test]
    fn int_large_number() {
        // 1_000_000 = "1000000" (7 digits) → Devanagari १ followed by six ०
        let one  = char::from_u32(0x0967).unwrap(); // १
        let zero = char::from_u32(0x0966).unwrap(); // ०
        let expected: String = std::iter::once(one)
            .chain(std::iter::repeat(zero).take(6))
            .collect();
        assert_eq!(to_numeral_int(1_000_000, 0x0966), expected);
    }

    // ── to_numeral_float ──────────────────────────────────────────────────────

    #[test]
    fn float_ascii_passthrough() {
        assert_eq!(to_numeral_float(3.14, ASCII_BASE), "3.14");
        assert_eq!(to_numeral_float(-0.5, ASCII_BASE), "-0.5");
    }

    #[test]
    fn float_devanagari() {
        assert_eq!(to_numeral_float(3.14, 0x0966), "३.१४");
    }

    #[test]
    fn float_thai() {
        assert_eq!(to_numeral_float(0.5, 0x0E50), "๐.๕");
    }

    #[test]
    fn float_scientific_digits_converted_sign_and_e_stay_ascii() {
        // 1e10 formats as "10000000000" in Rust (no scientific notation for small exponents)
        // but large floats may use scientific form — verify the 'e' stays ASCII
        let s = to_numeral_float(1e20, 0x0966);
        // Must contain only Devanagari digits, 'e', '+'/'-', '.'
        for ch in s.chars() {
            let is_deva = ch as u32 >= 0x0966 && ch as u32 <= 0x096F;
            let is_ascii_structural = matches!(ch, 'e' | 'E' | '+' | '-' | '.');
            assert!(
                is_deva || is_ascii_structural,
                "unexpected char '{}' (U+{:04X}) in float output",
                ch, ch as u32
            );
        }
    }

    // ── to_numeral_bool ───────────────────────────────────────────────────────

    #[test]
    fn bool_ascii() {
        assert_eq!(to_numeral_bool(false, ASCII_BASE), "#0");
        assert_eq!(to_numeral_bool(true, ASCII_BASE), "#1");
    }

    #[test]
    fn bool_devanagari() {
        assert_eq!(to_numeral_bool(false, 0x0966), "#०");
        assert_eq!(to_numeral_bool(true, 0x0966), "#१");
    }

    #[test]
    fn bool_thai() {
        assert_eq!(to_numeral_bool(false, 0x0E50), "#๐");
        assert_eq!(to_numeral_bool(true, 0x0E50), "#๑");
    }
}
