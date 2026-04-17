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
//!   bytes[0..8] as u64 (LE) — raw pointer from Rc::into_raw()
//!   On x86-64 / arm64, user-space pointers have bit 63 == 0,
//!   so they are always distinguishable from inline values.
//! ```
//!
//! # Safety
//!
//! Valid only on platforms where user-space virtual addresses have bit 63 == 0
//! (x86-64, arm64 with 48-bit VA). Asserted at compile time.

use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::{fmt, hash};

const _ASSERT_64BIT: () = {
    assert!(
        std::mem::size_of::<usize>() == 8,
        "ZyStr requires a 64-bit platform"
    );
};

const INLINE_FLAG: u8 = 0x80;
const LEN_MASK: u8 = 0x7F;
const MAX_INLINE: usize = 7;

/// 8-byte string value with inline SSO for strings up to 7 bytes.
#[repr(transparent)]
pub struct ZyStr([u8; 8]);

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
            let ptr = Rc::into_raw(Rc::new(s)) as u64;
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
            let ptr = Rc::into_raw(Rc::new(s.to_string())) as u64;
            ZyStr(ptr.to_le_bytes())
        }
    }

    /// Wrap an existing Rc<String> (takes ownership).
    #[inline]
    pub fn from_rc(rc: Rc<String>) -> Self {
        if rc.len() <= MAX_INLINE {
            let z = Self::from_str_ref(rc.as_str());
            drop(rc);
            z
        } else {
            let ptr = Rc::into_raw(rc) as u64;
            ZyStr(ptr.to_le_bytes())
        }
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
            unsafe { (*self.heap_ptr()).as_str() }
        }
    }

    #[inline]
    fn heap_ptr(&self) -> *const String {
        u64::from_le_bytes(self.0) as *const String
    }

    /// Clone the underlying Rc<String> (for callers that need an owned Rc).
    #[inline]
    pub fn to_rc(&self) -> Rc<String> {
        if self.is_inline() {
            Rc::new(self.as_str().to_string())
        } else {
            unsafe {
                let rc = ManuallyDrop::new(Rc::from_raw(self.heap_ptr()));
                Rc::clone(&*rc)
            }
        }
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
