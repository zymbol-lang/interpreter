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
//! walking needs the string to carry more than its bytes. Two types carry
//! them — the character count (ZYVM-028) and a crumb every `CRUMB_EVERY`
//! characters (GLB-139): the VM's `ZyStr`, and [`Text`] below, the
//! tree-walker's. Both build their crumbs with [`crumbs_of`] and walk from one
//! with [`byte_from_crumb`], so the two engines find a position the same way.

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

/// One crumb every this many characters: a byte offset (8 bytes) per 64
/// characters, and a walk of at most 63 to reach any position (GLB-139).
pub const CRUMB_EVERY: usize = 64;

/// The byte where every `CRUMB_EVERY`-th character of `s` starts.
pub fn crumbs_of(s: &str) -> Vec<usize> {
    s.char_indices().step_by(CRUMB_EVERY).map(|(b, _)| b).collect()
}

/// The byte where character `i` (0-based) of `s` starts, from its crumbs; the
/// length in bytes when `i` is past the last character.
pub fn byte_from_crumb(s: &str, crumbs: &[usize], i: usize) -> usize {
    let Some(&from) = crumbs.get(i / CRUMB_EVERY) else {
        return s.len();
    };
    match s[from..].char_indices().nth(i % CRUMB_EVERY) {
        Some((b, _)) => from + b,
        None => s.len(),
    }
}

/// The tree-walker's text: shared, so reading a variable does not copy it
/// (ZYTW-016), and addressed by character through a kept count and crumbs, so
/// `s[i]` and a slice do not walk it from the start (GLB-139).
///
/// It was a plain `String`, cloned on every read: a loop of `s$#` over a text
/// of 8 000 characters moved 128 MB where the VM moved 2.6, and a loop of
/// `s[i]` over a text that is not ASCII was quadratic. The VM's `ZyStr` solved
/// both for the VM; this is the same model without the inline form and the
/// unsafe code that earns its keep in a bytecode loop.
///
/// Nothing changes the text behind an `Rc` that others hold. [`Text::edit`]
/// writes in place only for a sole owner — copying first otherwise — and
/// forgets the count and the crumbs, which belonged to the text before.
#[derive(Clone, Default)]
pub struct Text(std::rc::Rc<TextInner>);

#[derive(Default)]
struct TextInner {
    s: String,
    chars: std::cell::Cell<Option<usize>>,
    crumbs: std::cell::OnceCell<Box<Vec<usize>>>,
}

impl Clone for TextInner {
    /// The copy `Rc::make_mut` takes before a write: the text, and nothing it
    /// knew about itself.
    fn clone(&self) -> Self {
        TextInner { s: self.s.clone(), ..TextInner::default() }
    }
}

impl Text {
    pub fn new(s: String) -> Self {
        Text(std::rc::Rc::new(TextInner { s, ..TextInner::default() }))
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0.s
    }

    /// How many characters — code points — the text has. Counted once.
    pub fn char_len(&self) -> usize {
        if let Some(n) = self.0.chars.get() {
            return n;
        }
        let n = char_len(&self.0.s);
        self.0.chars.set(Some(n));
        n
    }

    /// The byte where character `i` (0-based) starts; the length in bytes when
    /// `i` is the count or past it. By byte on an ASCII text; on any other,
    /// from the nearest crumb, built the first time a position is asked.
    pub fn char_byte(&self, i: usize) -> usize {
        let s = &self.0.s;
        let n = self.char_len();
        if i >= n {
            return s.len();
        }
        if n == s.len() {
            return i;
        }
        let crumbs = self.0.crumbs.get_or_init(|| Box::new(crumbs_of(s)));
        byte_from_crumb(s, crumbs, i)
    }

    /// Character `i` (0-based), if there is one.
    pub fn char_at(&self, i: usize) -> Option<char> {
        if i >= self.char_len() {
            return None;
        }
        self.0.s[self.char_byte(i)..].chars().next()
    }

    /// The characters from position `a` up to, not including, `b` — 0-based,
    /// `a <= b <= char_len()`, as [`char_range`] takes them.
    pub fn char_range(&self, a: usize, b: usize) -> &str {
        assert!(a <= b && b <= self.char_len());
        &self.0.s[self.char_byte(a)..self.char_byte(b)]
    }

    /// The `String` to write into: the same one when this is its sole owner,
    /// a copy otherwise. The count and the crumbs are forgotten either way.
    pub fn edit(&mut self) -> &mut String {
        let inner = std::rc::Rc::make_mut(&mut self.0);
        inner.chars.set(None);
        inner.crumbs = std::cell::OnceCell::new();
        &mut inner.s
    }

    /// The `String`, without a copy when this is its sole owner.
    pub fn into_string(self) -> String {
        match std::rc::Rc::try_unwrap(self.0) {
            Ok(inner) => inner.s,
            Err(rc) => rc.s.clone(),
        }
    }

    /// Whether the two share one buffer.
    pub fn ptr_eq(a: &Text, b: &Text) -> bool {
        std::rc::Rc::ptr_eq(&a.0, &b.0)
    }
}

impl std::ops::Deref for Text {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        &self.0.s
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        &self.0.s
    }
}

impl std::borrow::Borrow<str> for Text {
    fn borrow(&self) -> &str {
        &self.0.s
    }
}

impl From<String> for Text {
    fn from(s: String) -> Self {
        Text::new(s)
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Text::new(s.to_string())
    }
}

impl From<&String> for Text {
    fn from(s: &String) -> Self {
        Text::new(s.clone())
    }
}

impl From<Text> for String {
    fn from(t: Text) -> String {
        t.into_string()
    }
}

impl FromIterator<char> for Text {
    fn from_iter<I: IntoIterator<Item = char>>(it: I) -> Self {
        Text::new(it.into_iter().collect())
    }
}

impl<'a> FromIterator<&'a char> for Text {
    fn from_iter<I: IntoIterator<Item = &'a char>>(it: I) -> Self {
        Text::new(it.into_iter().collect())
    }
}

impl<'a> FromIterator<&'a str> for Text {
    fn from_iter<I: IntoIterator<Item = &'a str>>(it: I) -> Self {
        Text::new(it.into_iter().collect())
    }
}

impl FromIterator<String> for Text {
    fn from_iter<I: IntoIterator<Item = String>>(it: I) -> Self {
        Text::new(it.into_iter().collect())
    }
}

impl AsRef<std::path::Path> for Text {
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(&self.0.s)
    }
}

impl AsRef<std::ffi::OsStr> for Text {
    fn as_ref(&self) -> &std::ffi::OsStr {
        std::ffi::OsStr::new(&self.0.s)
    }
}

impl std::fmt::Display for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0.s, f)
    }
}

/// As a `String` prints with `{:?}`, so any output built from a value's
/// `Debug` reads as it did.
impl std::fmt::Debug for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0.s, f)
    }
}

impl PartialEq for Text {
    fn eq(&self, other: &Text) -> bool {
        self.0.s == other.0.s
    }
}
impl Eq for Text {}

impl PartialEq<str> for Text {
    fn eq(&self, other: &str) -> bool {
        self.0.s == other
    }
}
impl PartialEq<&str> for Text {
    fn eq(&self, other: &&str) -> bool {
        self.0.s == *other
    }
}
impl PartialEq<String> for Text {
    fn eq(&self, other: &String) -> bool {
        &self.0.s == other
    }
}

impl PartialOrd for Text {
    fn partial_cmp(&self, other: &Text) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Text {
    fn cmp(&self, other: &Text) -> std::cmp::Ordering {
        self.0.s.cmp(&other.0.s)
    }
}

impl std::hash::Hash for Text {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.0.s.hash(h)
    }
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

    #[test]
    fn text_positions_agree_with_a_walk_from_the_start() {
        // Characters of one to four bytes, across several crumbs.
        let src: String = "aé€🦆".chars().cycle().take(64 * 5 + 7).collect();
        let t = Text::from(src.clone());
        let chars: Vec<char> = src.chars().collect();
        assert_eq!(t.char_len(), chars.len());
        for (i, c) in chars.iter().enumerate() {
            assert_eq!(t.char_at(i), Some(*c), "position {i}");
        }
        assert_eq!(t.char_at(chars.len()), None);
        for (a, b) in [(0, 0), (0, 64), (63, 65), (100, 300), (5, chars.len())] {
            let want: String = chars[a..b].iter().collect();
            assert_eq!(t.char_range(a, b), want, "{a}..{b}");
            assert_eq!(t.char_range(a, b), char_range(&src, a, b));
        }
    }

    #[test]
    fn an_edit_forgets_what_the_text_knew_and_never_reaches_a_sharer() {
        let mut t = Text::from("ñandú");
        assert_eq!(t.char_len(), 5);
        assert_eq!(t.char_at(4), Some('ú'));
        let kept = t.clone();
        t.edit().push_str("🦆🦆");
        assert_eq!(t.char_len(), 7);
        assert_eq!(t.char_at(6), Some('🦆'));
        assert_eq!(kept.as_str(), "ñandú");
        assert_eq!(kept.char_len(), 5);
        assert!(!Text::ptr_eq(&t, &kept));
        // A sole owner is written in place, and asked again from scratch.
        let mut sole = t;
        sole.edit().push('x');
        assert_eq!(sole.char_len(), 8);
        assert_eq!(sole.char_at(7), Some('x'));
    }
}
