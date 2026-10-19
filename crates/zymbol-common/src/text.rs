//! A text addressed by character position, without copying it.
//!
//! A position in Zymbol counts characters — code points — and a Rust `&str` is
//! addressed by bytes. The engines used to bridge the two by copying the text
//! into a `Vec<char>`, four bytes a character, to cut a piece of ten: a slice
//! cost what the text was long, in memory as well as in time (GLB-130,
//! PERF-05 a). These borrow instead.
//!
//! On a text that is not ASCII they still walk it to find a position, as
//! `chars().count()` does: knowing where the n-th character starts without
//! walking needs the string to carry more than its bytes. The VM's `ZyStr`
//! carries them — its character count (ZYVM-028) and a crumb every 64
//! characters (GLB-139) — and no longer calls these. The tree-walker, whose
//! text is a plain `String`, does: GLB-139 stays open there, decided as debt
//! for v0.0.11.

/// How many characters `s` has.
pub fn char_len(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        s.chars().count()
    }
}

/// The characters of `s` from position `a` up to, not including, `b` —
/// 0-based, `a <= b <= char_len(s)` — borrowed from `s`.
///
/// # Panics
///
/// If the positions are out of order or past the end: the callers have
/// already checked them against the language's rule, and an index that gets
/// here unchecked is the engine's bug.
pub fn char_range(s: &str, a: usize, b: usize) -> &str {
    assert!(a <= b);
    if s.is_ascii() {
        return &s[a..b];
    }
    let mut start = None;
    let mut seen = 0;
    for (i, (at, _)) in s.char_indices().enumerate() {
        if i == a {
            start = Some(at);
        }
        if i == b {
            return &s[start.expect("a <= b, so a was passed first")..at];
        }
        seen = i + 1;
    }
    // `b` is the end of the text, or past it.
    assert!(b == seen);
    &s[start.unwrap_or(s.len())..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_and_not() {
        assert_eq!(char_len("hola"), 4);
        assert_eq!(char_len("ñandú🦆"), 6);
        assert_eq!(char_range("hola", 1, 3), "ol");
        assert_eq!(char_range("ñandú🦆", 0, 1), "ñ");
        assert_eq!(char_range("ñandú🦆", 4, 6), "ú🦆");
        assert_eq!(char_range("ñandú🦆", 2, 2), "");
        assert_eq!(char_range("ñandú🦆", 6, 6), "");
        assert_eq!(char_range("", 0, 0), "");
    }

    #[test]
    #[should_panic]
    fn past_the_end() {
        char_range("ñandú", 3, 9);
    }
}
