//! std/term — terminal display metrics for Zymbol-Lang.
//!
//! These functions answer a question about the *screen*, not about the string's
//! content: how many terminal columns does the text occupy when drawn? CJK
//! ideographs, kana, hangul and most emoji take two columns each, so grapheme
//! count (`$#`) is not column count. Anything that operates on the *content* of
//! a string — split (`$/`), slice (`$[..]`), replace (`$~~`), repeat (`$*`) — is
//! a language symbol and never lives here.
//!
//! Width is computed with the `unicode-width` tables (the same the wider Rust
//! ecosystem uses) over grapheme clusters, so a multi-code-point cluster such as
//! an emoji with a variation selector is measured as one unit.
//!
//! Names are the international (English) forms; a project wanting localized
//! names re-exports them through the i18n three-layer pattern.

use crate::{FunctionDef, Result, RuntimeError, Value};
use std::collections::HashMap;
use std::rc::Rc;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use zymbol_span::Span;

/// Display width in columns of a string (its grapheme clusters) or a single
/// character. Control characters contribute 0.
pub(crate) fn width_of(v: &Value) -> Option<i64> {
    match v {
        Value::String(s) => Some(UnicodeWidthStr::width(s.as_str()) as i64),
        Value::Char(c)   => Some(UnicodeWidthChar::width(*c).unwrap_or(0) as i64),
        _ => None,
    }
}

/// Pad `s` with spaces to exactly `cols` columns. A string already at least
/// `cols` wide is returned untouched — truncation is `truncate`'s job, and
/// silently cutting a label is worse than a one-column overflow.
pub(crate) fn pad(s: &str, cols: i64, on_left: bool) -> String {
    let deficit = cols - UnicodeWidthStr::width(s) as i64;
    if deficit <= 0 {
        return s.to_string();
    }
    let spaces = " ".repeat(deficit as usize);
    if on_left {
        format!("{spaces}{s}")
    } else {
        format!("{s}{spaces}")
    }
}

/// Centre `s` within `cols` columns; a spare column goes to the right.
pub(crate) fn center(s: &str, cols: i64) -> String {
    let deficit = cols - UnicodeWidthStr::width(s) as i64;
    if deficit <= 0 {
        return s.to_string();
    }
    let left = deficit / 2;
    let right = deficit - left;
    format!("{}{s}{}", " ".repeat(left as usize), " ".repeat(right as usize))
}

/// Truncate `s` to at most `cols` columns, never splitting a grapheme cluster.
pub(crate) fn truncate(s: &str, cols: i64) -> String {
    if UnicodeWidthStr::width(s) as i64 <= cols {
        return s.to_string();
    }
    let mut used = 0i64;
    let mut out = String::new();
    for g in s.graphemes(true) {
        let w = UnicodeWidthStr::width(g) as i64;
        if used + w > cols {
            break;
        }
        out.push_str(g);
        used += w;
    }
    out
}

// --- Native functions --------------------------------------------------------

fn err_width(args: &[Value], span: Span) -> RuntimeError {
    let got = args.first().map(|v| v.type_name()).unwrap_or("nothing");
    RuntimeError::Generic {
        message: format!("term::width: expected a String or Char, got {got}"),
        span,
    }
}

fn err_pad(fname: &str, span: Span) -> RuntimeError {
    RuntimeError::Generic {
        message: format!("term::{fname}: expected (String, ###)"),
        span,
    }
}

fn term_width(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(width_of) {
        Some(w) => Ok(Value::Int(w)),
        None => Err(err_width(&args, span)),
    }
}

fn term_pad_left(args: Vec<Value>, span: Span) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::String(s)), Some(Value::Int(n))) => Ok(Value::String(pad(s, *n, true))),
        _ => Err(err_pad("pad_left", span)),
    }
}

fn term_pad_right(args: Vec<Value>, span: Span) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::String(s)), Some(Value::Int(n))) => Ok(Value::String(pad(s, *n, false))),
        _ => Err(err_pad("pad_right", span)),
    }
}

fn term_center(args: Vec<Value>, span: Span) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::String(s)), Some(Value::Int(n))) => Ok(Value::String(center(s, *n))),
        _ => Err(err_pad("center", span)),
    }
}

fn term_truncate(args: Vec<Value>, span: Span) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::String(s)), Some(Value::Int(n))) => Ok(Value::String(truncate(s, *n))),
        _ => Err(err_pad("truncate", span)),
    }
}

// --- Registry ----------------------------------------------------------------

pub(crate) fn register() -> HashMap<String, Rc<FunctionDef>> {
    let mut m: HashMap<String, Rc<FunctionDef>> = HashMap::new();

    macro_rules! native {
        ($name:literal, $arity:expr, $fn:expr) => {
            m.insert($name.into(), Rc::new(FunctionDef::Native {
                name: $name, arity: $arity, func: $fn,
            }));
        };
    }

    native!("width",     1, term_width);
    native!("pad_left",  2, term_pad_left);
    native!("pad_right", 2, term_pad_right);
    native!("center",    2, term_center);
    native!("truncate",  2, term_truncate);

    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_counts_columns_not_graphemes() {
        assert_eq!(width_of(&Value::String("abc".into())), Some(3));
        assert_eq!(width_of(&Value::String("手番".into())), Some(4)); // 2 wide glyphs
        assert_eq!(width_of(&Value::String("go碁🌑".into())), Some(6)); // 1+1+2+2
        assert_eq!(width_of(&Value::String(String::new())), Some(0));
        assert_eq!(width_of(&Value::Char('あ')), Some(2));
        assert_eq!(width_of(&Value::Char('A')), Some(1));
        assert_eq!(width_of(&Value::Int(9)), None);
    }

    #[test]
    fn pad_reaches_exact_column_count() {
        assert_eq!(pad("x", 5, false), "x    ");
        assert_eq!(pad("x", 5, true), "    x");
        // wide content is measured in columns, not chars
        assert_eq!(pad("手番", 6, false), "手番  ");
        // already at or over width → untouched
        assert_eq!(pad("toolong", 3, false), "toolong");
        assert_eq!(pad("ab", 2, false), "ab");
    }

    #[test]
    fn center_gives_spare_column_to_the_right() {
        assert_eq!(center("go", 10), "    go    ");
        assert_eq!(center("x", 4), " x  ");
        assert_eq!(center("x", 1), "x");
    }

    #[test]
    fn truncate_never_splits_a_wide_glyph() {
        assert_eq!(truncate("形勢判断形勢", 6), "形勢判"); // 3 ideographs = 6 cols
        assert_eq!(truncate("🌑🌕", 1), "");             // one emoji is 2 cols
        assert_eq!(truncate("ab", 0), "");
        assert_eq!(truncate("short", 20), "short");
    }
}
