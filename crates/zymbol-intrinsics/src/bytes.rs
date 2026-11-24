//! A file's bytes as a text of bytes (D-22, decided 2026-11-24).
//!
//! Zymbol has no bytes type, and needs none: a byte is a `Char` from `0x00` to
//! `0xFF`, which is how the language already writes one — `0x01` is the byte
//! Ctrl+A sends, `##'n` builds one from a number and `##!c` reads it back. So
//! `io::read_bytes` gives a String with one character per byte, U+0000–U+00FF,
//! and `io::write_bytes` writes each character back as that byte: the round
//! trip returns the bytes it was given. ZyBF settled the same question for its
//! tape the same way (GAP-BF-001, GLB-109).

/// Each byte as the character of that code.
pub fn text_of(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

/// Each character as one byte, or the first that is no byte.
pub fn bytes_of(text: &str) -> Result<Vec<u8>, char> {
    text.chars().map(|c| u8::try_from(c).map_err(|_| c)).collect()
}

/// What `io::write_bytes` says about a character that is no byte. "out of
/// range" makes it a `##Range` (`zymbol_common::errkind`), as `##'` is for a
/// code with no character.
pub fn not_a_byte(c: char) -> String {
    format!(
        "io::write_bytes: '{}' is U+{:04X}, out of range for a byte: a byte is a Char from 0x00 to 0xFF",
        c, c as u32
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_byte_goes_there_and_back() {
        let all: Vec<u8> = (0..=255).collect();
        let t = text_of(&all);
        assert_eq!(t.chars().count(), 256);
        assert_eq!(bytes_of(&t), Ok(all));
        assert_eq!(bytes_of("a\u{3042}"), Err('\u{3042}'));
    }
}
