//! Unicode digit block table and normalization utilities.
//!
//! Every supported numeral system maps to a contiguous block of exactly 10
//! codepoints: `block_base + 0` … `block_base + 9`.  The normalization formula
//! is uniform across all blocks:
//!
//! ```text
//! digit_value = codepoint − block_base
//! ```
//!
//! Detection is O(n_blocks) over the flat table below (69 entries).

/// `(block_base_codepoint, script_name)`
///
/// Each entry describes a contiguous Unicode digit block of exactly 10
/// codepoints.  Entries are sorted by codepoint (BMP first, then SMP) so that
/// a future binary-search optimisation is a drop-in replacement.
pub const DIGIT_BLOCKS: &[(u32, &str)] = &[
    // ── BMP ──────────────────────────────────────────────────────────────────
    (0x0030, "ASCII"),
    (0x0660, "Arabic-Indic"),
    (0x06F0, "Extended Arabic-Indic"),
    (0x07C0, "NKo"),
    (0x0966, "Devanagari"),
    (0x09E6, "Bengali"),
    (0x0A66, "Gurmukhi"),
    (0x0AE6, "Gujarati"),
    (0x0B66, "Oriya"),
    (0x0BE6, "Tamil"),
    (0x0C66, "Telugu"),
    (0x0CE6, "Kannada"),
    (0x0D66, "Malayalam"),
    (0x0DE6, "Sinhala Archaic"),
    (0x0E50, "Thai"),
    (0x0ED0, "Lao"),
    (0x0F20, "Tibetan"),
    (0x1040, "Myanmar"),
    (0x1090, "Myanmar Shan"),
    (0x17E0, "Khmer"),
    (0x1810, "Mongolian"),
    (0x1946, "Limbu"),
    (0x19D0, "New Tai Lue"),
    (0x1A80, "Tai Tham Hora"),
    (0x1A90, "Tai Tham Tham"),
    (0x1B50, "Balinese"),
    (0x1BB0, "Sundanese"),
    (0x1C40, "Lepcha"),
    (0x1C50, "Ol Chiki"),
    (0xA620, "Vai"),
    (0xA8D0, "Saurashtra"),
    (0xA900, "Kayah Li"),
    (0xA9D0, "Javanese"),
    (0xA9F0, "Myanmar Tai Laing"),
    (0xAA50, "Cham"),
    (0xABF0, "Meetei Mayek"),
    // ── BMP — ConScript Unicode Registry (CSUR) — fictional scripts ──────────
    // Klingon pIqaD digits (CSUR PUA U+F8F0–U+F8F9). Only fictional exception.
    // Requires a pIqaD-capable font (e.g. KLI pIqaD) to render visually.
    (0xF8F0, "Klingon pIqaD"),
    (0xFF10, "Fullwidth"),
    // ── SMP — historical & modern scripts ────────────────────────────────────
    (0x104A0, "Osmanya"),
    (0x10D30, "Hanifi Rohingya"),
    (0x11066, "Brahmi"),
    (0x110F0, "Sora Sompeng"),
    (0x11136, "Chakma"),
    (0x111D0, "Sharada"),
    (0x112F0, "Khudawadi"),
    (0x11450, "Newa"),
    (0x114D0, "Tirhuta"),
    (0x11650, "Modi"),
    (0x116C0, "Takri"),
    (0x11730, "Ahom"),
    (0x118E0, "Warang Citi"),
    (0x11950, "Dives Akuru"),
    (0x11C50, "Bhaiksuki"),
    (0x11D50, "Masaram Gondi"),
    (0x11DA0, "Gunjala Gondi"),
    (0x11F50, "Kawi"),
    (0x16A60, "Mro"),
    (0x16AC0, "Tangsa"),
    (0x16B50, "Pahawh Hmong"),
    // ── SMP — mathematical styling variants ──────────────────────────────────
    (0x1D7CE, "Mathematical Bold"),
    (0x1D7D8, "Mathematical Double-struck"),
    (0x1D7E2, "Mathematical Sans-serif"),
    (0x1D7EC, "Mathematical Sans-serif Bold"),
    (0x1D7F6, "Mathematical Monospace"),
    // ── SMP — modern scripts ─────────────────────────────────────────────────
    (0x1E140, "Nyiakeng Puachue Hmong"),
    (0x1E2F0, "Wancho"),
    (0x1E4F0, "Nag Mundari"),
    (0x1E950, "Adlam"),
    // ── SMP — display / specialty ─────────────────────────────────────────────
    (0x1FBF0, "Segmented/LCD"),
];

/// Returns the numeric value (0–9) of `ch` if it belongs to any supported
/// digit block, or `None` if the character is not a recognised digit.
///
/// # Examples
/// ```
/// use zymbol_lexer::digit_blocks::digit_value;
/// assert_eq!(digit_value('5'), Some(5));       // ASCII
/// assert_eq!(digit_value('५'), Some(5));       // Devanagari
/// assert_eq!(digit_value('٥'), Some(5));       // Arabic-Indic
/// assert_eq!(digit_value('a'), None);
/// ```
pub fn digit_value(ch: char) -> Option<u8> {
    let cp = ch as u32;
    for &(base, _) in DIGIT_BLOCKS {
        if cp >= base && cp <= base + 9 {
            return Some((cp - base) as u8);
        }
    }
    None
}

/// Returns the block base codepoint of the digit block that `ch` belongs to,
/// or `None` if `ch` is not a recognised digit.
///
/// Two characters belong to the same script when their `digit_block_base`
/// values are equal.
///
/// # Examples
/// ```
/// use zymbol_lexer::digit_blocks::digit_block_base;
/// assert_eq!(digit_block_base('0'), Some(0x0030));   // ASCII
/// assert_eq!(digit_block_base('०'), Some(0x0966));   // Devanagari
/// assert_eq!(digit_block_base('a'), None);
/// ```
pub fn digit_block_base(ch: char) -> Option<u32> {
    let cp = ch as u32;
    DIGIT_BLOCKS.iter().map(|&(base, _)| base).find(|&base| cp >= base && cp <= base + 9)
}

// ── Script separators ────────────────────────────────────────────────────────

/// `(block_base, decimal_separator, thousands_separator)` for the scripts that
/// encode their own.
///
/// The admission bar is deliberately narrow and objective: **Unicode itself must
/// name the character a numeric separator for that script.** Exactly one script
/// clears it — Arabic, through U+066B ARABIC DECIMAL SEPARATOR and U+066C ARABIC
/// THOUSANDS SEPARATOR — and it clears it for both of its digit blocks: the
/// Arabic-Indic one and the Extended block that Persian and Urdu use.
///
/// Every other entry in `DIGIT_BLOCKS` writes ASCII `.` and `,`, which is what
/// they do in practice. A Devanagari-specific decimal point would be an
/// invention, and a script whose separator is settled by locale preference
/// rather than by Unicode belongs to a locale table — which this language does
/// not carry and is not going to grow. A script joins this list the day Unicode
/// names its separator, not the day a locale prefers one.
const SCRIPT_SEPARATORS: &[(u32, char, char)] = &[
    (0x0660, '\u{066B}', '\u{066C}'), // Arabic-Indic
    (0x06F0, '\u{066B}', '\u{066C}'), // Extended Arabic-Indic (Persian, Urdu)
];

/// The decimal separator of the script identified by `block_base`; ASCII `.`
/// for every script that does not encode one of its own.
pub fn decimal_separator(block_base: u32) -> char {
    SCRIPT_SEPARATORS
        .iter()
        .find(|&&(base, _, _)| base == block_base)
        .map_or('.', |&(_, dec, _)| dec)
}

/// The thousands separator of the script identified by `block_base`; ASCII `,`
/// for every script that does not encode one of its own.
///
/// Only `#,` ever emits one: it is the single operator whose output is text
/// rather than a number.
pub fn thousands_separator(block_base: u32) -> char {
    SCRIPT_SEPARATORS
        .iter()
        .find(|&&(base, _, _)| base == block_base)
        .map_or(',', |&(_, _, thousands)| thousands)
}

/// Whether `ch` reads as a decimal point: ASCII `.` or any script's own.
///
/// Reading is script-blind on purpose, exactly as `digit_value` is. Writing
/// picks one separator, from the active numeral mode; reading accepts them all,
/// so `٣٫٥` and `٣.٥` are the same number and a rendered value parses back.
pub fn is_decimal_separator(ch: char) -> bool {
    ch == '.' || SCRIPT_SEPARATORS.iter().any(|&(_, dec, _)| dec == ch)
}

/// The ASCII form of a numeric string written in any supported digit script,
/// or `None` if the string is not a number at all.
///
/// Accepts an optional leading `-`, digits from any supported script, and at
/// most one decimal separator — ASCII or the script's own, because a number
/// rendered under an active numeral mode has to read back
/// (`corpus/i18n/numeral_mode_round_trip.zy`). A thousands separator is *not*
/// accepted, in any script: `#|"1,234"|` hands the text back untouched today
/// and `#|"١٬٢٣٤"|` does the same, which is the symmetry that matters.
///
/// Both engines call this. It used to be two hand-written copies, one per
/// engine — the shape a divergence takes before anybody notices it.
pub fn ascii_number(s: &str) -> Option<String> {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    if chars.peek() == Some(&'-') {
        result.push('-');
        chars.next();
    }
    let mut has_digit = false;
    let mut has_dot = false;
    for ch in chars {
        if let Some(dv) = digit_value(ch) {
            result.push(char::from_digit(dv as u32, 10).unwrap());
            has_digit = true;
        } else if is_decimal_separator(ch) && !has_dot {
            result.push('.');
            has_dot = true;
        } else {
            return None; // non-numeric character — not a number
        }
    }
    if has_digit { Some(result) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── DIGIT_BLOCKS table sanity ─────────────────────────────────────────────

    #[test]
    fn table_has_expected_length() {
        assert_eq!(DIGIT_BLOCKS.len(), 69);
    }

    #[test]
    fn table_is_sorted_by_codepoint() {
        for w in DIGIT_BLOCKS.windows(2) {
            assert!(w[0].0 < w[1].0, "not sorted: 0x{:X} >= 0x{:X}", w[0].0, w[1].0);
        }
    }

    #[test]
    fn table_blocks_do_not_overlap() {
        for w in DIGIT_BLOCKS.windows(2) {
            let end_prev = w[0].0 + 9;
            let start_next = w[1].0;
            assert!(
                end_prev < start_next,
                "overlap between {} (ends 0x{:X}) and {} (starts 0x{:X})",
                w[0].1, end_prev, w[1].1, start_next
            );
        }
    }

    // ── digit_value ───────────────────────────────────────────────────────────

    #[test]
    fn ascii_digits() {
        for d in 0u8..=9 {
            let ch = char::from_u32(0x0030 + d as u32).unwrap();
            assert_eq!(digit_value(ch), Some(d), "ASCII digit '{}'", ch);
        }
    }

    #[test]
    fn arabic_indic_digits() {
        // U+0660–U+0669
        for d in 0u8..=9 {
            let ch = char::from_u32(0x0660 + d as u32).unwrap();
            assert_eq!(digit_value(ch), Some(d), "Arabic-Indic digit U+{:04X}", 0x0660 + d as u32);
        }
    }

    #[test]
    fn devanagari_digits() {
        // U+0966–U+096F
        for d in 0u8..=9 {
            let ch = char::from_u32(0x0966 + d as u32).unwrap();
            assert_eq!(digit_value(ch), Some(d), "Devanagari digit U+{:04X}", 0x0966 + d as u32);
        }
    }

    #[test]
    fn adlam_digits() {
        // U+1E950–U+1E959
        for d in 0u8..=9 {
            let ch = char::from_u32(0x1E950 + d as u32).unwrap();
            assert_eq!(digit_value(ch), Some(d), "Adlam digit U+{:05X}", 0x1E950 + d as u32);
        }
    }

    #[test]
    fn mathematical_bold_digits() {
        // U+1D7CE–U+1D7D7
        for d in 0u8..=9 {
            let ch = char::from_u32(0x1D7CE + d as u32).unwrap();
            assert_eq!(digit_value(ch), Some(d));
        }
    }

    #[test]
    fn segmented_lcd_digits() {
        // U+1FBF0–U+1FBF9
        for d in 0u8..=9 {
            let ch = char::from_u32(0x1FBF0 + d as u32).unwrap();
            assert_eq!(digit_value(ch), Some(d));
        }
    }

    #[test]
    fn non_digit_chars_return_none() {
        for ch in ['a', 'z', 'A', 'Z', ' ', '\n', '#', '+', '-', '.'] {
            assert_eq!(digit_value(ch), None, "expected None for '{}'", ch);
        }
    }

    #[test]
    fn codepoints_just_outside_blocks_return_none() {
        // One before ASCII '0' and one after ASCII '9'
        assert_eq!(digit_value(char::from_u32(0x002F).unwrap()), None); // '/'
        assert_eq!(digit_value(char::from_u32(0x003A).unwrap()), None); // ':'
        // One before and after Devanagari block
        assert_eq!(digit_value(char::from_u32(0x0965).unwrap()), None);
        assert_eq!(digit_value(char::from_u32(0x0970).unwrap()), None);
    }

    // ── digit_block_base ──────────────────────────────────────────────────────

    #[test]
    fn block_base_ascii() {
        assert_eq!(digit_block_base('0'), Some(0x0030));
        assert_eq!(digit_block_base('9'), Some(0x0030));
        assert_eq!(digit_block_base('5'), Some(0x0030));
    }

    #[test]
    fn block_base_devanagari() {
        assert_eq!(digit_block_base('०'), Some(0x0966));
        assert_eq!(digit_block_base('९'), Some(0x0966));
    }

    #[test]
    fn block_base_thai() {
        assert_eq!(digit_block_base('๐'), Some(0x0E50));
        assert_eq!(digit_block_base('๙'), Some(0x0E50));
    }

    #[test]
    fn block_base_non_digit_returns_none() {
        assert_eq!(digit_block_base('a'), None);
        assert_eq!(digit_block_base('#'), None);
    }

    #[test]
    fn same_script_digits_share_block_base() {
        // All Devanagari digits must return the same base
        let bases: Vec<_> = (0x0966u32..=0x096F)
            .map(|cp| digit_block_base(char::from_u32(cp).unwrap()))
            .collect();
        assert!(bases.iter().all(|b| *b == Some(0x0966)));
    }

    #[test]
    fn different_scripts_have_different_block_bases() {
        assert_ne!(digit_block_base('0'), digit_block_base('०'));   // ASCII vs Devanagari
        assert_ne!(digit_block_base('٠'), digit_block_base('۰'));   // Arabic-Indic vs Extended
    }

    // ── separators ────────────────────────────────────────────────────────────

    #[test]
    fn only_arabic_encodes_its_own_separators() {
        assert_eq!(decimal_separator(0x0660), '\u{066B}');
        assert_eq!(thousands_separator(0x0660), '\u{066C}');
        assert_eq!(decimal_separator(0x06F0), '\u{066B}');
        assert_eq!(thousands_separator(0x06F0), '\u{066C}');
    }

    #[test]
    fn every_other_script_writes_ascii_separators() {
        // The bar is that Unicode name the character a numeric separator FOR
        // the script. Only Arabic clears it; a new entry here means a new
        // Unicode fact, not a new locale preference.
        for &(base, name) in DIGIT_BLOCKS {
            if base == 0x0660 || base == 0x06F0 {
                continue;
            }
            assert_eq!(decimal_separator(base), '.', "{name} decimal");
            assert_eq!(thousands_separator(base), ',', "{name} thousands");
        }
    }

    #[test]
    fn reading_a_separator_is_script_blind() {
        // Writing picks one separator from the active mode; reading accepts
        // them all, which is what lets a rendered number parse back.
        assert!(is_decimal_separator('.'));
        assert!(is_decimal_separator('\u{066B}'));
        assert!(!is_decimal_separator('\u{066C}')); // thousands is not a decimal point
        assert!(!is_decimal_separator(','));
        assert!(!is_decimal_separator('a'));
    }

    // ── ascii_number ──────────────────────────────────────────────────────────

    #[test]
    fn ascii_number_normalizes_any_script() {
        assert_eq!(ascii_number("४२").as_deref(), Some("42"));
        assert_eq!(ascii_number("42").as_deref(), Some("42"));
        assert_eq!(ascii_number("-٧").as_deref(), Some("-7"));
        assert_eq!(ascii_number("๓.๕").as_deref(), Some("3.5"));
    }

    #[test]
    fn ascii_number_accepts_the_scripts_own_decimal_point() {
        // What an active numeral mode writes has to read back.
        assert_eq!(ascii_number("٤٫٧٥").as_deref(), Some("4.75"));
        assert_eq!(ascii_number("٤.٧٥").as_deref(), Some("4.75"));
        assert_eq!(ascii_number("۳٫۵").as_deref(), Some("3.5"));
    }

    #[test]
    fn ascii_number_rejects_a_thousands_separator_in_every_script() {
        // `#|"1,234"|` hands the text back untouched, and the Arabic spelling
        // does the same. That symmetry is the point: `#,` is the one operator
        // whose result is text, and text is not read back.
        assert_eq!(ascii_number("1,234"), None);
        assert_eq!(ascii_number("١٬٢٣٤"), None);
    }

    #[test]
    fn ascii_number_rejects_two_decimal_points() {
        assert_eq!(ascii_number("1.2.3"), None);
        assert_eq!(ascii_number("١٫٢٫٣"), None);
        assert_eq!(ascii_number("١.٢٫٣"), None);
    }

    #[test]
    fn ascii_number_rejects_what_is_not_a_number() {
        assert_eq!(ascii_number("hola"), None);
        assert_eq!(ascii_number(""), None);
        assert_eq!(ascii_number("-"), None);
        assert_eq!(ascii_number("."), None);
    }
}
