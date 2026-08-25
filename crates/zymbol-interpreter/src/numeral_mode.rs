//! Runtime numeral-mode conversion for multi-script output.
//!
//! The active numeral mode is stored in `Interpreter::numeral_mode` as the
//! block base codepoint of the chosen script (e.g. `0x0030` for ASCII,
//! `0x0966` for Devanagari).  Every `>>` output that produces a number maps
//! each decimal digit through the active script before writing.
//!
//! Non-numeric values (strings, arrays, lambdas …) are not affected.
//!
//! The separators follow the script too, where the script encodes its own —
//! see `zymbol_lexer::digit_blocks::decimal_separator`. Only Arabic does, so in
//! practice a number is written `٤٫٧٥` under `#٠٩#` and `४.७५` under `#०९#`.
//! The `-` sign and the `e`/`E` exponent marker always remain ASCII.
//!
//! Which separator a program *writes* is settled by the active mode; which ones
//! it can *read* is every one of them (`digit_blocks::is_decimal_separator`),
//! so a rendered number still parses back. The language itself never inverts
//! the pair: `,` groups and `.` divides, in every script. A program that wants
//! `100.000,00` builds it.

/// Block base for the ASCII digit block (default numeral mode).
pub const ASCII_BASE: u32 = 0x0030;

/// Converts an `i64` to a string in the numeral system identified by `block_base`.
///
/// Negative values retain their ASCII `-` prefix.
pub fn to_numeral_int(value: i64, block_base: u32) -> String {
    map_numeral_number(value.to_string(), block_base)
}

/// Converts an `f64` to a string in the numeral system identified by `block_base`.
///
/// The digit groups are converted and so is the decimal separator, where the
/// script encodes one. Any `e`/`E` exponent marker remains ASCII.
pub fn to_numeral_float(value: f64, block_base: u32) -> String {
    map_numeral_number(value.to_string(), block_base)
}

/// Converts a `bool` to `"#0"` or `"#1"` in the active numeral system.
///
/// The `#` prefix is always ASCII so that boolean output is visually distinct
/// from integer output. The digit is `digit_at(block_base + 0)` for `false`
/// and `digit_at(block_base + 1)` for `true`.
pub fn to_numeral_bool(value: bool, block_base: u32) -> String {
    format!("#{}", to_numeral_int(if value { 1 } else { 0 }, block_base))
}

/// Rewrites one **formatted number** into the script identified by
/// `block_base`: every ASCII digit becomes its equivalent, `.` becomes the
/// script's decimal separator and `,` its thousands separator.
///
/// The argument must be a single number and nothing else, and every caller
/// hands it one: `to_numeral_int`, `to_numeral_float`, and the `#,`/`#^`
/// formatters. That precondition is what lets it touch `.` and `,` at all —
/// they are ordinary punctuation, so running this over composite text
/// (`[१, २, ३]`, `"n=42"`, a file path) would rewrite marks that were never
/// separators. Composite text never reaches here: a list, an interpolation and
/// a concatenation each map their numbers one at a time and add their own
/// commas afterwards.
///
/// Takes `s` by value so the ASCII fast-path — the default mode, and the path a
/// `"label" i` concatenation takes on every iteration — hands the buffer
/// straight back instead of re-allocating it.
pub fn map_numeral_number(s: String, block_base: u32) -> String {
    if block_base == ASCII_BASE {
        return s;
    }
    let decimal = zymbol_lexer::digit_blocks::decimal_separator(block_base);
    let thousands = zymbol_lexer::digit_blocks::thousands_separator(block_base);
    s.chars()
        .map(|ch| match ch {
            '0'..='9' => char::from_u32(block_base + (ch as u32 - ASCII_BASE)).unwrap_or(ch),
            '.' => decimal,
            ',' => thousands,
            _ => ch,
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
            .chain(std::iter::repeat_n(zero, 6))
            .collect();
        assert_eq!(to_numeral_int(1_000_000, 0x0966), expected);
    }

    // ── to_numeral_float ──────────────────────────────────────────────────────

    #[test]
    #[allow(clippy::approx_constant)] // 3.14 is a numeral-formatting fixture, not π
    fn float_ascii_passthrough() {
        assert_eq!(to_numeral_float(3.14, ASCII_BASE), "3.14");
        assert_eq!(to_numeral_float(-0.5, ASCII_BASE), "-0.5");
    }

    #[test]
    #[allow(clippy::approx_constant)] // 3.14 is a numeral-formatting fixture, not π
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

    // ── map_numeral_number: separators follow the script ──────────────────────

    #[test]
    fn ascii_mode_is_untouched() {
        assert_eq!(map_numeral_number("1,234,567.89".to_string(), ASCII_BASE), "1,234,567.89");
    }

    #[test]
    fn arabic_writes_its_own_separators() {
        assert_eq!(map_numeral_number("1,234,567.89".to_string(), 0x0660), "١٬٢٣٤٬٥٦٧٫٨٩");
        assert_eq!(map_numeral_number("4.75".to_string(), 0x06F0), "۴٫۷۵");
    }

    #[test]
    fn devanagari_keeps_the_ascii_pair() {
        // 66 of the 69 scripts have no separator of their own, and inventing
        // one for them would be an invention, not a translation.
        assert_eq!(map_numeral_number("1,234.5".to_string(), 0x0966), "१,२३४.५");
        assert_eq!(map_numeral_number("1,234.5".to_string(), 0x0E50), "๑,๒๓๔.๕");
    }

    #[test]
    fn the_pair_never_inverts() {
        // The language says `,` groups and `.` divides, in every script. A
        // program that wants `100.000,00` builds it; the engine does not.
        assert_eq!(map_numeral_number("100,000.00".to_string(), 0x0966), "१००,०००.००");
    }

    #[test]
    fn the_sign_and_the_exponent_stay_ascii() {
        assert_eq!(map_numeral_number("-1.5e6".to_string(), 0x0660), "-١٫٥e٦");
    }
}
