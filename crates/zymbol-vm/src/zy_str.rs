//! ZyStr — 8-byte string representation combining SSO and Rc<String>.
//!
//! Sprint 5G: eliminate heap allocations for short strings (≤ 7 bytes).
//!
//! # Encoding (little-endian, 8 bytes)
//!
//! ```text
//! Inline  (byte[7] bit 7 == 1):
//!   byte[0..len]  — UTF-8 string data  (up to 7 bytes)
//!   byte[7]       — 0x80 | len         (len: 0–7)
//!   byte[len..7]  — zero padding
//!
//! Heap  (byte[7] bit 7 == 0):
//!   bytes[0..8] as u64 (LE) — raw pointer from Rc::into_raw(), zero-extended
//!   On x86-64 / arm64, user-space pointers have bit 63 == 0; on a 32-bit
//!   platform (wasm32) the pointer fills bytes[0..4] and bytes[4..8] are
//!   zero. Either way byte[7] has bit 7 clear, so a heap value is always
//!   distinguishable from an inline one.
//! ```
//!
//! # Safety
//!
//! Valid on 64-bit platforms where user-space virtual addresses have bit 63 == 0
//! (x86-64, arm64 with 48-bit VA) and on every 32-bit one, whose pointers never
//! reach byte[7]. The pointer width is asserted at compile time. 32-bit is there
//! for wasm32-unknown-unknown, the WebAssembly browsers run (ROADMAP_V0011_ES.md
//! § 3.4, decided 2026-10-20); until then this said "requires a 64-bit platform".

use std::cell::{Cell, OnceCell};
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::{fmt, hash};

const _ASSERT_POINTER_WIDTH: () = {
    assert!(
        std::mem::size_of::<usize>() == 8 || std::mem::size_of::<usize>() == 4,
        "ZyStr requires a 32-bit or 64-bit platform"
    );
};

const INLINE_FLAG: u8 = 0x80;
const LEN_MASK: u8 = 0x7F;
const MAX_INLINE: usize = 7;

/// 8-byte string value with inline SSO for strings up to 7 bytes.
#[repr(transparent)]
pub struct ZyStr([u8; 8]);

/// What a heap string points to: the text, and how many characters it has —
/// unknown until someone asks, then kept (PERF-05 b, 2026-10-17). A position
/// in Zymbol counts characters and a `str` is addressed by bytes, so `s$#`,
/// `s[i]` and a slice walked the text every time; the count is found once, and
/// a text whose count equals its length is ASCII, where a position is a byte.
///
/// Nothing changes the text behind an `Rc` that others hold: a write in place
/// (`try_into_string`) takes the `String` out of a sole owner, and the new
/// value is a new `Heap` with its count unknown again.
///
/// A text that is not ASCII also keeps, once a position is asked of it, the
/// byte where every `CRUMB_EVERY`-th character starts (GLB-139): `s[i]` and a
/// slice jump to the crumb before their position and walk fewer than
/// `CRUMB_EVERY` characters from there, instead of walking from the start —
/// which made a loop over the positions of a long text quadratic. Boxed, so
/// the cell of a text nobody indexes costs one word.
struct Heap {
    s: String,
    chars: Cell<usize>,
    crumbs: OnceCell<Box<Vec<usize>>>,
}

const UNKNOWN: usize = usize::MAX;

/// One crumb every this many characters: a byte offset (8 bytes) per 64
/// characters, and a walk of at most 63 to reach any position.
const CRUMB_EVERY: usize = 64;

impl Heap {
    #[inline]
    fn new(s: String) -> Self {
        Heap { s, chars: Cell::new(UNKNOWN), crumbs: OnceCell::new() }
    }
}

impl ZyStr {
    // ── constructors ─────────────────────────────��────────────────────────────

    /// Create from an owned String. Strings ≤ 7 bytes go inline; longer strings
    /// wrap the String directly in Rc (single allocation, no extra copy).
    #[inline]
    pub fn new(s: String) -> Self {
        let len = s.len();
        if len <= MAX_INLINE {
            let mut buf = [0u8; 8];
            buf[..len].copy_from_slice(s.as_bytes());
            buf[7] = INLINE_FLAG | (len as u8);
            ZyStr(buf)
        } else {
            let ptr = Rc::into_raw(Rc::new(Heap::new(s))) as u64;
            ZyStr(ptr.to_le_bytes())
        }
    }

    /// Create from a string slice.
    #[inline]
    pub fn from_str_ref(s: &str) -> Self {
        let len = s.len();
        if len <= MAX_INLINE {
            let mut buf = [0u8; 8];
            buf[..len].copy_from_slice(s.as_bytes());
            buf[7] = INLINE_FLAG | (len as u8);
            ZyStr(buf)
        } else {
            let ptr = Rc::into_raw(Rc::new(Heap::new(s.to_string()))) as u64;
            ZyStr(ptr.to_le_bytes())
        }
    }

    /// Consume self and return an owned String, reusing the heap buffer when
    /// the Rc has exactly one strong reference (avoids allocation in concat loops).
    #[inline]
    pub fn try_into_string(self) -> String {
        if self.is_inline() {
            self.as_str().to_string()
        } else {
            let ptr = self.heap_ptr();
            unsafe {
                // Peek at strong count without constructing an Rc.
                let rc = ManuallyDrop::new(Rc::from_raw(ptr));
                if Rc::strong_count(&*rc) == 1 {
                    // Exclusive ownership — unwrap the String without copying.
                    // Forget self so Drop doesn't call Rc::from_raw a second time.
                    let owned_rc = ManuallyDrop::into_inner(rc);
                    std::mem::forget(self);
                    // strong_count == 1, so try_unwrap always succeeds.
                    match Rc::try_unwrap(owned_rc) {
                        Ok(heap) => heap.s,
                        Err(rc) => rc.s.clone(),
                    }
                } else {
                    // Shared — must copy.
                    rc.s.clone()
                }
            }
        }
    }

    /// Wrap an existing Rc<String>: its text, moved out when this is the
    /// only holder.
    #[inline]
    pub fn from_rc(rc: Rc<String>) -> Self {
        Self::new(Rc::try_unwrap(rc).unwrap_or_else(|rc| (*rc).clone()))
    }

    /// How many characters — code points — the text has. Counted once and
    /// kept, for a heap string; an inline one is at most seven bytes.
    #[inline]
    pub fn char_len(&self) -> usize {
        if self.is_inline() {
            return self.as_str().chars().count();
        }
        // SAFETY: pointer from Rc::into_raw, alive while self is alive.
        let heap = unsafe { &*self.heap_ptr() };
        let known = heap.chars.get();
        if known != UNKNOWN {
            return known;
        }
        let n = if heap.s.is_ascii() { heap.s.len() } else { heap.s.chars().count() };
        heap.chars.set(n);
        n
    }

    /// Whether every character is one byte — then a position is a byte.
    #[inline]
    pub fn is_ascii_text(&self) -> bool {
        self.char_len() == self.len()
    }

    /// The byte where character `i` (0-based) starts, and the length in bytes
    /// when `i` is the count or past it. By byte on an ASCII text; on any other,
    /// from the nearest crumb (GLB-139), built the first time it is needed.
    #[inline]
    pub fn char_byte(&self, i: usize) -> usize {
        let n = self.char_len();
        if i >= n {
            return self.len();
        }
        if n == self.len() {
            return i;
        }
        let s = self.as_str();
        let (from, skip) = if self.is_inline() {
            (0, i)
        } else {
            // SAFETY: pointer from Rc::into_raw, alive while self is alive.
            let heap = unsafe { &*self.heap_ptr() };
            let crumbs = heap.crumbs.get_or_init(|| {
                Box::new(heap.s.char_indices().step_by(CRUMB_EVERY).map(|(b, _)| b).collect())
            });
            (crumbs[i / CRUMB_EVERY], i % CRUMB_EVERY)
        };
        match s[from..].char_indices().nth(skip) {
            Some((b, _)) => from + b,
            None => s.len(),
        }
    }

    /// Whether the two hold the same heap buffer — `Rc::ptr_eq` for a string.
    /// Two inline strings share nothing, so they never do.
    #[inline]
    pub fn shares_buffer(&self, other: &ZyStr) -> bool {
        !self.is_inline() && self.0 == other.0
    }

    // ── accessors ─────────────────────────────────────────────────────────────

    #[inline]
    fn is_inline(&self) -> bool {
        self.0[7] & INLINE_FLAG != 0
    }

    #[inline]
    fn inline_len(&self) -> usize {
        (self.0[7] & LEN_MASK) as usize
    }

    /// Borrow contents as `&str`.
    #[inline]
    pub fn as_str(&self) -> &str {
        if self.is_inline() {
            let len = self.inline_len();
            // SAFETY: bytes[0..len] contain valid UTF-8 copied from a valid &str.
            unsafe { std::str::from_utf8_unchecked(&self.0[..len]) }
        } else {
            // SAFETY: pointer from Rc::into_raw, allocation alive while self is alive.
            unsafe { (*self.heap_ptr()).s.as_str() }
        }
    }

    #[inline]
    fn heap_ptr(&self) -> *const Heap {
        u64::from_le_bytes(self.0) as *const Heap
    }

    /// The text in an `Rc<String>` of its own (a copy: the heap holds more
    /// than the text since PERF-05 b).
    #[inline]
    pub fn to_rc(&self) -> Rc<String> {
        Rc::new(self.as_str().to_string())
    }
}

// ── Clone / Drop ─────────────────────────────────────────────────────���────────

impl Clone for ZyStr {
    #[inline]
    fn clone(&self) -> Self {
        if self.is_inline() {
            ZyStr(self.0)
        } else {
            // SAFETY: pointer from Rc::into_raw, still alive. increment_strong_count
            // adds one reference without constructing an Rc value (no extra allocation).
            unsafe { Rc::increment_strong_count(self.heap_ptr()); }
            ZyStr(self.0)
        }
    }
}

impl Drop for ZyStr {
    #[inline]
    fn drop(&mut self) {
        if !self.is_inline() {
            unsafe { drop(Rc::from_raw(self.heap_ptr())); }
        }
    }
}

// ── Deref → str ───────────────────────────────────────────────────────────────

impl std::ops::Deref for ZyStr {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

// ── Comparisons ───────────────────────────────────────────────────────────────

impl PartialEq for ZyStr {
    fn eq(&self, other: &Self) -> bool { self.as_str() == other.as_str() }
}
impl Eq for ZyStr {}

impl PartialOrd for ZyStr {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ZyStr {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl hash::Hash for ZyStr {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

// ── Formatting ────────────────────────────────────────────────────────────────

impl fmt::Display for ZyStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for ZyStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

// ── Conversions ───────────────────────────────────────────────────────────────

impl From<String> for ZyStr {
    fn from(s: String) -> Self { ZyStr::new(s) }
}
impl From<&str> for ZyStr {
    fn from(s: &str) -> Self { ZyStr::from_str_ref(s) }
}
impl From<Rc<String>> for ZyStr {
    fn from(rc: Rc<String>) -> Self { ZyStr::from_rc(rc) }
}
impl AsRef<str> for ZyStr {
    fn as_ref(&self) -> &str { self.as_str() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crumbs_of_a_text_nobody_indexes_cost_one_word() {
        // In words, so it says the same on 64 bits (8, 40) and on wasm32 (4, 20):
        // the String's three, the character count, and the crumbs' one.
        let word = std::mem::size_of::<usize>();
        assert_eq!(std::mem::size_of::<OnceCell<Box<Vec<usize>>>>(), word);
        assert_eq!(std::mem::size_of::<Heap>(), 5 * word);
    }

    #[test]
    fn char_byte_agrees_with_a_walk_from_the_start() {
        // Every position of texts whose count falls on, before and after a
        // crumb boundary, with characters of one to four bytes, inline and on
        // the heap, ASCII and not — against `char_indices` from the start.
        let mixed = "añ€🦆";
        let mut texts: Vec<String> = vec!["ñ".into(), "añ€".into(), "abc".into(), String::new()];
        for n in [63usize, 64, 65, 127, 128, 129, 300] {
            texts.push(mixed.chars().cycle().take(n).collect());
            texts.push("x".repeat(n - 1) + "é");
        }
        for t in texts {
            let z = ZyStr::new(t.clone());
            let starts: Vec<usize> = t.char_indices().map(|(b, _)| b).collect();
            for (i, &b) in starts.iter().enumerate() {
                assert_eq!(z.char_byte(i), b, "text of {} chars, position {}", starts.len(), i);
            }
            assert_eq!(z.char_byte(starts.len()), t.len());
            assert_eq!(z.char_byte(starts.len() + 5), t.len());
        }
    }

    #[test]
    fn char_len_is_counted_once_and_stays_true() {
        let s = ZyStr::new("ñandú y un texto largo".to_string());
        assert_eq!(s.char_len(), 22);
        assert_eq!(s.char_len(), 22);
        assert!(!s.is_ascii_text());
        let a = ZyStr::new("ascii text, long".to_string());
        assert!(a.is_ascii_text());
        assert_eq!(ZyStr::new("ñ".to_string()).char_len(), 1);
        // grown in place: a new heap, counted again
        let mut grown = s.try_into_string();
        grown.push_str("éé");
        assert_eq!(ZyStr::new(grown).char_len(), 24);
    }

    #[test]
    fn size_is_8_bytes() {
        assert_eq!(std::mem::size_of::<ZyStr>(), 8);
    }

    #[test]
    fn inline_empty() {
        let z = ZyStr::from_str_ref("");
        assert!(z.is_inline());
        assert_eq!(z.as_str(), "");
    }

    #[test]
    fn inline_max_7() {
        let z = ZyStr::from_str_ref("abcdefg");
        assert!(z.is_inline());
        assert_eq!(z.as_str(), "abcdefg");
    }

    #[test]
    fn heap_at_8() {
        let z = ZyStr::from_str_ref("abcdefgh");
        assert!(!z.is_inline());
        assert_eq!(z.as_str(), "abcdefgh");
    }

    #[test]
    fn clone_inline() {
        let a = ZyStr::from_str_ref("hi");
        let b = a.clone();
        assert_eq!(a.as_str(), b.as_str());
    }

    #[test]
    fn clone_heap_no_double_free() {
        let a = ZyStr::new("a longer string, definitely heap".to_string());
        let b = a.clone();
        assert_eq!(a.as_str(), b.as_str());
        // both drop here — should not double-free
    }

    #[test]
    fn unicode_2byte_inline() {
        // "é" = 2 bytes → inline (≤ 7)
        let z = ZyStr::from_str_ref("ééé");
        assert!(z.is_inline());
        assert_eq!(z.as_str(), "ééé");
    }

    #[test]
    fn unicode_heap_boundary() {
        // 4 × "é" = 8 bytes → heap
        let z = ZyStr::from_str_ref("éééé");
        assert!(!z.is_inline());
        assert_eq!(z.as_str(), "éééé");
    }

    #[test]
    fn deref_methods_work() {
        let z = ZyStr::from_str_ref("hello");
        assert!(z.contains('e'));
        assert_eq!(z.len(), 5);
        assert!(z.is_ascii());
    }

    #[test]
    fn to_rc_inline() {
        let z = ZyStr::from_str_ref("hi");
        let rc = z.to_rc();
        assert_eq!(rc.as_str(), "hi");
    }

    #[test]
    fn to_rc_heap() {
        let s = "a longer string beyond seven bytes".to_string();
        let z = ZyStr::new(s.clone());
        let rc = z.to_rc();
        assert_eq!(rc.as_str(), s.as_str());
    }
}
