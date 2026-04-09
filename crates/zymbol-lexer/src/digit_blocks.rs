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
    for &(base, _) in DIGIT_BLOCKS {
        if cp >= base && cp <= base + 9 {
            return Some(base);
        }
    }
    None
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
}
