//! Split-based intrinsics — zero intermediate Vec<Value> allocation.

/// Count the number of parts produced by splitting `s` by `sep`.
/// Equivalent to `(s $/ sep)$#` but without creating any Value or Vec.
#[inline]
pub fn count(s: &str, sep: char) -> i64 {
    s.split(sep).count() as i64
}

/// Count the number of parts produced by splitting `s` by string `sep`.
#[inline]
pub fn count_str(s: &str, sep: &str) -> i64 {
    if sep.is_empty() { return s.len() as i64; }
    s.split(sep).count() as i64
}

/// Return the first part after splitting by `sep`, or the whole string if sep not found.
#[inline]
pub fn first(s: &str, sep: char) -> &str {
    s.split(sep).next().unwrap_or(s)
}

/// Return the last part after splitting by `sep`, or the whole string if sep not found.
#[inline]
pub fn last(s: &str, sep: char) -> &str {
    s.split(sep).last().unwrap_or(s)
}

/// Split by `sep` and rejoin with `joiner` — single allocation, no intermediate Vec<Value>.
/// `(s $/ sep) → join with joiner`
pub fn join(s: &str, sep: char, joiner: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut first_part = true;
    for part in s.split(sep) {
        if !first_part { out.push_str(joiner); }
        out.push_str(part);
        first_part = false;
    }
    out
}

/// Split by string `sep` and rejoin with `joiner`.
pub fn join_str(s: &str, sep: &str, joiner: &str) -> String {
    if sep.is_empty() {
        return s.chars().map(|c| c.to_string()).collect::<Vec<_>>().join(joiner);
    }
    let mut out = String::with_capacity(s.len());
    let mut first_part = true;
    for part in s.split(sep) {
        if !first_part { out.push_str(joiner); }
        out.push_str(part);
        first_part = false;
    }
    out
}

/// Iterate parts applying a predicate; return count of matching parts.
/// Used for `(s $/ sep) $| pred` → `$#` chains.
pub fn count_where<F>(s: &str, sep: char, pred: F) -> i64
where
    F: Fn(&str) -> bool,
{
    s.split(sep).filter(|p| pred(p)).count() as i64
}

/// Collect parts into a `Vec<String>` — the allocation-aware path used by StrSplit.
/// Short parts (≤7 bytes) remain inline in ZyStr when the VM wraps them.
pub fn parts(s: &str, sep: char) -> Vec<String> {
    s.split(sep).map(|p| p.to_string()).collect()
}

pub fn parts_str(s: &str, sep: &str) -> Vec<String> {
    s.split(sep).map(|p| p.to_string()).collect()
}
