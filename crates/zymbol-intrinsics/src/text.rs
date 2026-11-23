//! `std/text` — upper case, lower case and case folding (D-18, 2026-11-23).
//!
//! The three need Unicode's case tables, the way `std/term`'s `width` needs
//! its width tables, and no mark in the language derives them: `^` already
//! carries exponentiation, sort order and scientific notation. So they are a
//! module, with names (GAP-ZYE-010, GAP-TET-005).
//!
//! The rules are Unicode's, with no language of their own: a Turkish `I` lowers
//! to `i` here as everywhere else, so a program answers the same on every
//! machine whatever the locale.
//!
//! - [`upper`] and [`lower`] are the full mappings, which may change the length:
//!   `ß` upper-cases to `SS`. [`lower`] applies the one context rule Unicode
//!   gives, the final sigma: `ΣΑΣ` lowers to `σας`.
//! - [`fold`] is for comparing, and keeps the length: each code point becomes
//!   its simple case folding, one code point for one (Unicode's `CaseFolding.txt`,
//!   statuses C and S). A match found in the folded text is at the same
//!   position in the original, which is what a search that highlights needs.
//!   `ß` therefore stays `ß` and does not match `ss`.
//!
//! - [`graphemes`] cuts a text into its extended grapheme clusters (UAX #29,
//!   D-19): what a reader takes for one letter, `e` + U+0301, a flag, a family
//!   joined by U+200D. A `Char` stays one code point; this is how to see the
//!   clusters.
//!
//! The browser engines carry a table generated from these functions
//! (`web/src/zymbol/casemap.js`), because a browser's own `toUpperCase` follows
//! whatever Unicode version it shipped with: Node 24 and Rust 1.100 differed on
//! 41 letters on the day this was written.

/// The full upper-case mapping: `"straße"` → `"STRASSE"`.
pub fn upper(s: &str) -> String {
    s.to_uppercase()
}

/// The full lower-case mapping, with the final sigma: `"ΣΑΣ"` → `"σας"`.
pub fn lower(s: &str) -> String {
    s.to_lowercase()
}

/// The upper case of one character, which may be more than one: `ß` → `"SS"`.
pub fn upper_char(c: char) -> String {
    c.to_uppercase().collect()
}

/// The lower case of one character, alone — a sigma alone is not word-final.
pub fn lower_char(c: char) -> String {
    c.to_lowercase().collect()
}

/// The simple case folding of one code point.
///
/// For almost every letter it is the lower case. The exceptions are the letters
/// whose lower case is itself but which fold to another — the Greek final sigma
/// and the symbol variants, the long s, the micro sign, old Cyrillic variants —
/// and Cherokee, whose lower-case letters fold to the upper case. Checked against
/// `CaseFolding.txt` of Unicode 18, the version of Rust's own tables, for every
/// code point on 2026-11-23: no difference. A letter whose lower case is more
/// than one code point (`İ`) has no simple folding and stays.
pub fn fold_char(c: char) -> char {
    match c {
        'ς' => 'σ',
        'ϐ' => 'β',
        'ϑ' => 'θ',
        'ϕ' => 'φ',
        'ϖ' => 'π',
        'ϰ' => 'κ',
        'ϱ' => 'ρ',
        'ϵ' => 'ε',
        'ẛ' => 'ṡ',
        'µ' => 'μ',
        'ſ' => 's',
        '\u{0345}' | '\u{1FBE}' => 'ι',
        'ẞ' => 'ß',
        // Old Cyrillic letter variants, Greek letters with a precomposed twin,
        // a ligature, and a Latin letter added in Unicode 17.
        '\u{1C80}' => 'в',
        '\u{1C81}' => 'д',
        '\u{1C82}' => 'о',
        '\u{1C83}' => 'с',
        '\u{1C84}' | '\u{1C85}' => 'т',
        '\u{1C86}' => 'ъ',
        '\u{1C87}' => 'ѣ',
        '\u{1C88}' => '\u{A64B}',
        '\u{1FD3}' => '\u{0390}',
        '\u{1FE3}' => '\u{03B0}',
        '\u{FB05}' => '\u{FB06}',
        '\u{1DF95}' => 'ß',
        // Cherokee: the small letters fold to the capitals.
        '\u{AB70}'..='\u{ABBF}' => char::from_u32(c as u32 - 0xAB70 + 0x13A0).unwrap_or(c),
        '\u{13F8}'..='\u{13FD}' => char::from_u32(c as u32 - 8).unwrap_or(c),
        '\u{13A0}'..='\u{13F5}' => c,
        _ => {
            let mut l = c.to_lowercase();
            match (l.next(), l.next()) {
                (Some(one), None) => one,
                _ => c,
            }
        }
    }
}

/// The simple case folding of a text, point by point: the same length.
pub fn fold(s: &str) -> String {
    s.chars().map(fold_char).collect()
}

/// The extended grapheme clusters of a text, in order (UAX #29). The rules are
/// `unicode-segmentation`'s, of `UNICODE_VERSION_GRAPHEMES`: the browser engines
/// follow the same version's tables, and both pass its `GraphemeBreakTest.txt`.
pub fn graphemes(s: &str) -> Vec<&str> {
    use unicode_segmentation::UnicodeSegmentation;
    s.graphemes(true).collect()
}

/// The Unicode version of [`graphemes`]' rules.
pub const UNICODE_VERSION_GRAPHEMES: (u64, u64, u64) = unicode_segmentation::UNICODE_VERSION;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upper_and_lower_are_the_full_mappings() {
        assert_eq!(upper("straße"), "STRASSE");
        assert_eq!(lower("ТЕТРИС"), "тетрис");
        assert_eq!(upper("ёлка"), "ЁЛКА");
        assert_eq!(lower("ΣΑΣ"), "σας");
        assert_eq!(lower("Σ"), "σ");
        assert_eq!(upper("ﬁ"), "FI");
        assert_eq!(lower("İ"), "i\u{307}");
        assert_eq!(upper("i"), "I"); // no locale: never the dotted capital
        assert_eq!(upper_char('ß'), "SS");
    }

    #[test]
    fn fold_keeps_the_length_and_matches_across_case() {
        for (a, b) in [("Árbol", "áRBOL"), ("ΣΑΣ", "σας"), ("ЁЖ", "ёж"), ("ſ", "S")] {
            assert_eq!(fold(a), fold(b), "{a} {b}");
        }
        assert_eq!(fold("Straße"), "straße");
        assert_eq!(fold("Straße").chars().count(), 6);
        assert_eq!(fold_char('İ'), 'İ');
        assert_eq!(fold_char('ᏸ'), 'Ᏸ');
        assert_eq!(fold_char('ꭰ'), 'Ꭰ');
        assert_eq!(fold_char('Ꭰ'), 'Ꭰ');
        assert_eq!(fold_char('7'), '7');
    }

    #[test]
    fn graphemes_are_what_a_reader_takes_for_one() {
        let family = "👨\u{200D}👩\u{200D}👧";
        let s = format!("{family} e\u{301}🇪🇸👍🏽");
        assert_eq!(graphemes(&s), vec![family, " ", "e\u{301}", "🇪🇸", "👍🏽"]);
        assert_eq!(graphemes("한국"), vec!["한", "국"]);
        assert_eq!(graphemes("\u{1100}\u{1161}\u{11A8}"), vec!["\u{1100}\u{1161}\u{11A8}"]);
        assert_eq!(graphemes("\r\n"), vec!["\r\n"]);
        assert!(graphemes("").is_empty());
    }
}
