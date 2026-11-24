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

//!
//! The second hint is about two whole names (D-21, decided 2026-11-24): `a` and
//! `а` — Latin and Cyrillic — are two valid names and two variables, and look
//! the same. When two names of one file have the same confusable skeleton
//! (UTS #39, the `unicode-security` crate) and are not the same name, the
//! later one is named with the earlier, again in the editor only. A name after
//! `::` or `.` is left out: it names something of another module or a field,
//! like the `mat.E` that a Greek layer re-exports as `Ε` on purpose. Measured
//! over the 3784 `.zy` of the workspace on 2026-11-24: that pair aside, the
//! only look-alikes were in the probes written for the 2026-09-25 decision.

use std::collections::{HashMap, HashSet};

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
        if !matches!(token.kind, TokenKind::Ident(_) | TokenKind::HotIdent(_) | TokenKind::PreHotIdent(_)) {
            continue;
        }
        // The characters as typed: the token's name is in NFC (D-20), and a
        // name typed decomposed is not found in the source in that form.
        let Some((at, typed)) = written(source, token) else { continue };
        let chars: Vec<char> = typed.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            let Some(want) = intended(c) else { continue };
            if is_script_letter(c) {
                let before = i > 0 && is_latin_letter(chars[i - 1]);
                let after = i + 1 < chars.len() && is_latin_letter(chars[i + 1]);
                if !before && !after {
                    continue;
                }
            }
            let byte = at + chars[..i].iter().map(|ch| ch.len_utf8()).sum::<usize>();
            let range = range_at(source, byte, c.len_utf16() as u32);
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
    out.extend(look_alike_names(document));
    out
}

/// One warning for each name that looks like an earlier, different name of
/// the same file, on that name's first occurrence.
fn look_alike_names(document: &crate::document::Document) -> Vec<Diagnostic> {
    let source = document.source();
    // Each name once, in order of first occurrence, with where it is.
    let mut first: Vec<(&str, &zymbol_lexer::Token)> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut after_member = false;
    for token in document.token_list() {
        match &token.kind {
            TokenKind::LineComment(_) | TokenKind::BlockComment(_) => continue,
            TokenKind::Ident(n) | TokenKind::HotIdent(n) | TokenKind::PreHotIdent(n)
                if !n.is_empty() && !after_member =>
            {
                if seen.insert(n.as_str()) {
                    first.push((n.as_str(), token));
                }
            }
            _ => {}
        }
        after_member = matches!(token.kind, TokenKind::ScopeResolution | TokenKind::Dot);
    }
    let mut by_skeleton: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();
    for (i, &(name, token)) in first.iter().enumerate() {
        let skeleton: String = unicode_security::skeleton(name).collect();
        let Some(&j) = by_skeleton.get(&skeleton) else {
            by_skeleton.insert(skeleton, i);
            continue;
        };
        let (earlier, at) = first[j];
        let Some((byte, typed)) = written(source, token) else { continue };
        let range = range_at(source, byte, typed.encode_utf16().count() as u32);
        out.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String("confusable-name".to_string())),
            code_description: None,
            source: Some("zymbol".to_string()),
            message: look_alike_message(name, earlier, at.span.start.line),
            related_information: None,
            tags: None,
            data: None,
        });
    }
    out
}

/// `'а' looks like 'a' (line 1), a different name: it has U+0430 where 'a' has U+0061`
/// — the first code point where the two differ, or `nothing` past the end of one.
fn look_alike_message(name: &str, earlier: &str, line: u32) -> String {
    let (a, b): (Vec<char>, Vec<char>) = (name.chars().collect(), earlier.chars().collect());
    let i = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i)).unwrap_or(0);
    let shown = |c: Option<&char>| c.map_or_else(|| "nothing".to_string(), |&c| code_point(c));
    format!(
        "'{}' looks like '{}' (line {}), a different name: it has {} where '{}' has {}",
        name, earlier, line, shown(a.get(i)), earlier, shown(b.get(i))
    )
}

/// A name token as the source spells it, and the byte where that begins: the
/// token's own text without a `°` mark on either side.
fn written<'a>(source: &'a str, token: &zymbol_lexer::Token) -> Option<(usize, &'a str)> {
    let (start, end) = (token.span.start.byte_offset as usize, token.span.end.byte_offset as usize);
    let text = source.get(start..end)?;
    let typed = text.trim_matches('°');
    if typed.is_empty() {
        return None;
    }
    Some((start + text.find(typed)?, typed))
}

/// `width` UTF-16 units from byte `byte`, in the LSP's lines and columns.
fn range_at(source: &str, byte: usize, width: u32) -> Range {
    let line_start = source[..byte].rfind('\n').map_or(0, |p| p + 1);
    let line = source[..byte].matches('\n').count() as u32;
    let col = source[line_start..byte].encode_utf16().count() as u32;
    Range {
        start: Position { line, character: col },
        end: Position { line, character: col + width },
    }
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
    fn look_alike_names_warn_on_the_later_one() {
        // D-21: Latin `a`, then Cyrillic `а`.
        let w = warnings("a = 1\n\u{430} = 2\n>> a \" \" \u{430} \u{b6}\n");
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(w[0].message,
            "'\u{430}' looks like 'a' (line 1), a different name: it has U+0430 where 'a' has U+0061");
        assert_eq!(w[0].range.start, Position { line: 1, character: 0 });
        assert_eq!(w[0].code, Some(NumberOrString::String("confusable-name".into())));
    }

    #[test]
    fn a_name_typed_decomposed_is_found_where_it_is() {
        // Both hints place their warning on the name as typed, which is not
        // the NFC the token carries (D-20): Greek Η + U+0300, then Cyrillic Н + U+0300.
        let w = warnings("q\u{397}\u{300} = 1\nq\u{41D}\u{300} = 2\n");
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(w[0].range.start, Position { line: 1, character: 0 });
        assert_eq!(w[0].range.end, Position { line: 1, character: 3 });
        let w = warnings("x\u{301}\u{BA}t = 1\n");
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(w[0].range.start, Position { line: 0, character: 2 });
    }

    #[test]
    fn a_member_name_is_left_out() {
        // The Greek layer's `mat.E => Ε`: `E` names a constant of std/math.
        assert!(warnings("# m {\n    <# std/math => mat\n    #> { mat.E => \u{395} }\n}\n").is_empty());
        // The same name twice, or names that only look alike in part, are fine.
        assert!(warnings("x = 1\nx = 2\nrn = 3\n").is_empty());
    }

    #[test]
    fn plain_names_and_strings_do_not_warn() {
        assert!(warnings("total = 1\n>> \"ºtotal\" ¶\n").is_empty());
    }
}
