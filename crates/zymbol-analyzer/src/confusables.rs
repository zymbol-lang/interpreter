//! Characters in a name that look like one of Zymbol's symbols.
//!
//! Decided by the author on 2026-09-25: every one of these stays a valid name
//! character — a developer is free to write a name in any script, even one that
//! looks encrypted — and the EDITOR says, as a warning, which symbol the name
//! may have meant. Only the editor: `zymbol check` and `zymbol run` say nothing,
//! so a program that uses one on purpose is never refused or nagged at in a
//! terminal. The playground panel gives the same hint (`confusableHints` in
//! web/src/zymbol/zymbol.js, same table).
//!
//! The case that asked for it: a Spanish keyboard types `º` (U+00BA) on the key
//! where `°` (U+00B0) was meant, `ºtotal += i` failed with `undefined variable
//! 'ºtotal'`, and nothing in the editor told the two characters apart.
//!
//! Measured over the 2913 `.zy` of the workspace before it was written: one name
//! would warn, a probe written to test exactly this. `·` is not in the table
//! because it is a letter in Catalan (`col·lecció`). The Cyrillic `о` and the
//! Greek `ο` are letters of their scripts, so they only warn next to a Latin
//! letter (`contadοr`); 1736 names hold one, and none of them does.

use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range};
use zymbol_lexer::TokenKind;

/// Each character, and the symbol or letter it is taken for.
pub const CONFUSABLES: &[(char, char)] = &[
    ('º', '°'), ('ª', '°'), ('ᵒ', '°'), ('ₒ', '°'), ('∘', '°'), ('◦', '°'),
    ('ⁿ', 'n'),
    ('＠', '@'), ('？', '?'), ('！', '!'), ('＃', '#'),
    ('¿', '?'), ('¡', '!'),
    ('§', '$'),
    ('•', '.'),
    ('¹', '1'),
    ('о', 'o'), ('ο', 'o'),
];

/// The two that are letters of another script: they warn only when they touch
/// a Latin letter, which is where one is mistaken for the Latin `o`.
fn is_script_letter(c: char) -> bool {
    matches!(c, 'о' | 'ο')
}

fn is_latin_letter(c: char) -> bool {
    c.is_ascii_alphabetic()
        || ('\u{00C0}'..='\u{024F}').contains(&c) && c != '×' && c != '÷'
        || ('\u{1E00}'..='\u{1EFF}').contains(&c)
}

/// `U+00BA`, the way the message names a character.
fn code_point(c: char) -> String {
    format!("U+{:04X}", c as u32)
}

fn intended(c: char) -> Option<char> {
    CONFUSABLES.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
}

/// One warning per confusable character in a name, placed on that character.
pub fn collect(document: &crate::document::Document) -> Vec<Diagnostic> {
    let source = document.source();
    let mut out = Vec::new();
    for token in document.token_list() {
        let name = match &token.kind {
            TokenKind::Ident(n) | TokenKind::HotIdent(n) | TokenKind::PreHotIdent(n) => n,
            _ => continue,
        };
        let chars: Vec<char> = name.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            let Some(want) = intended(c) else { continue };
            if is_script_letter(c) {
                let before = i > 0 && is_latin_letter(chars[i - 1]);
                let after = i + 1 < chars.len() && is_latin_letter(chars[i + 1]);
                if !before && !after {
                    continue;
                }
            }
            let Some(range) = char_range(source, token.span.start.byte_offset as usize,
                                         token.span.end.byte_offset as usize, c, i, &chars)
            else { continue };
            let meant: String = chars.iter().enumerate()
                .map(|(j, &d)| if j == i { want } else { d })
                .collect();
            out.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String("confusable-character".to_string())),
                code_description: None,
                source: Some("zymbol".to_string()),
                message: format!(
                    "'{}' ({}) looks like '{}' ({}) — did you mean '{}'?",
                    c, code_point(c), want, code_point(want), meant
                ),
                related_information: None,
                tags: None,
                data: None,
            });
        }
    }
    out
}

/// Where the `nth` character of the name sits, in the LSP's UTF-16 columns. The
/// name is found inside the token's own bytes, which may also hold a `°` mark.
fn char_range(source: &str, start: usize, end: usize, c: char, nth: usize,
              chars: &[char]) -> Option<Range> {
    let text = source.get(start..end)?;
    let name: String = chars.iter().collect();
    let name_at = start + text.find(&name)?;
    let byte = name_at + chars[..nth].iter().map(|ch| ch.len_utf8()).sum::<usize>();
    let line_start = source[..byte].rfind('\n').map_or(0, |p| p + 1);
    let line = source[..byte].matches('\n').count() as u32;
    let col = source[line_start..byte].encode_utf16().count() as u32;
    Some(Range {
        start: Position { line, character: col },
        end: Position { line, character: col + c.len_utf16() as u32 },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use zymbol_span::FileId;

    fn warnings(src: &str) -> Vec<Diagnostic> {
        let doc = Document::new("file:///t.zy".into(), src.to_string(), 1, FileId(0));
        collect(&doc)
    }

    #[test]
    fn ordinal_for_degree_is_named() {
        let w = warnings("@ i:1..3 {\n    ºtotal += i\n}\n");
        assert_eq!(w.len(), 1);
        assert!(w[0].message.contains("did you mean '°total'"), "{}", w[0].message);
        assert_eq!(w[0].range.start, Position { line: 1, character: 4 });
    }

    #[test]
    fn greek_o_inside_latin_warns_and_greek_word_does_not() {
        assert_eq!(warnings("contadοr = 1\n").len(), 1);
        assert!(warnings("λόγος = 1\n").is_empty());
        assert!(warnings("πλ_el = 1\n").is_empty());
    }

    #[test]
    fn catalan_middle_dot_is_left_alone() {
        assert!(warnings("col·lecció = 1\n").is_empty());
    }

    #[test]
    fn plain_names_and_strings_do_not_warn() {
        assert!(warnings("total = 1\n>> \"ºtotal\" ¶\n").is_empty());
    }
}
